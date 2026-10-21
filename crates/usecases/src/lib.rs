//! # paperflow-usecases
//!
//! 用例层。**传输无关** —— 这个 crate 里不许出现 `tauri::` 或 `axum::`。
//!
//! 这是整套架构的枢纽：Tauri command、未来的 HTTP handler、CLI，
//! 都只是这里几个函数的薄壳（各 5 行）。所以想加一个新的传输方式，
//! 成本是 5 行，不是重构。
//!
//! ```ignore
//! // Tauri
//! #[tauri::command]
//! async fn search_papers(...) -> ... { state.app.search_papers(q, n).await }
//!
//! // Axum（将来给 Web/TUI 用）
//! async fn search(Query(q): Query<Q>) -> Json<...> { app.search_papers(q.q, n).await }
//! ```

mod feed;
mod qa;
mod search;

#[cfg(test)]
mod tests;

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Paper, Result, Summary};

/// Default page size for the interest feed.
pub const FEED_DEFAULT_LIMIT: usize = 12;
/// Upper bound accepted from IPC callers.
pub const FEED_MAX_LIMIT: usize = 50;
/// Keep the generated source query short enough for a GET URL.
pub const MAX_FEED_INTERESTS: usize = 8;

/// Character budget for the full-paper Q&A context.
pub const QA_CONTEXT_BUDGET: usize = 40_000;
pub const QA_SELECTION_BUDGET: usize = 8_000;
pub const QA_QUESTION_BUDGET: usize = 2_000;
pub const QA_HISTORY_TURNS: usize = 6;
pub const QA_HISTORY_BUDGET: usize = 12_000;

/// 应用上下文：所有适配器在这里注入。
///
/// 用泛型而非 `dyn` —— 编译期单态化，且避免了 `dyn` + `async fn` 的对象安全问题。
pub struct App<A, L, O> {
    pub arxiv: A,
    pub llm: L,
    pub openalex: O,
}

impl<A: PaperSearch, L: LlmProvider, O: PaperSearch> App<A, L, O> {
    pub fn new(arxiv: A, llm: L, openalex: O) -> Self {
        Self {
            arxiv,
            llm,
            openalex,
        }
    }

    /// 分析论文
    pub async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        self.llm.summarize(paper).await
    }

    /// 流式论文分析：增量通过 `on_delta` 推送，最后返回结构化摘要。
    pub async fn summarize_stream<F>(&self, paper: &Paper, mut on_delta: F) -> Result<Summary>
    where
        F: FnMut(String) + Send,
    {
        self.llm.summarize_stream(paper, &mut on_delta).await
    }
}

/// 跨源去重：arXiv 和 OpenAlex 对同一篇论文的 id 不同，按归一化标题去重。
///
/// 纯函数，无 IO —— 这是 `core`/`usecases` 分层的价值所在：可以直接单测。
pub fn dedupe(papers: Vec<Paper>) -> Vec<Paper> {
    let mut seen = std::collections::HashSet::new();
    papers
        .into_iter()
        .filter(|p| seen.insert(p.dedup_key()))
        .collect()
}
