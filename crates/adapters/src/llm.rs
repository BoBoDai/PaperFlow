//! LLM Provider.
//!
//! The client speaks the OpenAI-compatible chat-completions protocol, so it
//! works with DeepSeek by default and with any other compatible endpoint.
//!
//! ## The most important rule
//!
//! **LLM output must pass structural validation before it is returned.**
//!
//! Never fall back to "parse failed, use the raw text" — that is the worst
//! failure mode: the user sees garbage and the logs stay quiet. Validation
//! failures return [`paperflow_core::Error::InvalidLlmOutput`] so the caller can
//! retry or surface the error.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use paperflow_core::{contains_cjk, Error, Paper, Result, Summary};
use serde::{Deserialize, Serialize};

use crate::search::describe_error;

/// LLM capabilities.
#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    /// Whether the provider has enough configuration to make a request.
    fn is_configured(&self) -> bool;

    /// Analyze a paper and produce a structured summary.
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;

    /// Translate a Chinese query into English keywords.
    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String>;
}

/// Runtime-editable LLM settings.
///
/// The defaults target DeepSeek. `base_url` may point at any OpenAI-compatible
/// endpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.deepseek.com".into(),
            model: "deepseek-chat".into(),
        }
    }
}

impl LlmConfig {
    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

/// Client for OpenAI-compatible `/chat/completions` endpoints.
///
/// Settings are shared with the settings page: every request reads the current
/// config, so saving in the UI takes effect without restarting the app.
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
                "请先在设置中填写 DeepSeek API key".into(),
            ));
        }

        let base_url = config.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            return Err(Error::Config("LLM Base URL 不能为空".into()));
        }

        let model = config.model.trim();
        if model.is_empty() {
            return Err(Error::Config("LLM 模型名不能为空".into()));
        }

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": 0.2,
            "max_tokens": max_tokens,
            "response_format": { "type": "json_object" },
            "stream": false,
        });

        let response = self
            .http
            .post(format!("{base_url}/chat/completions"))
            .bearer_auth(config.api_key.trim())
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Llm(describe_error("LLM request failed", &e)))?;

        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            let detail = detail.trim();
            let message = match status {
                reqwest::StatusCode::UNAUTHORIZED => {
                    "DeepSeek 鉴权失败（401），请检查 API key".to_string()
                }
                reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    "LLM 限流（HTTP 429），请稍后重试".to_string()
                }
                _ => format!("LLM 返回 HTTP {status}"),
            };

            return Err(Error::Llm(if detail.is_empty() {
                message
            } else {
                format!("{message}: {detail}")
            }));
        }

        let body = response
            .text()
            .await
            .map_err(|e| Error::Llm(describe_error("failed to read LLM response", &e)))?;

        parse_chat_content(&body)
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
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| Error::InvalidLlmOutput("LLM returned an empty response".into()))
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
    fn default_config_targets_deepseek() {
        let config = LlmConfig::default();
        assert_eq!(config.base_url, "https://api.deepseek.com");
        assert_eq!(config.model, "deepseek-chat");
        assert!(!config.is_configured());
    }
}
