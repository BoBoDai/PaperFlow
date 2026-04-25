//! MiniMax LLM Provider

use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::{Error, Paper, Summary, Result};
use crate::modules::llm::{LlmProvider, LlmConfig};
use crate::modules::llm::prompt::PromptManager;

/// MiniMax API 响应
#[derive(Debug, Deserialize)]
struct MiniMaxResponse {
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

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// MiniMax LLM Provider 实现
pub struct MiniMaxProvider {
    client: Client,
    config: LlmConfig,
}

impl MiniMaxProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// 调用 MiniMax API
    async fn chat(&self, prompt: &str) -> Result<String> {
        let base_url = self.config.base_url.as_deref()
            .unwrap_or("https://api.minimax.chat/v1");

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self.client
            .post(format!("{}/text/chatcompletion_pro", base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Request failed: {}", e)))?;

        let body: MiniMaxResponse = response.json().await
            .map_err(|e| Error::Llm(format!("Failed to parse response: {}", e)))?;

        body.choices.first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| Error::Llm("No response from LLM".to_string()))
    }

    /// 解析 JSON 摘要响应
    fn parse_summary_response(&self, content: &str, paper_id: &str) -> Result<Summary> {
        // 尝试提取 JSON
        let json_str = if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                &content[start..=end]
            } else {
                content
            }
        } else {
            content
        };

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
            provider: "minimax".to_string(),
        })
    }
}

#[async_trait]
impl LlmProvider for MiniMaxProvider {
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

        // 提取数字
        let score: f64 = content.trim()
            .parse()
            .map_err(|_| Error::Parse(format!("Failed to parse score: {}", content)))?;

        Ok(score.min(10.0).max(0.0))
    }

    fn name(&self) -> &str {
        "minimax"
    }

    async fn complete(&self, prompt: &str) -> Result<String> {
        self.chat(prompt).await
    }
}

/// 生成简单 UUID
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", now)
}
