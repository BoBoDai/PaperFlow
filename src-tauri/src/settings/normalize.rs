//! Validation and normalization of persisted settings.

use std::collections::HashSet;

use paperflow_adapters::{ProviderProfileId, normalize_base_url};

use super::SETTINGS_SCHEMA_VERSION;
use super::types::LlmSettings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalizeMode {
    Load,
    Save,
}

pub(crate) fn normalize_settings(
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
