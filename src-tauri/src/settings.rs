//! Persistent LLM settings.
//!
//! Supports multiple OpenAI-compatible providers. The settings file lives in
//! the platform config directory and is shared with the LLM client through an
//! `RwLock`, so saving in the UI takes effect on the next request.
//!
//! Legacy single-provider settings (`{ api_key, base_url, model }`) are
//! migrated into a one-provider list on load.
//!
//! API keys are stored in plain text in a user-only file (0600 on Unix) and are
//! never written to logs.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use paperflow_adapters::LlmConfig;
use serde::{Deserialize, Serialize};

const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
const DEFAULT_MODEL: &str = "deepseek-chat";

/// One configurable model provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

/// The full provider list plus the id of the provider currently in use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmSettings {
    pub providers: Vec<LlmProviderConfig>,
    #[serde(default)]
    pub active_id: String,
}

impl Default for LlmSettings {
    fn default() -> Self {
        let provider = default_provider();
        Self {
            active_id: provider.id.clone(),
            providers: vec![provider],
        }
    }
}

impl LlmSettings {
    fn from_legacy(config: LlmConfig) -> Self {
        let provider = LlmProviderConfig {
            id: "default".into(),
            name: "DeepSeek 官方".into(),
            api_key: config.api_key,
            base_url: non_empty_or(config.base_url, DEFAULT_BASE_URL),
            model: non_empty_or(config.model, DEFAULT_MODEL),
        };

        Self {
            active_id: provider.id.clone(),
            providers: vec![provider],
        }
    }

    fn active_provider(&self) -> Option<&LlmProviderConfig> {
        self.providers
            .iter()
            .find(|provider| provider.id == self.active_id)
            .or_else(|| self.providers.first())
    }

    fn active_config(&self) -> LlmConfig {
        self.active_provider()
            .map(|provider| LlmConfig {
                api_key: provider.api_key.clone(),
                base_url: provider.base_url.clone(),
                model: provider.model.clone(),
            })
            .unwrap_or_default()
    }
}

fn default_provider() -> LlmProviderConfig {
    LlmProviderConfig {
        id: "deepseek".into(),
        name: "DeepSeek 官方".into(),
        api_key: String::new(),
        base_url: DEFAULT_BASE_URL.into(),
        model: DEFAULT_MODEL.into(),
    }
}

