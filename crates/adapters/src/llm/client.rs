//! OpenAI-compatible HTTP client.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use paperflow_core::{Answer, Error, Paper, QaRequest, Result, Summary};

use super::LlmProvider;
use super::config::{LlmConfig, normalize_base_url};
use super::error::{
    ConnectionTestResult, LlmEndpointError, LlmErrorKind, LlmModelList, map_http_error,
    request_id_from_headers, transport_error,
};
use super::parsing::{
    parse_answer, parse_chat_content, parse_keywords, parse_models_response, parse_summary_text,
};
use super::profiles::{ThinkingFormat, ThinkingMode};
use super::prompts::{KEYWORDS_SYSTEM_PROMPT, build_qa_messages, summary_messages};
use super::streaming::{AnswerStream, StreamChunk, next_sse_event};

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
        json_mode: bool,
    ) -> Result<String> {
        if !config.is_configured() {
            return Err(Error::Config("请先在设置中填写 API key".into()));
        }

        let base_url = normalize_base_url(&config.base_url).map_err(Error::Config)?;
        if config.model.trim().is_empty() {
            return Err(Error::Config("LLM 模型名不能为空".into()));
        }

        let body = build_chat_body(config, messages, max_tokens, json_mode, false);

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

        let body = response.text().await.map_err(|e| {
            Error::Llm(transport_error("failed to read LLM response", e).to_string())
        })?;

        parse_chat_content(&body)
    }

    /// Fetch model IDs from `{base_url}/models`.
    pub async fn fetch_models(
        &self,
        config: &LlmConfig,
    ) -> std::result::Result<LlmModelList, LlmEndpointError> {
        if !config.is_configured() {
            return Err(LlmEndpointError::missing("请先填写或保存 API Key"));
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
            return Err(LlmEndpointError::missing("请先填写或保存 API Key"));
        }

        let base_url = normalize_base_url(&config.base_url).map_err(LlmEndpointError::missing)?;
        if config.model.trim().is_empty() {
            return Err(LlmEndpointError::missing("请填写模型名"));
        }

        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "ping",
        })];
        let body = build_chat_body(config, messages, 16, false, false);

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

    /// Send a streaming chat request and return the accumulated answer text.
    async fn stream_chat(
        &self,
        config: &LlmConfig,
        messages: Vec<serde_json::Value>,
        max_tokens: u32,
        timeout: Duration,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<String> {
        if !config.is_configured() {
            return Err(Error::Config("请先在设置中填写 API key".into()));
        }
        let base_url = normalize_base_url(&config.base_url).map_err(Error::Config)?;
        if config.model.trim().is_empty() {
            return Err(Error::Config("LLM 模型名不能为空".into()));
        }

        let body = build_chat_body(config, messages, max_tokens, false, true);

        let response = self
            .http
            .post(format!("{base_url}/chat/completions"))
            .bearer_auth(config.api_key.trim())
            .timeout(timeout)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Llm(transport_error("LLM stream failed", e).to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(
                map_http_error(status, &headers, &body).to_string(),
            ));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut full_body = String::new();
        let mut answer_stream = AnswerStream::default();
        let mut saw_event = false;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                Error::Llm(transport_error("failed to read LLM stream", e).to_string())
            })?;
            let text = String::from_utf8_lossy(&chunk);
            full_body.push_str(&text);
            buffer.push_str(&text);

            while let Some((index, length)) = next_sse_event(&buffer) {
                let event: String = buffer.drain(..index + length).collect();

                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data.is_empty() {
                        continue;
                    }

                    saw_event = true;
                    if data == "[DONE]" {
                        continue;
                    }

                    match serde_json::from_str::<StreamChunk>(data) {
                        Ok(parsed) => {
                            if let Some(content) = parsed
                                .choices
                                .into_iter()
                                .next()
                                .and_then(|choice| choice.delta.content)
                            {
                                let out = answer_stream.push(&content);
                                if !out.is_empty() {
                                    on_delta(out);
                                }
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%error, data, "ignoring malformed SSE chunk");
                        }
                    }
                }
            }
        }

        if !saw_event {
            // Some gateways ignore stream=true and return a normal completion.
            let content = parse_chat_content(&full_body)?;
            on_delta(content.clone());
            return Ok(content);
        }

        let answer = answer_stream.finish();
        if answer.is_empty() {
            return Err(Error::InvalidLlmOutput(
                "LLM returned an empty response".into(),
            ));
        }

        Ok(answer)
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

        let content = self
            .chat(
                &config,
                summary_messages(paper),
                4096,
                Duration::from_secs(120),
                false,
            )
            .await?;

        parse_summary_text(&content, config.model.trim())
    }

    async fn summarize_stream(
        &self,
        paper: &Paper,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Summary> {
        let config = self.current_config()?;
        tracing::info!(id = %paper.id, model = %config.model, "streaming paper summary");

        let content = self
            .stream_chat(
                &config,
                summary_messages(paper),
                4096,
                Duration::from_secs(300),
                on_delta,
            )
            .await?;

        parse_summary_text(&content, config.model.trim())
    }

    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String> {
        let config = self.current_config()?;
        tracing::info!(query = chinese_query, model = %config.model, "translating query");

        let messages = vec![
            serde_json::json!({ "role": "system", "content": KEYWORDS_SYSTEM_PROMPT }),
            serde_json::json!({ "role": "user", "content": chinese_query }),
        ];

        let content = self
            .chat(&config, messages, 512, Duration::from_secs(45), true)
            .await?;

        parse_keywords(&content)
    }

    async fn ask_paper(&self, request: &QaRequest) -> Result<Answer> {
        let config = self.current_config()?;
        tracing::info!(
            id = %request.paper.id,
            scope = ?request.scope,
            model = %config.model,
            "answering paper question"
        );

        let messages = build_qa_messages(request);
        let content = self
            .chat(&config, messages, 4096, Duration::from_secs(180), false)
            .await?;

        parse_answer(&content, config.model.trim())
    }

    async fn ask_paper_stream(
        &self,
        request: &QaRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Answer> {
        let config = self.current_config()?;
        tracing::info!(
            id = %request.paper.id,
            scope = ?request.scope,
            model = %config.model,
            "streaming paper answer"
        );

        let content = self
            .stream_chat(
                &config,
                build_qa_messages(request),
                4096,
                Duration::from_secs(300),
                on_delta,
            )
            .await?;

        parse_answer(&content, config.model.trim())
    }
}

pub(crate) fn build_chat_body(
    config: &LlmConfig,
    messages: Vec<serde_json::Value>,
    max_tokens: u32,
    json_mode: bool,
    stream: bool,
) -> serde_json::Value {
    let capabilities = config.capabilities();

    let mut body = serde_json::json!({
        "model": config.model.trim(),
        "messages": messages,
        "temperature": 0.2,
        "stream": stream,
    });

    body[capabilities.max_tokens_field.as_str()] = serde_json::json!(max_tokens);

    if json_mode && capabilities.json_mode {
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
