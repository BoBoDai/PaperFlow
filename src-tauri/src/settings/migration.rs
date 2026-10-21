//! Migration from legacy single-provider and unversioned settings.

use std::collections::{BTreeMap, HashSet};

use paperflow_adapters::{LlmCompat, ProviderProfileId, base_url_host};
use serde::Deserialize;

use super::SETTINGS_SCHEMA_VERSION;
use super::types::{LlmProviderConfig, LlmSettings};

#[derive(Debug, Deserialize)]
pub(crate) struct RawSettings {
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

pub(crate) fn migrate_raw(raw: RawSettings) -> (LlmSettings, BTreeMap<String, String>, bool) {
    if let Some(raw_providers) = raw.providers {
        let mut providers = Vec::new();
        let mut keys = BTreeMap::new();
        let mut seen = HashSet::new();
        let mut should_persist = raw.schema_version != Some(SETTINGS_SCHEMA_VERSION);

        for (index, raw_provider) in raw_providers.into_iter().enumerate() {
            let mut id = raw_provider.id.unwrap_or_default().trim().to_string();
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

        let active_id = raw.active_id.unwrap_or_else(|| {
            providers
                .first()
                .map(|provider| provider.id.clone())
                .unwrap_or_default()
        });

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
