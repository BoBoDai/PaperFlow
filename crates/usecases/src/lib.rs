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

use paperflow_adapters::{ArxivClient, LlmProvider};
use paperflow_core::{Paper, Result, Summary};

/// 应用上下文：所有适配器在这里注入。
///
/// 用泛型而非 `dyn` —— 编译期单态化，且避免了 `dyn` + `async fn` 的对象安全问题。
pub struct App<A, L> {
    pub arxiv: A,
    pub llm: L,
}

impl<A: ArxivClient, L: LlmProvider> App<A, L> {
    pub fn new(arxiv: A, llm: L) -> Self {
        Self { arxiv, llm }
    }

    /// 检索论文，并做跨源去重
    pub async fn search_papers(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        let papers = self.arxiv.search(query, limit).await?;
        Ok(dedupe(papers))
    }

    /// 分析论文
    pub async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        self.llm.summarize(paper).await
    }
}

/// 跨源去重：arXiv 和 Semantic Scholar 对同一篇论文的 id 不同，按归一化标题去重。
///
/// 纯函数，无 IO —— 这是 `core`/`usecases` 分层的价值所在：可以直接单测。
pub fn dedupe(papers: Vec<Paper>) -> Vec<Paper> {
    let mut seen = std::collections::HashSet::new();
    papers
        .into_iter()
        .filter(|p| seen.insert(p.dedup_key()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use paperflow_core::Source;

    fn paper(id: &str, title: &str) -> Paper {
        Paper {
            id: id.into(),
            title: title.into(),
            authors: vec![],
            abstract_text: String::new(),
            categories: vec![],
            published: Utc::now(),
            pdf_url: String::new(),
            source: Source::Arxiv,
            venue: None,
        }
    }

    #[test]
    fn dedupe_removes_cross_source_duplicates() {
        let input = vec![
            paper("arxiv:1706.03762", "Attention Is All You Need"),
            paper("s2:abc123", "attention is all you need!"),
            paper("arxiv:1512.03385", "Deep Residual Learning"),
        ];
        let out = dedupe(input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "arxiv:1706.03762");
    }
}
