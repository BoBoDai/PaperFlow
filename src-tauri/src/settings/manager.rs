//! Settings manager: load, save, and publish the active LLM config.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use paperflow_adapters::{LlmConfig, normalize_base_url};

use crate::credentials::{CredentialStore, set_private_permissions};

use super::SETTINGS_SCHEMA_VERSION;
use super::migration::{RawSettings, migrate_raw};
use super::normalize::{NormalizeMode, normalize_settings};
use super::types::{
    LlmProbeInput, LlmProviderConfig, LlmProviderView, LlmSettings, LlmSettingsInput,
    LlmSettingsView,
};

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

fn persist_settings(path: &Path, settings: &LlmSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }

    let raw = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化配置失败: {e}"))?;

    let temp = path.with_extension("json.tmp");
    fs::write(&temp, raw).map_err(|e| format!("写入配置失败: {e}"))?;
    set_private_permissions(&temp)?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("替换配置失败: {e}"))?;
    }

    fs::rename(&temp, path).map_err(|e| format!("保存配置失败: {e}"))
}
