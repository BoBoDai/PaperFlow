//! LLM provider support.
//!
//! The client speaks the OpenAI Chat Completions protocol, so it works with
//! DeepSeek, OpenAI, MiniMax, and custom compatible gateways.
//!
//! ## The most important rule
//!
//! **LLM output must pass structural validation before it is returned.**
//!
//! Never fall back to "parse failed, use the raw text" — that is the worst
//! failure mode: the user sees garbage and the logs stay quiet. Validation
//! failures return [`paperflow_core::Error::InvalidLlmOutput`].

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use paperflow_core::{contains_cjk, Error, Paper, Result, Summary};
use serde::{Deserialize, Serialize};

use crate::search::describe_error;

// ---------------------------------------------------------------------------
// Provider profiles
// ---------------------------------------------------------------------------

/// Provider family. The profile supplies defaults and request capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProviderProfileId {
    DeepSeek,
    #[default]
    OpenAi,
    MiniMax,
    Custom,
}

impl ProviderProfileId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAi => "openai",
            Self::MiniMax => "minimax",
            Self::Custom => "custom",
        }
    }

    pub fn from_id(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" => Self::OpenAi,
            "deepseek" => Self::DeepSeek,
            "openai" => Self::OpenAi,
            "minimax" => Self::MiniMax,
            _ => Self::Custom,
        }
    }

    pub const fn defaults(self) -> ProviderDefaults {
        match self {
            Self::DeepSeek => ProviderDefaults {
                label: "DeepSeek",
                base_url: "https://api.deepseek.com",
                default_model: "deepseek-chat",
            },
            Self::OpenAi => ProviderDefaults {
                label: "OpenAI",
                base_url: "https://api.openai.com/v1",
                default_model: "gpt-4o-mini",
            },
            Self::MiniMax => ProviderDefaults {
                label: "MiniMax",
                base_url: "https://api.minimaxi.com/v1",
                default_model: "MiniMax-M2",
            },
            Self::Custom => ProviderDefaults {
                label: "自定义服务商",
                base_url: "",
                default_model: "",
            },
        }
    }

    pub const fn capabilities(self) -> EffectiveCapabilities {
        match self {
            Self::DeepSeek => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::OpenAi => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxCompletionTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::MiniMax => EffectiveCapabilities {
                json_mode: false,
                max_tokens_field: MaxTokensField::MaxTokens,
                // Omit thinking by default: M3.1 requires adaptive thinking
                // and rejects `disabled`, while M2.x works with the default.
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::Custom => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
        }
    }
}

impl Serialize for ProviderProfileId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderProfileId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::from_id(&raw))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDefaults {
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    MaxTokens,
    MaxCompletionTokens,
}

