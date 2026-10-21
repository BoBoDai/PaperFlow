//! Runtime LLM configuration and URL validation.

use serde::{Deserialize, Serialize};

use super::profiles::{EffectiveCapabilities, LlmCompat, ProviderProfileId};

/// Runtime LLM configuration. Debug is redacted so keys never reach logs.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub provider: ProviderProfileId,
    #[serde(default)]
    pub compat: LlmCompat,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LlmConfig")
            .field("api_key", &"***")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("provider", &self.provider)
            .field("compat", &self.compat)
            .finish()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: ProviderProfileId::DeepSeek.defaults().base_url.into(),
            model: ProviderProfileId::DeepSeek.defaults().default_model.into(),
            provider: ProviderProfileId::DeepSeek,
            compat: LlmCompat::default(),
        }
    }
}

impl LlmConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub fn capabilities(&self) -> EffectiveCapabilities {
        self.provider.capabilities().with_overrides(&self.compat)
    }
}

/// Normalize a base URL that will have `/chat/completions` or `/models`
/// appended.
pub fn base_url_host(raw: &str) -> Option<String> {
    reqwest::Url::parse(raw.trim())
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
}

pub fn normalize_base_url(raw: &str) -> std::result::Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Base URL 不能为空".into());
    }

    let url = reqwest::Url::parse(trimmed).map_err(|error| format!("Base URL 无效: {error}"))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err("Base URL 必须以 http:// 或 https:// 开头".into());
    }
    if url.host_str().is_none() {
        return Err("Base URL 缺少主机名".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Base URL 不能包含用户名或密码".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Base URL 不能包含查询参数或片段".into());
    }

    let path = url.path().trim_end_matches('/');
    if path.ends_with("/chat/completions") || path.ends_with("/models") {
        return Err(
            "Base URL 请填写到版本路径为止（例如 .../v1），不要包含 /chat/completions 或 /models"
                .into(),
        );
    }

    Ok(trimmed.to_string())
}
