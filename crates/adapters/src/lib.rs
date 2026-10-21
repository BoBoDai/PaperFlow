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
pub mod openalex;
pub mod pdf;
pub mod search;
pub mod storage;

pub use arxiv::ArxivHttpClient;
pub use pdf::PdfFetcher;
pub use llm::{
    base_url_host, normalize_base_url, provider_catalog, ConnectionTestResult,
    EffectiveCapabilities, LlmCompat, LlmConfig, LlmEndpointError, LlmErrorKind, LlmModelList,
    LlmProvider, MaxTokensField, OpenAiCompatibleClient, ProviderProfileId,
    ProviderProfileInfo, ThinkingFormat, ThinkingMode,
};
pub use openalex::OpenAlexHttpClient;
pub use search::PaperSearch;
pub use storage::PaperStore;