impl MaxTokensField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingFormat {
    Thinking,
    EnableThinking,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingMode {
    /// Omit the thinking parameter and use the provider's default.
    #[default]
    Default,
    /// Send `thinking: {"type": "disabled"}` (or `enable_thinking: false`).
    Disabled,
    /// Send `thinking: {"type": "adaptive"}` (required by some models).
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EffectiveCapabilities {
    pub json_mode: bool,
    pub max_tokens_field: MaxTokensField,
    pub thinking_mode: ThinkingMode,
    pub thinking_format: ThinkingFormat,
}

/// Per-provider compatibility overrides. `None` means inherit the profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmCompat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<MaxTokensField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_mode: Option<ThinkingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<ThinkingFormat>,
}

impl EffectiveCapabilities {
    pub fn with_overrides(self, compat: &LlmCompat) -> Self {
        Self {
            json_mode: compat.json_mode.unwrap_or(self.json_mode),
            max_tokens_field: compat.max_tokens_field.unwrap_or(self.max_tokens_field),
            thinking_mode: compat.thinking_mode.unwrap_or(self.thinking_mode),
            thinking_format: compat.thinking_format.unwrap_or(self.thinking_format),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderProfileInfo {
    pub id: ProviderProfileId,
    pub label: &'static str,
    pub default_base_url: &'static str,
    pub default_model: &'static str,
    pub capabilities: EffectiveCapabilities,
    pub protocol: &'static str,
}

pub fn provider_catalog() -> Vec<ProviderProfileInfo> {
    [
        ProviderProfileId::DeepSeek,
        ProviderProfileId::OpenAi,
        ProviderProfileId::MiniMax,
        ProviderProfileId::Custom,
    ]
    .into_iter()
    .map(|id| {
        let defaults = id.defaults();
        ProviderProfileInfo {
            id,
            label: defaults.label,
            default_base_url: defaults.base_url,
            default_model: defaults.default_model,
            capabilities: id.capabilities(),
            protocol: "openai_chat_completions",
        }
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Runtime config
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    fn is_configured(&self) -> bool;
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;
    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String>;
}

// ---------------------------------------------------------------------------
// Error model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmErrorKind {
    MissingConfig,
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    Server,
    Timeout,
    Network,
    UnknownModel,
    InvalidResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmEndpointError {
    pub kind: LlmErrorKind,
    pub message: String,
    pub status: Option<u16>,
    pub request_id: Option<String>,
    pub provider_message: Option<String>,
}

impl std::fmt::Display for LlmEndpointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)?;
        if let Some(request_id) = &self.request_id {
            write!(formatter, " (request_id: {request_id})")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmModelList {
    pub models: Vec<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub model: String,
    pub request_id: Option<String>,
}

impl std::error::Error for LlmEndpointError {}

impl LlmEndpointError {
    fn missing(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::MissingConfig,
            message: message.into(),
            status: None,
            request_id: None,
            provider_message: None,
        }
    }

    fn invalid_response(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::InvalidResponse,
            message: message.into(),
            status: None,
            request_id: None,
            provider_message: None,
        }
    }
}

fn truncate(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }

    let truncated: String = trimmed.chars().take(limit).collect();
    format!("{truncated}…")
}

fn request_id_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    [
        "x-request-id",
        "request-id",
        "openai-request-id",
        "x-ds-request-id",
        "cf-ray",
    ]
    .iter()
    .find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    })
}

fn map_http_error(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &str,
) -> LlmEndpointError {
    let json: Option<serde_json::Value> = serde_json::from_str(body).ok();

    let provider_message = json
        .as_ref()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(|message| message.as_str())
                .or_else(|| value.get("message").and_then(|message| message.as_str()))
        })
        .map(|message| truncate(message, 500))
        .or_else(|| {
            if body.trim().is_empty() {
                None
            } else {
                Some(truncate(body, 500))
            }
        });

    let code = json.as_ref().and_then(|value| {
        value
            .pointer("/error/code")
            .and_then(|code| code.as_str())
            .or_else(|| value.pointer("/error/type").and_then(|code| code.as_str()))
    });

    let request_id = request_id_from_headers(headers).or_else(|| {
        json.as_ref()
            .and_then(|value| value.get("request_id"))
            .and_then(|id| id.as_str())
            .map(str::to_string)
    });

    let model_issue = code
        .map(|code| {
            let code = code.to_ascii_lowercase();
            code.contains("model")
                && (code.contains("not_found")
                    || code.contains("invalid")
                    || code.contains("unknown"))
        })
        .unwrap_or(false)
        || provider_message
            .as_deref()
            .map(|message| {
                let message = message.to_ascii_lowercase();
                message.contains("unknown model")
                    || message.contains("invalid model")
                    || message.contains("model not found")
                    || message.contains("does not exist")
                    || message.contains("no such model")
            })
            .unwrap_or(false);

    let (kind, mut message) = if model_issue {
        (
            LlmErrorKind::UnknownModel,
            "模型不可用，请拉取模型列表或检查模型名".to_string(),
        )
    } else {
        match status.as_u16() {
            400 => (
                LlmErrorKind::BadRequest,
                "请求参数无效".to_string(),
            ),
            401 => (
                LlmErrorKind::Unauthorized,
                "鉴权失败，请检查 API key".to_string(),
            ),
            403 => (
                LlmErrorKind::Forbidden,
                "没有访问权限或额度不足".to_string(),
            ),
            404 => (
                LlmErrorKind::NotFound,
                "接口地址不存在，请检查 Base URL（可能缺少 /v1）".to_string(),
            ),
            429 => (
                LlmErrorKind::RateLimited,
                "请求被限流，请稍后重试".to_string(),
            ),
            500..=599 => (
                LlmErrorKind::Server,
                "服务端错误，请稍后重试".to_string(),
            ),
            _ => (
                LlmErrorKind::BadRequest,
                format!("LLM 返回 HTTP {status}"),
            ),
        }
    };

    if status == reqwest::StatusCode::BAD_REQUEST
        && provider_message
            .as_deref()
            .map(|text| {
                let text = text.to_ascii_lowercase();
                text.contains("response_format") || text.contains("json_object")
            })
            .unwrap_or(false)
    {
        message.push_str("（可在高级兼容性中关闭 JSON 模式）");
    }

    if let Some(provider_message) = &provider_message {
        message = format!("{message}: {provider_message}");
    }

    LlmEndpointError {
        kind,
        message,
        status: Some(status.as_u16()),
        request_id,
        provider_message,
    }
}

