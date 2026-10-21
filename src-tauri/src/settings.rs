//! Persistent LLM settings and provider configuration.
//!
//! Provider settings live in `settings.json`; API keys live in a separate
//! write-only `credentials.json` (0600 on Unix). The UI receives a redacted
//! view and never gets a saved key back.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use paperflow_adapters::{
    base_url_host, normalize_base_url, LlmCompat, LlmConfig, ProviderProfileId,
};
use serde::{Deserialize, Serialize};

use crate::credentials::{set_private_permissions, CredentialStore};

pub const SETTINGS_SCHEMA_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Persisted domain types (never contain keys)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub provider: ProviderProfileId,
    #[serde(default)]
    pub compat: LlmCompat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmSettings {
    #[serde(default)]
    pub schema_version: u32,
    pub providers: Vec<LlmProviderConfig>,
    #[serde(default)]
    pub active_id: String,
}

impl Default for LlmSettings {
    fn default() -> Self {
        let provider = default_provider();
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            active_id: provider.id.clone(),
            providers: vec![provider],
        }
    }
}

impl LlmSettings {
    fn active_provider(&self) -> Option<&LlmProviderConfig> {
        self.providers
            .iter()
            .find(|provider| provider.id == self.active_id)
            .or_else(|| self.providers.first())
    }

    fn active_config(&self, credentials: &CredentialStore) -> LlmConfig {
        match self.active_provider() {
            Some(provider) => LlmConfig {
                api_key: credentials.get(&provider.id).unwrap_or_default(),
                base_url: provider.base_url.clone(),
                model: provider.model.clone(),
                provider: provider.provider,
                compat: provider.compat.clone(),
            },
            None => LlmConfig::default(),
        }
    }
}

fn default_provider() -> LlmProviderConfig {
    let defaults = ProviderProfileId::DeepSeek.defaults();
    LlmProviderConfig {
        id: "deepseek".into(),
        name: "DeepSeek 官方".into(),
        base_url: defaults.base_url.into(),
        model: defaults.default_model.into(),
        provider: ProviderProfileId::DeepSeek,
        compat: LlmCompat::default(),
    }
}

// ---------------------------------------------------------------------------
// IPC views and inputs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmProviderView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub provider: ProviderProfileId,
    pub compat: LlmCompat,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmSettingsView {
    pub schema_version: u32,
    pub providers: Vec<LlmProviderView>,
    pub active_id: String,
}

#[derive(Clone, Deserialize)]
pub struct LlmProviderInput {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub provider: ProviderProfileId,
    #[serde(default)]
    pub compat: LlmCompat,
    /// Non-empty replaces the stored key; absent/empty keeps it.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[derive(Clone, Deserialize)]
pub struct LlmSettingsInput {
    pub providers: Vec<LlmProviderInput>,
    #[serde(default)]
    pub active_id: String,
}

/// Input for model discovery and connection tests. A blank `api_key` falls
/// back to the saved key for `provider_id`.
#[derive(Clone, Deserialize)]
pub struct LlmProbeInput {
    #[serde(default)]
    pub provider_id: Option<String>,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub provider: ProviderProfileId,
    #[serde(default)]
    pub compat: LlmCompat,
    #[serde(default)]
    pub api_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

pub struct SettingsManager {
    settings_path: PathBuf,
    credentials: RwLock<CredentialStore>,
    settings: RwLock<LlmSettings>,
    active: Arc<RwLock<LlmConfig>>,
}

impl SettingsManager {
    /// Load provider settings and credentials, migrating older formats.
    pub fn load(config_dir: PathBuf) -> Self {
        let settings_path = config_dir.join("settings.json");
        let credentials_path = config_dir.join("credentials.json");

        let mut credentials = CredentialStore::load(credentials_path);

        let (settings, should_persist) = match fs::read_to_string(&settings_path) {
            Ok(raw) => match serde_json::from_str::<RawSettings>(&raw) {
                Ok(raw_settings) => {
                    let (settings, migrated_keys, should_persist) = migrate_raw(raw_settings);
                    for (id, key) in migrated_keys {
                        credentials.set(&id, key);
                    }
                    (settings, should_persist)
                }
                Err(error) => {
                    tracing::warn!(
                        path = %settings_path.display(),
                        %error,
                        "invalid settings file; using defaults"
                    );
                    (LlmSettings::default(), false)
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (LlmSettings::default(), false)
            }
            Err(error) => {
                tracing::warn!(
                    path = %settings_path.display(),
                    %error,
                    "cannot read settings; using defaults"
                );
                (LlmSettings::default(), false)
            }
        };

        let settings = match normalize_settings(settings, NormalizeMode::Load) {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(%error, "cannot normalize settings; using defaults");
                LlmSettings::default()
            }
        };

        if should_persist {
            match credentials.persist() {
                Ok(()) => {
                    if let Err(error) = persist_settings(&settings_path, &settings) {
                        tracing::warn!(%error, "failed to persist migrated settings");
                    }
                }
                Err(error) => {
                    // Keep the plaintext settings file so a later launch can
                    // retry the migration instead of losing the keys.
                    tracing::warn!(
                        %error,
                        "failed to persist migrated credentials; keeping plaintext settings"
                    );
                }
            }
        }

        let active = Arc::new(RwLock::new(settings.active_config(&credentials)));

        Self {
            settings_path,
            credentials: RwLock::new(credentials),
            settings: RwLock::new(settings),
            active,
        }
    }

