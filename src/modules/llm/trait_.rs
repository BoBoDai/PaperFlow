//! LLM Provider trait 定义

use async_trait::async_trait;
use crate::core::{Error, Paper, Summary, Result};

/// LLM Provider 配置
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider_type: String,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

/// LLM Provider trait（可插拔设计）
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 生成论文摘要
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;

    /// 将摘要转换为口语化表达
    async fn verbalize(&self, summary: &Summary) -> Result<String>;

    /// 评估论文相关性
    async fn score_relevance(&self, paper: &Paper, interests: &[String]) -> Result<f64>;

    /// 获取 Provider 名称
    fn name(&self) -> &str;

    /// 通用文本补全（用于翻译等场景）
    async fn complete(&self, prompt: &str) -> Result<String>;
}