fn transport_error(context: &str, error: reqwest::Error) -> LlmEndpointError {
    let (kind, message) = if error.is_timeout() {
        (LlmErrorKind::Timeout, "请求超时，请检查网络或代理".to_string())
    } else {
        (
            LlmErrorKind::Network,
            describe_error(context, &error),
        )
    };

    LlmEndpointError {
        kind,
        message,
        status: None,
        request_id: None,
        provider_message: None,
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    config: Arc<RwLock<LlmConfig>>,
}

impl OpenAiCompatibleClient {
    pub fn new(config: Arc<RwLock<LlmConfig>>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("PaperFlow/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Config(format!("failed to build LLM HTTP client: {e}")))?;

        Ok(Self { http, config })
    }

    fn current_config(&self) -> Result<LlmConfig> {
        self.config
            .read()
            .map(|config| config.clone())
            .map_err(|_| Error::Config("LLM 配置不可用".into()))
    }

    async fn chat(
        &self,
        config: &LlmConfig,
        messages: Vec<serde_json::Value>,
        max_tokens: u32,
        timeout: Duration,
    ) -> Result<String> {
        if !config.is_configured() {
            return Err(Error::Config(
                "请先在设置中填写 API key".into(),
            ));
        }

        let base_url = normalize_base_url(&config.base_url).map_err(Error::Config)?;
        if config.model.trim().is_empty() {
            return Err(Error::Config("LLM 模型名不能为空".into()));
        }

        let body = build_chat_body(config, messages, max_tokens);

        let response = self
            .http
            .post(format!("{base_url}/chat/completions"))
            .bearer_auth(config.api_key.trim())
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Llm(transport_error("LLM request failed", e).to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(
                map_http_error(status, &headers, &body).to_string(),
            ));
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::Llm(transport_error("failed to read LLM response", e).to_string()))?;

        parse_chat_content(&body)
    }

    /// Fetch model IDs from `{base_url}/models`.
    pub async fn fetch_models(
        &self,
        config: &LlmConfig,
    ) -> std::result::Result<LlmModelList, LlmEndpointError> {
        if !config.is_configured() {
            return Err(LlmEndpointError::missing(
                "请先填写或保存 API Key",
            ));
        }

        let base_url = normalize_base_url(&config.base_url).map_err(LlmEndpointError::missing)?;

        let response = self
            .http
            .get(format!("{base_url}/models"))
            .bearer_auth(config.api_key.trim())
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| transport_error("model list request failed", e))?;

        let request_id = request_id_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            let mut error = map_http_error(status, &headers, &body);
            if status == reqwest::StatusCode::NOT_FOUND {
                error.message =
                    "该端点可能不支持 /models，请手动输入模型 ID，或检查 Base URL".into();
                error.kind = LlmErrorKind::NotFound;
            }
            return Err(error);
        }

        let body = response
            .text()
            .await
            .map_err(|e| transport_error("failed to read model list", e))?;

        let models = parse_models_response(&body)?;
        Ok(LlmModelList { models, request_id })
    }

    /// Send a tiny chat request to verify endpoint, auth, and model.
    pub async fn test_connection(
        &self,
        config: &LlmConfig,
    ) -> std::result::Result<ConnectionTestResult, LlmEndpointError> {
        if !config.is_configured() {
            return Err(LlmEndpointError::missing(
                "请先填写或保存 API Key",
            ));
        }

        let base_url = normalize_base_url(&config.base_url).map_err(LlmEndpointError::missing)?;
        if config.model.trim().is_empty() {
            return Err(LlmEndpointError::missing("请填写模型名"));
        }

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "ping",
        })];
        let body = build_chat_body(config, messages, 16);

        let started = Instant::now();
        let response = self
            .http
            .post(format!("{base_url}/chat/completions"))
            .bearer_auth(config.api_key.trim())
            .timeout(Duration::from_secs(30))
            .json(&body)
            .send()
            .await
            .map_err(|e| transport_error("connection test failed", e))?;

        let latency_ms = started.elapsed().as_millis() as u64;
        let request_id = request_id_from_headers(response.headers());

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            return Err(map_http_error(status, &headers, &body));
        }

        Ok(ConnectionTestResult {
            ok: true,
            latency_ms,
            model: config.model.trim().to_string(),
            request_id,
        })
    }
}