    pub fn shared(&self) -> Arc<RwLock<LlmConfig>> {
        self.active.clone()
    }

    pub fn current(&self) -> LlmSettings {
        self.settings
            .read()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn current_view(&self) -> LlmSettingsView {
        let settings = self.current();
        let credentials = self.credentials.read().unwrap_or_else(|e| e.into_inner());
        view(&settings, &credentials)
    }

    /// Validate, persist credentials and settings, then publish the new state.
    pub fn save(&self, input: &LlmSettingsInput) -> Result<LlmSettingsView, String> {
        let settings = LlmSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            providers: input
                .providers
                .iter()
                .map(|provider| LlmProviderConfig {
                    id: provider.id.clone(),
                    name: provider.name.clone(),
                    base_url: provider.base_url.clone(),
                    model: provider.model.clone(),
                    provider: provider.provider,
                    compat: provider.compat.clone(),
                })
                .collect(),
            active_id: input.active_id.clone(),
        };

        let settings = normalize_settings(settings, NormalizeMode::Save)?;
        let provider_ids: HashSet<String> = settings
            .providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect();

        {
            let mut credentials = self.credentials.write().unwrap_or_else(|e| e.into_inner());

            for provider in &input.providers {
                let id = provider.id.trim();

                if provider.clear_api_key {
                    credentials.remove(id);
                } else if let Some(key) = provider.api_key.as_deref()
                    && !key.trim().is_empty()
                {
                    credentials.set(id, key.to_string());
                }
            }

            credentials.retain(&provider_ids);
            credentials.persist()?;
        }

        persist_settings(&self.settings_path, &settings)?;

        if let Ok(mut current) = self.settings.write() {
            *current = settings.clone();
        }

        let active = {
            let credentials = self.credentials.read().unwrap_or_else(|e| e.into_inner());
            settings.active_config(&credentials)
        };

        if let Ok(mut current) = self.active.write() {
            *current = active;
        }

        let credentials = self.credentials.read().unwrap_or_else(|e| e.into_inner());
        Ok(view(&settings, &credentials))
    }

    /// Resolve a form/probe input into a runtime config, using the saved key
    /// when the incoming key is blank.
    pub fn resolve_probe(&self, input: &LlmProbeInput) -> Result<LlmConfig, String> {
        let base_url = normalize_base_url(&input.base_url)?;
        let model = input.model.trim().to_string();
        if model.is_empty() {
            return Err("请填写模型名".into());
        }

        let api_key = input
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string)
            .or_else(|| {
                input.provider_id.as_deref().and_then(|id| {
                    self.credentials
                        .read()
                        .ok()
                        .and_then(|credentials| credentials.get(id))
                })
            })
            .unwrap_or_default();