fn non_empty_or(value: String, fallback: &str) -> String {
    let value = value.trim().to_string();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

pub struct SettingsManager {
    path: PathBuf,
    settings: RwLock<LlmSettings>,
    active: Arc<RwLock<LlmConfig>>,
}

impl SettingsManager {
    /// Load settings from disk, migrating the legacy single-provider format and
    /// falling back to defaults when the file is missing or unreadable.
    pub fn load(path: PathBuf) -> Self {
        let settings = match fs::read_to_string(&path) {
            Ok(raw) => match parse_settings(&raw) {
                Some(settings) => settings,
                None => {
                    tracing::warn!(
                        path = %path.display(),
                        "invalid settings file; using defaults"
                    );
                    LlmSettings::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => LlmSettings::default(),
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "cannot read settings; using defaults"
                );
                LlmSettings::default()
            }
        };

        let active = Arc::new(RwLock::new(settings.active_config()));

        Self {
            path,
            settings: RwLock::new(settings),
            active,
        }
    }

    /// Shared handle for the LLM client. It always holds the active provider.
    pub fn shared(&self) -> Arc<RwLock<LlmConfig>> {
        self.active.clone()
    }

    pub fn current(&self) -> LlmSettings {
        self.settings
            .read()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    /// Validate, persist, then publish the new provider list and active config.
    pub fn save(&self, settings: &LlmSettings) -> Result<LlmSettings, String> {
        let settings = normalize_for_save(settings.clone())?;
        persist(&self.path, &settings)?;

        let active = settings.active_config();

        if let Ok(mut current) = self.settings.write() {
            *current = settings.clone();
        }
        if let Ok(mut current) = self.active.write() {
            *current = active;
        }

        Ok(settings)
    }
}

fn parse_settings(raw: &str) -> Option<LlmSettings> {
    if let Ok(settings) = serde_json::from_str::<LlmSettings>(raw) {
        return Some(normalize_loaded(settings));
    }

    // Migrate the old single-provider format.
    serde_json::from_str::<LlmConfig>(raw)
        .ok()
        .map(LlmSettings::from_legacy)
        .map(normalize_loaded)
}

fn normalize_loaded(mut settings: LlmSettings) -> LlmSettings {
    if settings.providers.is_empty() {
        return LlmSettings::default();
    }

    let mut seen = HashSet::new();

    for (index, provider) in settings.providers.iter_mut().enumerate() {
        provider.id = provider.id.trim().to_string();
        if provider.id.is_empty() {
            provider.id = format!("provider-{index}");
        }
        if !seen.insert(provider.id.clone()) {
            provider.id = format!("{}-{index}", provider.id);
            seen.insert(provider.id.clone());
        }

        provider.name = provider.name.trim().to_string();
        if provider.name.is_empty() {
            provider.name = format!("服务商 {}", index + 1);
        }

        provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
        if provider.base_url.is_empty() {
            provider.base_url = DEFAULT_BASE_URL.into();
        }

        provider.model = provider.model.trim().to_string();
        if provider.model.is_empty() {
            provider.model = DEFAULT_MODEL.into();
        }

        provider.api_key = provider.api_key.trim().to_string();
    }

    settings.active_id = settings.active_id.trim().to_string();
    if !settings
        .providers
        .iter()
        .any(|provider| provider.id == settings.active_id)
    {
        settings.active_id = settings.providers[0].id.clone();
    }

    settings
}

fn normalize_for_save(mut settings: LlmSettings) -> Result<LlmSettings, String> {
    if settings.providers.is_empty() {
        return Err("至少保留一个模型服务商".into());
    }

    let mut seen = HashSet::new();

    for provider in &mut settings.providers {
        provider.id = provider.id.trim().to_string();
        provider.name = provider.name.trim().to_string();
        provider.api_key = provider.api_key.trim().to_string();
        provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
        provider.model = provider.model.trim().to_string();

        if provider.id.is_empty() {
            return Err("服务商 ID 不能为空".into());
        }
        if !seen.insert(provider.id.clone()) {
            return Err(format!("服务商 ID 重复: {}", provider.id));
        }
        if provider.name.is_empty() {
            provider.name = "未命名服务商".into();
        }
        if provider.base_url.is_empty() {
            return Err(format!("「{}」的 Base URL 不能为空", provider.name));
        }
        if !provider.base_url.starts_with("http://")
            && !provider.base_url.starts_with("https://")
        {
            return Err(format!(
                "「{}」的 Base URL 必须以 http:// 或 https:// 开头",
                provider.name
            ));
        }
        if provider.model.is_empty() {
            return Err(format!("「{}」的模型名不能为空", provider.name));
        }
    }

    settings.active_id = settings.active_id.trim().to_string();
    if !settings
        .providers
        .iter()
        .any(|provider| provider.id == settings.active_id)
    {
        settings.active_id = settings.providers[0].id.clone();
    }

    Ok(settings)
}

fn persist(path: &Path, settings: &LlmSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }

    let raw = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化配置失败: {e}"))?;

    let temp = path.with_extension("json.tmp");
    fs::write(&temp, raw).map_err(|e| format!("写入配置失败: {e}"))?;
    set_private_permissions(&temp)?;

    // Windows does not replace an existing file on rename.
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("替换配置失败: {e}"))?;
    }

    fs::rename(&temp, path).map_err(|e| format!("保存配置失败: {e}"))
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|e| format!("读取配置权限失败: {e}"))?
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).map_err(|e| format!("设置配置权限失败: {e}"))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "paperflow-settings-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir.join("settings.json")
    }

    fn provider(id: &str, name: &str, base_url: &str, model: &str) -> LlmProviderConfig {
        LlmProviderConfig {
            id: id.into(),
            name: name.into(),
            api_key: format!("key-{id}"),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    #[test]
    fn defaults_to_one_active_deepseek_provider() {
        let settings = LlmSettings::default();
        assert_eq!(settings.providers.len(), 1);
        assert_eq!(settings.active_id, settings.providers[0].id);
        assert_eq!(settings.providers[0].base_url, DEFAULT_BASE_URL);
        assert_eq!(settings.providers[0].model, DEFAULT_MODEL);
    }

    #[test]
    fn saves_and_loads_multiple_providers() {
        let path = temp_path("roundtrip");
        let manager = SettingsManager::load(path.clone());

        let settings = LlmSettings {
            active_id: "second".into(),
            providers: vec![
                provider("first", "DeepSeek", "https://api.deepseek.com", "deepseek-chat"),
                provider("second", "OpenAI", "https://api.openai.com/v1", "gpt-4o-mini"),
            ],
        };

        let saved = manager.save(&settings).expect("settings should save");
        assert_eq!(saved, settings);
        assert_eq!(manager.current(), settings);
        assert_eq!(manager.shared().read().unwrap().model, "gpt-4o-mini");

        let reloaded = SettingsManager::load(path.clone());
        assert_eq!(reloaded.current(), settings);
        assert_eq!(reloaded.shared().read().unwrap().api_key, "key-second");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn migrates_legacy_single_provider_settings() {
        let path = temp_path("legacy");
        fs::write(
            &path,
            r#"{"api_key":"sk-old","base_url":"https://api.deepseek.com/","model":"deepseek-chat"}"#,
        )
        .expect("legacy file should be written");

        let manager = SettingsManager::load(path.clone());
        let settings = manager.current();

        assert_eq!(settings.providers.len(), 1);
        assert_eq!(settings.active_id, "default");
        assert_eq!(settings.providers[0].api_key, "sk-old");
        assert_eq!(settings.providers[0].base_url, "https://api.deepseek.com");
        assert_eq!(manager.shared().read().unwrap().api_key, "sk-old");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn falls_back_to_first_provider_when_active_id_is_stale() {
        let path = temp_path("stale-active");
        fs::write(
            &path,
            r#"{"active_id":"missing","providers":[{"id":"a","name":"A","base_url":"https://a.example","model":"m"}]}"#,
        )
        .expect("settings file should be written");

        let manager = SettingsManager::load(path.clone());
        assert_eq!(manager.current().active_id, "a");

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn rejects_invalid_base_url() {
        let manager = SettingsManager::load(temp_path("invalid"));

        let error = manager
            .save(&LlmSettings {
                active_id: "a".into(),
                providers: vec![provider("a", "A", "api.example.com", "m")],
            })
            .expect_err("missing scheme should be rejected");

        assert!(error.contains("http"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let manager = SettingsManager::load(temp_path("duplicate"));

        let error = manager
            .save(&LlmSettings {
                active_id: "a".into(),
                providers: vec![
                    provider("a", "A", "https://a.example", "m"),
                    provider("a", "B", "https://b.example", "m"),
                ],
            })
            .expect_err("duplicate ids should be rejected");

        assert!(error.contains("重复"));
    }
}