impl LlmProvider for OpenAiCompatibleClient {
    fn is_configured(&self) -> bool {
        self.current_config()
            .map(|config| config.is_configured())
            .unwrap_or(false)
    }

    async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        let config = self.current_config()?;
        tracing::info!(id = %paper.id, model = %config.model, "requesting paper summary");

        let authors = paper.authors.join(", ");
        let categories = paper.categories.join(", ");
        let user = format!(
            "Title: {}\nAuthors: {}\nCategories: {}\nAbstract:\n{}",
            paper.title,
            if authors.is_empty() { "unknown" } else { &authors },
            if categories.is_empty() {
                "unknown"
            } else {
                &categories
            },
            paper.abstract_text,
        );

        let messages = vec![
            serde_json::json!({ "role": "system", "content": SUMMARY_SYSTEM_PROMPT }),
            serde_json::json!({ "role": "user", "content": user }),
        ];

        let content = self
            .chat(&config, messages, 1200, Duration::from_secs(90))
            .await?;

        parse_summary(&content, config.model.trim())
    }

    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String> {
        let config = self.current_config()?;
        tracing::info!(query = chinese_query, model = %config.model, "translating query");

        let messages = vec![
            serde_json::json!({ "role": "system", "content": KEYWORDS_SYSTEM_PROMPT }),
            serde_json::json!({ "role": "user", "content": chinese_query }),
        ];

        let content = self
            .chat(&config, messages, 120, Duration::from_secs(25))
            .await?;

        parse_keywords(&content)
    }
}

// ---------------------------------------------------------------------------
// Request/response helpers
// ---------------------------------------------------------------------------