        Ok(LlmConfig {
            api_key,
            base_url,
            model,
            provider: input.provider,
            compat: input.compat.clone(),
        })
    }
}

fn view(settings: &LlmSettings, credentials: &CredentialStore) -> LlmSettingsView {
    LlmSettingsView {
        schema_version: settings.schema_version,
        active_id: settings.active_id.clone(),
        providers: settings
            .providers
            .iter()
            .map(|provider| LlmProviderView {
                id: provider.id.clone(),
                name: provider.name.clone(),
                base_url: provider.base_url.clone(),
                model: provider.model.clone(),
                provider: provider.provider,
                compat: provider.compat.clone(),
                has_api_key: credentials.has(&provider.id),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum NormalizeMode {
    Load,
    Save,
}

fn normalize_settings(
    mut settings: LlmSettings,
    mode: NormalizeMode,
) -> Result<LlmSettings, String> {
    if settings.providers.is_empty() {
        return match mode {
            NormalizeMode::Load => Ok(LlmSettings::default()),
            NormalizeMode::Save => Err("至少保留一个模型服务商".into()),
        };
    }

    let mut seen = HashSet::new();

    for (index, provider) in settings.providers.iter_mut().enumerate() {
        provider.id = provider.id.trim().to_string();
        provider.name = provider.name.trim().to_string();
        provider.base_url = provider.base_url.trim().trim_end_matches('/').to_string();
        provider.model = provider.model.trim().to_string();

        if provider.id.is_empty() {
            match mode {
                NormalizeMode::Load => provider.id = format!("provider-{index}"),
                NormalizeMode::Save => return Err("服务商 ID 不能为空".into()),
            }
        }

        if !seen.insert(provider.id.clone()) {
            match mode {
                NormalizeMode::Load => {
                    provider.id = format!("{}-{index}", provider.id);
                    seen.insert(provider.id.clone());
                }
                NormalizeMode::Save => {
                    return Err(format!("服务商 ID 重复: {}", provider.id));
                }
            }
        }

        if provider.name.is_empty() {
            provider.name = "未命名服务商".into();
        }

        let defaults = provider.provider.defaults();

        if provider.base_url.is_empty() {
            match mode {
                NormalizeMode::Load => {
                    if provider.provider != ProviderProfileId::Custom {
                        provider.base_url = defaults.base_url.to_string();
                    }
                }
                NormalizeMode::Save => {
                    return Err(format!("「{}」的 Base URL 不能为空", provider.name));
                }
            }
        } else {
            match normalize_base_url(&provider.base_url) {
                Ok(base_url) => provider.base_url = base_url,
                Err(error) => match mode {
                    NormalizeMode::Load => {
                        tracing::warn!(
                            provider = %provider.id,
                            %error,
                            "invalid base URL in settings"
                        );
                    }
                    NormalizeMode::Save => {
                        return Err(format!("「{}」的 Base URL 无效: {error}", provider.name));
                    }
                },
            }
        }

        if provider.model.is_empty() {
            match mode {
                NormalizeMode::Load => {
                    if provider.provider != ProviderProfileId::Custom {
                        provider.model = defaults.default_model.to_string();
                    }
                }
                NormalizeMode::Save => {
                    return Err(format!("「{}」的模型名不能为空", provider.name));
                }
            }
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

    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    Ok(settings)
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawSettings {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    active_id: Option<String>,
    #[serde(default)]
    providers: Option<Vec<RawProvider>>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProvider {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    compat: Option<LlmCompat>,
}

fn migrate_raw(raw: RawSettings) -> (LlmSettings, BTreeMap<String, String>, bool) {
    if let Some(raw_providers) = raw.providers {
        let mut providers = Vec::new();
        let mut keys = BTreeMap::new();
        let mut seen = HashSet::new();
        let mut should_persist = raw.schema_version != Some(SETTINGS_SCHEMA_VERSION);

        for (index, raw_provider) in raw_providers.into_iter().enumerate() {
            let mut id = raw_provider
                .id
                .unwrap_or_default()
                .trim()
                .to_string();
            if id.is_empty() {
                id = format!("provider-{index}");
            }
            if !seen.insert(id.clone()) {
                id = format!("{id}-{index}");
                seen.insert(id.clone());
            }

            let base_url = raw_provider.base_url.unwrap_or_default();
            let model = raw_provider.model.unwrap_or_default();
            let provider = infer_provider(raw_provider.provider.as_deref(), &base_url, &model);
            let defaults = provider.defaults();

            let name = raw_provider
                .name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| defaults.label.to_string());

            if let Some(key) = raw_provider
                .api_key
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty())
            {
                keys.insert(id.clone(), key);
                should_persist = true;
            }

            providers.push(LlmProviderConfig {
                id,
                name,
                base_url: if base_url.trim().is_empty() {
                    defaults.base_url.to_string()
                } else {
                    base_url.trim().to_string()
                },
                model: if model.trim().is_empty() {
                    defaults.default_model.to_string()
                } else {
                    model.trim().to_string()
                },
                provider,
                compat: raw_provider.compat.unwrap_or_default(),
            });
        }

        let active_id = raw
            .active_id
            .unwrap_or_else(|| providers.first().map(|provider| provider.id.clone()).unwrap_or_default());

        let settings = LlmSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            providers,
            active_id,
        };

        (settings, keys, should_persist)
    } else {
        // Legacy single-provider format: { api_key, base_url, model }.
        let base_url = raw.base_url.unwrap_or_default();
        let model = raw.model.unwrap_or_default();
        let provider = infer_provider(None, &base_url, &model);
        let defaults = provider.defaults();
        let id = "default".to_string();

        let mut keys = BTreeMap::new();
        if let Some(key) = raw
            .api_key
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
        {
            keys.insert(id.clone(), key);
        }

        let settings = LlmSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            providers: vec![LlmProviderConfig {
                id: id.clone(),
                name: defaults.label.to_string(),
                base_url: if base_url.trim().is_empty() {
                    defaults.base_url.to_string()
                } else {
                    base_url.trim().to_string()
                },
                model: if model.trim().is_empty() {
                    defaults.default_model.to_string()
                } else {
                    model.trim().to_string()
                },
                provider,
                compat: LlmCompat::default(),
            }],
            active_id: id,
        };

        (settings, keys, true)
    }
}

fn infer_provider(raw: Option<&str>, base_url: &str, model: &str) -> ProviderProfileId {
    if let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) {
        return match raw.to_ascii_lowercase().as_str() {
            "deepseek" => ProviderProfileId::DeepSeek,
            "minimax" => ProviderProfileId::MiniMax,
            "openai" => match base_url_host(base_url).as_deref() {
                Some("api.openai.com") => ProviderProfileId::OpenAi,
                Some("api.deepseek.com") => ProviderProfileId::DeepSeek,
                Some(host) if host.contains("minimax") => ProviderProfileId::MiniMax,
                // Existing custom gateways were saved as "openai"; keep them
                // on the max_tokens-compatible custom profile.
                _ => ProviderProfileId::Custom,
            },
            "custom" => ProviderProfileId::Custom,
            _ => ProviderProfileId::Custom,
        };
    }

    match base_url_host(base_url).as_deref() {
        Some("api.deepseek.com") => ProviderProfileId::DeepSeek,
        Some("api.openai.com") => ProviderProfileId::OpenAi,
        Some(host) if host.contains("minimax") => ProviderProfileId::MiniMax,
        _ if model.to_ascii_lowercase().starts_with("minimax") => ProviderProfileId::MiniMax,
        _ => ProviderProfileId::Custom,
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

fn persist_settings(path: &Path, settings: &LlmSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }

    let raw = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化配置失败: {e}"))?;

    let temp = path.with_extension("json.tmp");
    fs::write(&temp, raw).map_err(|e| format!("写入配置失败: {e}"))?;
    set_private_permissions(&temp)?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("替换配置失败: {e}"))?;
    }

    fs::rename(&temp, path).map_err(|e| format!("保存配置失败: {e}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "paperflow-settings-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    fn provider(
        id: &str,
        name: &str,
        base_url: &str,
        model: &str,
        provider: ProviderProfileId,
    ) -> LlmProviderInput {
        LlmProviderInput {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            model: model.into(),
            provider,
            compat: LlmCompat::default(),
            api_key: None,
            clear_api_key: false,
        }
    }

    #[test]
    fn defaults_to_one_active_deepseek_provider() {
        let settings = LlmSettings::default();
        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.providers.len(), 1);
        assert_eq!(settings.active_id, settings.providers[0].id);
        assert_eq!(settings.providers[0].provider, ProviderProfileId::DeepSeek);
    }

    #[test]
    fn saves_redacted_view_and_separate_credentials() {
        let dir = temp_dir("roundtrip");
        let manager = SettingsManager::load(dir.clone());

        let mut first = provider(
            "deepseek",
            "DeepSeek 官方",
            "https://api.deepseek.com/",
            "deepseek-chat",
            ProviderProfileId::DeepSeek,
        );
        first.api_key = Some("sk-deepseek".into());

        let mut second = provider(
            "minimax",
            "MiniMax",
            "https://api.minimaxi.com/v1",
            "MiniMax-M2",
            ProviderProfileId::MiniMax,
        );
        second.api_key = Some("sk-minimax".into());

        let saved = manager
            .save(&LlmSettingsInput {
                active_id: "minimax".into(),
                providers: vec![first, second],
            })
            .expect("settings should save");

        assert_eq!(saved.active_id, "minimax");
        assert!(saved.providers.iter().all(|provider| provider.has_api_key));
        assert!(!serde_json::to_string(&saved).unwrap().contains("sk-"));

        let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!settings_raw.contains("sk-"));
        let credentials_raw = fs::read_to_string(dir.join("credentials.json")).unwrap();
        assert!(credentials_raw.contains("sk-deepseek"));

        let reloaded = SettingsManager::load(dir.clone());
        let view = reloaded.current_view();
        assert_eq!(view.providers.len(), 2);
        assert!(view.providers.iter().all(|provider| provider.has_api_key));
        assert_eq!(reloaded.shared().read().unwrap().model, "MiniMax-M2");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn blank_key_keeps_existing_and_clear_removes() {
        let dir = temp_dir("keys");
        let manager = SettingsManager::load(dir.clone());

        let mut provider = provider(
            "deepseek",
            "DeepSeek 官方",
            "https://api.deepseek.com",
            "deepseek-chat",
            ProviderProfileId::DeepSeek,
        );
        provider.api_key = Some("sk-test".into());
        manager
            .save(&LlmSettingsInput {
                active_id: "deepseek".into(),
                providers: vec![provider.clone()],
            })
            .expect("settings should save");

        provider.api_key = None;
        let saved = manager
            .save(&LlmSettingsInput {
                active_id: "deepseek".into(),
                providers: vec![provider.clone()],
            })
            .expect("blank key should keep existing");
        assert!(saved.providers[0].has_api_key);

        provider.clear_api_key = true;
        let saved = manager
            .save(&LlmSettingsInput {
                active_id: "deepseek".into(),
                providers: vec![provider],
            })
            .expect("clear should remove");
        assert!(!saved.providers[0].has_api_key);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn deleting_a_provider_prunes_its_key() {
        let dir = temp_dir("prune");
        let manager = SettingsManager::load(dir.clone());

        let mut first = provider(
            "a",
            "A",
            "https://a.example/v1",
            "model-a",
            ProviderProfileId::Custom,
        );
        first.api_key = Some("key-a".into());
        let mut second = provider(
            "b",
            "B",
            "https://b.example/v1",
            "model-b",
            ProviderProfileId::Custom,
        );
        second.api_key = Some("key-b".into());

        manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![first, second],
            })
            .expect("settings should save");

        let first = provider(
            "a",
            "A",
            "https://a.example/v1",
            "model-a",
            ProviderProfileId::Custom,
        );
        manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![first],
            })
            .expect("settings should save");

        let credentials = fs::read_to_string(dir.join("credentials.json")).unwrap();
        assert!(credentials.contains("key-a"));
        assert!(!credentials.contains("key-b"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_legacy_single_provider_with_key() {
        let dir = temp_dir("legacy");
        fs::write(
            dir.join("settings.json"),
            r#"{"api_key":"sk-old","base_url":"https://api.deepseek.com/","model":"deepseek-chat"}"#,
        )
        .unwrap();

        let manager = SettingsManager::load(dir.clone());
        let view = manager.current_view();

        assert_eq!(view.providers.len(), 1);
        assert_eq!(view.providers[0].provider, ProviderProfileId::DeepSeek);
        assert!(view.providers[0].has_api_key);

        let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!settings_raw.contains("sk-old"));
        let credentials_raw = fs::read_to_string(dir.join("credentials.json")).unwrap();
        assert!(credentials_raw.contains("sk-old"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_current_format_and_keeps_custom_gateways_custom() {
        let dir = temp_dir("current");
        fs::write(
            dir.join("settings.json"),
            r#"{
              "active_id": "gateway",
              "providers": [
                {"id":"gateway","name":"Gateway","base_url":"https://gateway.example/v1","model":"custom-chat","provider":"openai","api_key":"sk-gateway"},
                {"id":"mini","name":"MiniMax","base_url":"https://api.minimaxi.com/v1","model":"MiniMax-M2","provider":"minimax","api_key":"sk-mini"}
              ]
            }"#,
        )
        .unwrap();

        let manager = SettingsManager::load(dir.clone());
        let view = manager.current_view();

        assert_eq!(view.providers[0].provider, ProviderProfileId::Custom);
        assert_eq!(view.providers[1].provider, ProviderProfileId::MiniMax);
        assert!(view.providers.iter().all(|provider| provider.has_api_key));

        let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(!settings_raw.contains("sk-gateway"));
        assert!(!settings_raw.contains("sk-mini"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_invalid_base_url_and_duplicate_ids() {
        let dir = temp_dir("validation");
        let manager = SettingsManager::load(dir.clone());

        let invalid = provider(
            "a",
            "A",
            "api.example.com",
            "model",
            ProviderProfileId::Custom,
        );
        assert!(manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![invalid],
            })
            .is_err());

        let duplicate_a = provider(
            "a",
            "A",
            "https://a.example/v1",
            "model",
            ProviderProfileId::Custom,
        );
        let duplicate_b = provider(
            "a",
            "B",
            "https://b.example/v1",
            "model",
            ProviderProfileId::Custom,
        );
        assert!(manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![duplicate_a, duplicate_b],
            })
            .is_err());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stale_active_id_falls_back_to_first_provider() {
        let dir = temp_dir("stale");
        fs::write(
            dir.join("settings.json"),
            r#"{"schema_version":2,"active_id":"missing","providers":[{"id":"a","name":"A","base_url":"https://a.example/v1","model":"m","provider":"custom"}]}"#,
        )
        .unwrap();

        let manager = SettingsManager::load(dir.clone());
        assert_eq!(manager.current_view().active_id, "a");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_credentials_do_not_break_settings() {
        let dir = temp_dir("corrupt-credentials");
        fs::write(dir.join("credentials.json"), "{not json").unwrap();

        let manager = SettingsManager::load(dir.clone());
        assert!(!manager.current_view().providers[0].has_api_key);

        let _ = fs::remove_dir_all(dir);
    }
}
