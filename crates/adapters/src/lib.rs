//! # paperflow-adapters
//!
//! 适配层。每个外部依赖（arXiv、LLM、数据库）在这里包成一个 trait，
//! 换实现不影响上层。
//!
//! ## 纪律
//!
//! - 每个适配器只做「翻译」：外部格式 ↔ 领域类型
//! - 业务规则不写在这里，写 `paperflow-core`
//! - 不许把外部类型（reqwest::Response、sqlx::Row）泄漏到 trait 签名上

pub mod arxiv;
pub mod llm;
pub mod storage;

pub use arxiv::{ArxivClient, StubArxivClient};
pub use llm::{LlmProvider, StubLlmProvider};
pub use storage::PaperStore;