/// Build the chat-completions body from the effective provider capabilities.
fn build_chat_body(
    config: &LlmConfig,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
) -> serde_json::Value {
    let capabilities = config.capabilities();

    let mut body = serde_json::json!({
        "model": config.model.trim(),
        "messages": messages,
        "temperature": 0.2,
        "stream": false,
    });

    body[capabilities.max_tokens_field.as_str()] = serde_json::json!(max_tokens);

    if capabilities.json_mode {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }

    match (capabilities.thinking_mode, capabilities.thinking_format) {
        (ThinkingMode::Default, _) => {}
        (ThinkingMode::Disabled, ThinkingFormat::Thinking) => {
            body["thinking"] = serde_json::json!({ "type": "disabled" });
        }
        (ThinkingMode::Disabled, ThinkingFormat::EnableThinking) => {
            body["enable_thinking"] = serde_json::json!(false);
        }
        (ThinkingMode::Adaptive, ThinkingFormat::Thinking) => {
            body["thinking"] = serde_json::json!({ "type": "adaptive" });
        }
        (ThinkingMode::Adaptive, ThinkingFormat::EnableThinking) => {
            body["enable_thinking"] = serde_json::json!(true);
        }
    }

    body
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

    let url = reqwest::Url::parse(trimmed)
        .map_err(|error| format!("Base URL 无效: {error}"))?;

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

const SUMMARY_SYSTEM_PROMPT: &str = r#"You are an academic paper analyst.
Analyze the paper and reply with JSON only. The JSON object must have exactly these fields:
{
  "short": "one-sentence summary in Simplified Chinese",
  "detailed": "a detailed analysis in Simplified Chinese, 3-5 sentences",
  "key_points": ["3-6 key points in Simplified Chinese"]
}
Do not use markdown fences, comments, or any text outside the JSON object."#;

const KEYWORDS_SYSTEM_PROMPT: &str = r#"Translate the user's Chinese academic search query into concise English keywords for arXiv and OpenAlex.
Preserve well-known acronyms such as LLM, RAG, GAN, and Transformer.
Reply with JSON only, exactly in this shape: {"keywords": "3-8 English keywords separated by spaces"}
Do not include Chinese characters, quotes, explanations, or markdown fences."#;

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
}

fn parse_chat_content(body: &str) -> Result<String> {
    let response: ChatResponse = serde_json::from_str(body)
        .map_err(|e| Error::InvalidLlmOutput(format!("invalid chat completion response: {e}")))?;

    response
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .map(|content| strip_think_block(&content).to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| Error::InvalidLlmOutput("LLM returned an empty response".into()))
}

fn strip_think_block(content: &str) -> &str {
    if let Some(position) = content.rfind("</think")
        && let Some(end) = content[position..].find('>')
    {
        return content[position + end + 1..].trim();
    }

    content.trim()
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: Option<String>,
}

fn parse_models_response(body: &str) -> std::result::Result<Vec<String>, LlmEndpointError> {
    let response: ModelsResponse = serde_json::from_str(body)
        .map_err(|e| LlmEndpointError::invalid_response(format!("invalid model list response: {e}")))?;

    let mut models: Vec<String> = response
        .data
        .into_iter()
        .filter_map(|entry| entry.id)
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    models.sort_by_key(|id| id.to_ascii_lowercase());
    models.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(models)
}

#[derive(Debug, Deserialize)]
struct SummaryPayload {
    short: String,
    detailed: String,
    key_points: Vec<String>,
}

pub(crate) fn parse_summary(content: &str, model: &str) -> Result<Summary> {
    let payload: SummaryPayload = serde_json::from_str(content).map_err(|e| {
        Error::InvalidLlmOutput(format!("summary JSON does not match the schema: {e}"))
    })?;

    let short = payload.short.trim().to_string();
    let detailed = payload.detailed.trim().to_string();
    let key_points: Vec<String> = payload
        .key_points
        .into_iter()
        .map(|point| point.trim().to_string())
        .filter(|point| !point.is_empty())
        .collect();

    if short.is_empty() {
        return Err(Error::InvalidLlmOutput("summary.short is empty".into()));
    }
    if detailed.is_empty() {
        return Err(Error::InvalidLlmOutput("summary.detailed is empty".into()));
    }
    if key_points.is_empty() {
        return Err(Error::InvalidLlmOutput("summary.key_points is empty".into()));
    }

    Ok(Summary {
        short,
        detailed,
        key_points,
        model: model.to_string(),
        prompt_version: Summary::PROMPT_VERSION.into(),
    })
}

#[derive(Debug, Deserialize)]
struct KeywordsPayload {
    keywords: String,
}

