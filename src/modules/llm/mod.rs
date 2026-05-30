//! LLM 模块

mod trait_;
mod prompt;
pub mod providers;

pub use trait_::{LlmProvider, LlmConfig};
pub use prompt::PromptManager;
pub use providers::{MiniMaxProvider, OpenAiProvider};
