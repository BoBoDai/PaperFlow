//! LLM error types and HTTP error mapping.

use serde::{Deserialize, Serialize};

use crate::search::describe_error;

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
    pub(crate) fn missing(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::MissingConfig,
            message: message.into(),
            status: None,
            request_id: None,
            provider_message: None,
        }
    }

    pub(crate) fn invalid_response(message: impl Into<String>) -> Self {
        Self {
            kind: LlmErrorKind::InvalidResponse,
            message: message.into(),
            status: None,
            request_id: None,
            provider_message: None,
        }
    }
}

pub(crate) fn truncate(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_string();
    }

    let truncated: String = trimmed.chars().take(limit).collect();
    format!("{truncated}…")
}

pub(crate) fn request_id_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
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

pub(crate) fn map_http_error(
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
            400 => (LlmErrorKind::BadRequest, "请求参数无效".to_string()),
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
            500..=599 => (LlmErrorKind::Server, "服务端错误，请稍后重试".to_string()),
            _ => (LlmErrorKind::BadRequest, format!("LLM 返回 HTTP {status}")),
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

pub(crate) fn transport_error(context: &str, error: reqwest::Error) -> LlmEndpointError {
    let (kind, message) = if error.is_timeout() {
        (
            LlmErrorKind::Timeout,
            "请求超时，请检查网络或代理".to_string(),
        )
    } else {
        (LlmErrorKind::Network, describe_error(context, &error))
    };

    LlmEndpointError {
        kind,
        message,
        status: None,
        request_id: None,
        provider_message: None,
    }
}
