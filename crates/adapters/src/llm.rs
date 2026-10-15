//! LLM Provider
//!
//! 支持任何 OpenAI 兼容端点（MiniMax / DeepSeek / OpenAI）。
//!
//! ## 最重要的一条纪律
//!
//! **LLM 的输出必须过结构校验才能返回。**
//!
//! 绝不允许「解析失败就把原文当结果返回」这种兜底 —— 那是最糟的失败模式：
//! 用户看到垃圾，日志里一片安静。校验不过就返回
//! [`paperflow_core::Error::InvalidLlmOutput`]，让调用方决定重试还是报错。

use paperflow_core::{Paper, Result, Summary};

/// LLM 能力
#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    /// 分析论文，产出结构化摘要
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;

    /// 中文查询 → 英文关键词
    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String>;
}

/// 占位实现。接真实 provider 前，用它跑通链路。
pub struct StubLlmProvider;

impl LlmProvider for StubLlmProvider {
    async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        tracing::info!(id = %paper.id, "StubLlmProvider: 返回占位摘要");
        Ok(Summary {
            short: format!("（占位）{} 的一句话概括。", paper.title),
            detailed: "（占位）这里将是 LLM 生成的详细分析。".into(),
            key_points: vec!["（占位）要点一".into(), "（占位）要点二".into()],
            model: "stub".into(),
            prompt_version: Summary::PROMPT_VERSION.into(),
        })
    }

    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String> {
        tracing::info!(query = chinese_query, "StubLlmProvider: 直接返回原文");
        Ok(chinese_query.to_string())
    }
}