pub(crate) fn parse_keywords(content: &str) -> Result<String> {
    let payload: KeywordsPayload = serde_json::from_str(content).map_err(|e| {
        Error::InvalidLlmOutput(format!("keywords JSON does not match the schema: {e}"))
    })?;

    let keywords = payload.keywords.trim().to_string();
    if keywords.is_empty() {
        return Err(Error::InvalidLlmOutput("keywords is empty".into()));
    }
    if contains_cjk(&keywords) {
        return Err(Error::InvalidLlmOutput(
            "keywords still contain Chinese characters".into(),
        ));
    }

    Ok(keywords)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(provider: ProviderProfileId) -> LlmConfig {
        let defaults = provider.defaults();
        LlmConfig {
            api_key: "sk-test".into(),
            base_url: defaults.base_url.into(),
            model: defaults.default_model.into(),
            provider,
            compat: LlmCompat::default(),
        }
    }

    #[test]
    fn provider_capabilities_match_profiles() {
        let deepseek = ProviderProfileId::DeepSeek.capabilities();
        assert!(deepseek.json_mode);
        assert_eq!(deepseek.max_tokens_field, MaxTokensField::MaxTokens);

        let openai = ProviderProfileId::OpenAi.capabilities();
        assert_eq!(
            openai.max_tokens_field,
            MaxTokensField::MaxCompletionTokens
        );

        let minimax = ProviderProfileId::MiniMax.capabilities();
        assert!(!minimax.json_mode);
        assert_eq!(minimax.thinking_mode, ThinkingMode::Default);
    }

    #[test]
    fn provider_profile_id_parses_leniently() {
        assert_eq!(
            ProviderProfileId::from_id("deepseek"),
            ProviderProfileId::DeepSeek
        );
        assert_eq!(
            ProviderProfileId::from_id(" OPENAI "),
            ProviderProfileId::OpenAi
        );
        assert_eq!(ProviderProfileId::from_id(""), ProviderProfileId::OpenAi);
        assert_eq!(
            ProviderProfileId::from_id("unknown"),
            ProviderProfileId::Custom
        );
    }

    #[test]
    fn build_body_uses_profile_capabilities() {
        let deepseek = build_chat_body(&config(ProviderProfileId::DeepSeek), vec![], 10);
        assert_eq!(deepseek["response_format"]["type"], "json_object");
        assert_eq!(deepseek["max_tokens"].as_u64(), Some(10));
        assert!(deepseek.get("max_completion_tokens").is_none());

        let openai = build_chat_body(&config(ProviderProfileId::OpenAi), vec![], 10);
        assert_eq!(openai["max_completion_tokens"].as_u64(), Some(10));
        assert!(openai.get("max_tokens").is_none());

        let minimax = build_chat_body(&config(ProviderProfileId::MiniMax), vec![], 10);
        assert!(minimax.get("response_format").is_none());
        assert!(minimax.get("thinking").is_none());

        let mut minimax_disabled = config(ProviderProfileId::MiniMax);
        minimax_disabled.compat.thinking_mode = Some(ThinkingMode::Disabled);
        let body = build_chat_body(&minimax_disabled, vec![], 10);
        assert_eq!(body["thinking"]["type"], "disabled");
    }

    #[test]
    fn compat_overrides_take_precedence() {
        let mut config = config(ProviderProfileId::Custom);
        config.compat = LlmCompat {
            json_mode: Some(false),
            max_tokens_field: Some(MaxTokensField::MaxCompletionTokens),
            thinking_mode: Some(ThinkingMode::Disabled),
            thinking_format: Some(ThinkingFormat::EnableThinking),
        };

        let body = build_chat_body(&config, vec![], 10);
        assert!(body.get("response_format").is_none());
        assert_eq!(body["max_completion_tokens"].as_u64(), Some(10));
        assert_eq!(body["enable_thinking"].as_bool(), Some(false));
    }

    #[test]
    fn adaptive_thinking_is_sent_when_requested() {
        let mut config = config(ProviderProfileId::MiniMax);
        config.compat = LlmCompat {
            thinking_mode: Some(ThinkingMode::Adaptive),
            ..LlmCompat::default()
        };

        let body = build_chat_body(&config, vec![], 10);
        assert_eq!(body["thinking"]["type"], "adaptive");
    }

    #[test]
    fn normalizes_base_urls() {
        assert_eq!(
            normalize_base_url(" https://api.deepseek.com/ ").unwrap(),
            "https://api.deepseek.com"
        );
        assert_eq!(
            normalize_base_url("http://localhost:11434/v1").unwrap(),
            "http://localhost:11434/v1"
        );
        assert!(normalize_base_url("api.deepseek.com").is_err());
        assert!(normalize_base_url("https://api.openai.com/v1/chat/completions").is_err());
        assert!(normalize_base_url("https://user:pass@example.com/v1").is_err());
    }

    #[test]
    fn parses_and_sorts_model_list() {
        let body =
            r#"{"data":[{"id":"b-model"},{"id":"A-model"},{"id":"a-model"},{"id":""}]}"#;
        let models = parse_models_response(body).expect("models should parse");

        assert_eq!(models, vec!["A-model".to_string(), "b-model".to_string()]);
    }

    #[test]
    fn maps_unknown_model_errors() {
        let error = map_http_error(
            reqwest::StatusCode::BAD_REQUEST,
            &reqwest::header::HeaderMap::new(),
            r#"{"error":{"message":"unknown model 'foo'","type":"bad_request_error"},"request_id":"req-1"}"#,
        );

        assert_eq!(error.kind, LlmErrorKind::UnknownModel);
        assert_eq!(error.request_id.as_deref(), Some("req-1"));
        assert!(error.message.contains("模型不可用"));
    }

    #[test]
    fn maps_auth_and_rate_limit_errors() {
        let headers = reqwest::header::HeaderMap::new();

        let unauthorized = map_http_error(reqwest::StatusCode::UNAUTHORIZED, &headers, "{}");
        assert_eq!(unauthorized.kind, LlmErrorKind::Unauthorized);

        let limited =
            map_http_error(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, "{}");
        assert_eq!(limited.kind, LlmErrorKind::RateLimited);
    }

    #[test]
    fn debug_redacts_api_key() {
        let debug = format!("{:?}", config(ProviderProfileId::DeepSeek));
        assert!(!debug.contains("sk-test"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn parses_valid_summary() {
        let content =
            r#"{"short":"一句话","detailed":"详细分析","key_points":["要点一","要点二"]}"#;
        let summary = parse_summary(content, "deepseek-chat").expect("summary should parse");

        assert_eq!(summary.short, "一句话");
        assert_eq!(summary.detailed, "详细分析");
        assert_eq!(summary.key_points, ["要点一", "要点二"]);
        assert_eq!(summary.model, "deepseek-chat");
        assert_eq!(summary.prompt_version, Summary::PROMPT_VERSION);
    }

    #[test]
    fn rejects_summary_missing_fields() {
        assert!(matches!(
            parse_summary(r#"{"short":"x"}"#, "m"),
            Err(Error::InvalidLlmOutput(_))
        ));
    }

    #[test]
    fn rejects_summary_without_key_points() {
        assert!(matches!(
            parse_summary(
                r#"{"short":"x","detailed":"y","key_points":[]}"#,
                "m"
            ),
            Err(Error::InvalidLlmOutput(_))
        ));
    }

    #[test]
    fn parses_valid_keywords() {
        let keywords = parse_keywords(r#"{"keywords":"diffusion models image generation"}"#)
            .expect("keywords should parse");
        assert_eq!(keywords, "diffusion models image generation");
    }

    #[test]
    fn rejects_chinese_keywords() {
        assert!(matches!(
            parse_keywords(r#"{"keywords":"扩散模型"}"#),
            Err(Error::InvalidLlmOutput(_))
        ));
    }

    #[test]
    fn rejects_empty_chat_content() {
        let body = r#"{"choices":[{"message":{"content":""}}]}"#;
        assert!(matches!(
            parse_chat_content(body),
            Err(Error::InvalidLlmOutput(_))
        ));
    }

    #[test]
    fn strips_minimax_thinking_blocks() {
        let content =
            "<think>reasoning\nhere</think>\n{\"short\":\"x\",\"detailed\":\"y\",\"key_points\":[\"z\"]}";

        assert_eq!(
            strip_think_block(content),
            r#"{"short":"x","detailed":"y","key_points":["z"]}"#
        );
    }
}
