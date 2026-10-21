//! Persisted settings types and IPC views/inputs.

use paperflow_adapters::{LlmCompat, LlmConfig, ProviderProfileId};
use serde::{Deserialize, Serialize};

use crate::credentials::CredentialStore;

use super::SETTINGS_SCHEMA_VERSION;

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
    pub(crate) fn active_provider(&self) -> Option<&LlmProviderConfig> {
        self.providers
            .iter()
            .find(|provider| provider.id == self.active_id)
            .or_else(|| self.providers.first())
    }

    pub(crate) fn active_config(&self, credentials: &CredentialStore) -> LlmConfig {
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
