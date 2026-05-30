//! OpenAI-compatible LLM Provider
//!
//! 兼容 OpenAI API 格式，适用于任何 OpenAI 兼容服务（包括国内代理）

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::{Error, Paper, Summary, Result};
use crate::modules::llm::{LlmProvider, LlmConfig};
use crate::modules::llm::prompt::PromptManager;

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

pub struct OpenAiProvider {
    client: Client,
    config: LlmConfig,
}

impl OpenAiProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: Client::builder().no_proxy().build().unwrap_or_else(|_| Client::new()),
            config,
        }
    }

    async fn chat(&self, prompt: &str) -> Result<String> {
        let base_url = self.config.base_url.as_deref()
            .unwrap_or("https://api.openai.com");

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature: Some(0.3),
        };

        // Build URL — if base_url already includes /v1, don't duplicate
        let base = base_url.trim_end_matches('/');
        let url = if base.ends_with("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("OpenAI request failed: {}", e)))?;

        let status = response.status();
        let resp_text = response.text().await
            .map_err(|e| Error::Llm(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(Error::Llm(format!("OpenAI API error ({}): {}", status.as_u16(),
                &resp_text[..resp_text.len().min(300)])));
        }

        let body: ChatResponse = serde_json::from_str(&resp_text)
            .map_err(|e| Error::Llm(format!("Failed to parse OpenAI response: {} — body: {}",
                e, &resp_text[..resp_text.len().min(200)])))?;

        body.choices.first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| Error::Llm("No response from OpenAI".to_string()))
    }

    fn parse_summary_response(&self, content: &str, paper_id: &str) -> Result<Summary> {
        let json_str = if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                &content[start..=end]
            } else { content }
        } else { content };

        #[derive(Deserialize)]
        struct JsonSummary {
            short_summary: Option<String>,
            detailed_summary: Option<String>,
            key_points: Option<Vec<String>>,
        }

        let json: JsonSummary = serde_json::from_str(json_str)
            .map_err(|e| Error::Parse(format!("Failed to parse summary JSON: {}", e)))?;

        Ok(Summary {
            id: uuid_simple(),
            paper_id: paper_id.to_string(),
            short_summary: json.short_summary.unwrap_or_default(),
            detailed_summary: json.detailed_summary.unwrap_or_default(),
            key_points: json.key_points.unwrap_or_default(),
            generated_at: Utc::now(),
            provider: "openai".to_string(),
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        let prompt = PromptManager::summarize_prompt(paper);
        let content = self.chat(&prompt).await?;
        self.parse_summary_response(&content, &paper.id)
    }

    async fn verbalize(&self, summary: &Summary) -> Result<String> {
        let prompt = PromptManager::verbalize_prompt(&summary.detailed_summary);
        self.chat(&prompt).await
    }

    async fn score_relevance(&self, paper: &Paper, interests: &[String]) -> Result<f64> {
        let prompt = PromptManager::relevance_prompt(paper, interests);
        let content = self.chat(&prompt).await?;
        let score: f64 = content.trim().parse()
            .map_err(|_| Error::Parse(format!("Failed to parse score: {}", content)))?;
        Ok(score.min(10.0).max(0.0))
    }

    fn name(&self) -> &str { "openai" }

    async fn complete(&self, prompt: &str) -> Result<String> {
        self.chat(prompt).await
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("{:x}", now)
}
