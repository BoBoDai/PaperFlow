//! LLM provider support.
//!
//! The adapter owns the OpenAI-compatible HTTP protocol, provider profiles,
//! prompt construction, and strict parsing of model output. Business rules
//! (budgets, fallbacks) stay in `paperflow-usecases`.

mod client;
mod config;
mod error;
mod parsing;
mod profiles;
mod prompts;
mod streaming;

#[cfg(test)]
mod tests;

use paperflow_core::{Answer, Paper, QaRequest, Result, Summary};

pub use client::OpenAiCompatibleClient;
pub use config::{LlmConfig, base_url_host, normalize_base_url};
pub use error::{ConnectionTestResult, LlmEndpointError, LlmErrorKind, LlmModelList};
pub use profiles::{
    EffectiveCapabilities, LlmCompat, MaxTokensField, ProviderDefaults, ProviderProfileId,
    ProviderProfileInfo, ThinkingFormat, ThinkingMode, provider_catalog,
};

// Re-export internals for the unit tests in `llm/tests.rs`.
#[cfg(test)]
pub(crate) use client::build_chat_body;
#[cfg(test)]
pub(crate) use error::map_http_error;
#[cfg(test)]
pub(crate) use parsing::{
    parse_answer, parse_chat_content, parse_keywords, parse_models_response, parse_summary_text,
    strip_think_block,
};
#[cfg(test)]
pub(crate) use prompts::build_qa_messages;
#[cfg(test)]
pub(crate) use streaming::{AnswerStream, next_sse_event};

/// The LLM operations the application needs.
///
/// Implementations must validate model output structurally and return
/// `paperflow_core::Error::InvalidLlmOutput` instead of silently falling back.
#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    fn is_configured(&self) -> bool;
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;
    async fn summarize_stream(
        &self,
        paper: &Paper,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Summary>;
    async fn to_english_keywords(&self, chinese_query: &str) -> Result<String>;
    async fn ask_paper(&self, request: &QaRequest) -> Result<Answer>;
    async fn ask_paper_stream(
        &self,
        request: &QaRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Answer>;
}
