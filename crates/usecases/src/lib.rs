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

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Error, Paper, Result, Summary};

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

    /// 检索论文，并做跨源去重。
    ///
    /// arXiv 是主源；它失败或没有结果时回退到 OpenAlex。两个源都失败才
    /// 返回 [`Error::Sources`]，绝不假装成功返回空列表。
    pub async fn search_papers(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        match self.arxiv.search(query, limit).await {
            Ok(papers) if !papers.is_empty() => Ok(dedupe(papers)),
            Ok(_) => {
                tracing::info!(query, "arXiv returned no results; falling back to OpenAlex");

                match self.openalex.search(query, limit).await {
                    Ok(papers) => Ok(dedupe(papers)),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            query,
                            "OpenAlex fallback failed after an empty arXiv result"
                        );
                        Ok(Vec::new())
                    }
                }
            }
            Err(arxiv_error) => {
                tracing::warn!(
                    error = %arxiv_error,
                    query,
                    "arXiv failed; falling back to OpenAlex"
                );

                self.openalex
                    .search(query, limit)
                    .await
                    .map(dedupe)
                    .map_err(|openalex_error| {
                        Error::Sources(format!(
                            "arXiv: {arxiv_error}; OpenAlex: {openalex_error}"
                        ))
                    })
            }
        }
    }

    /// 分析论文
    pub async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        self.llm.summarize(paper).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::Utc;
    use paperflow_adapters::{ArxivHttpClient, OpenAlexHttpClient, StubLlmProvider};
    use paperflow_core::Source;

    fn paper(id: &str, title: &str, source: Source) -> Paper {
        Paper {
            id: id.into(),
            title: title.into(),
            authors: vec![],
            abstract_text: String::new(),
            categories: vec![],
            published: Utc::now(),
            pdf_url: String::new(),
            source,
            venue: None,
        }
    }

    struct FakeSearch {
        papers: Vec<Paper>,
        fail: bool,
        calls: AtomicUsize,
    }

    impl FakeSearch {
        fn ok(papers: Vec<Paper>) -> Self {
            Self {
                papers,
                fail: false,
                calls: AtomicUsize::new(0),
            }
        }

        fn failing() -> Self {
            Self {
                papers: Vec::new(),
                fail: true,
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl PaperSearch for FakeSearch {
        async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<Paper>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(Error::Network("source unavailable".into()))
            } else {
                Ok(self.papers.clone())
            }
        }
    }

    fn app(
        arxiv: FakeSearch,
        openalex: FakeSearch,
    ) -> App<FakeSearch, StubLlmProvider, FakeSearch> {
        App::new(arxiv, StubLlmProvider, openalex)
    }

    #[test]
    fn dedupe_removes_cross_source_duplicates() {
        let input = vec![
            paper("arxiv:1706.03762", "Attention Is All You Need", Source::Arxiv),
            paper(
                "openalex:W1",
                "attention is all you need!",
                Source::OpenAlex,
            ),
            paper("arxiv:1512.03385", "Deep Residual Learning", Source::Arxiv),
        ];
        let out = dedupe(input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "arxiv:1706.03762");
    }

    #[tokio::test]
    async fn falls_back_to_openalex_when_arxiv_fails() {
        let app = app(
            FakeSearch::failing(),
            FakeSearch::ok(vec![paper("W1", "Fallback Paper", Source::OpenAlex)]),
        );

        let papers = app
            .search_papers("diffusion", 10)
            .await
            .expect("fallback should succeed");

        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].source, Source::OpenAlex);
        assert_eq!(app.arxiv.calls(), 1);
        assert_eq!(app.openalex.calls(), 1);
    }

    #[tokio::test]
    async fn falls_back_when_arxiv_is_empty() {
        let app = app(
            FakeSearch::ok(Vec::new()),
            FakeSearch::ok(vec![paper("W2", "OpenAlex Only", Source::OpenAlex)]),
        );

        let papers = app
            .search_papers("rare query", 10)
            .await
            .expect("fallback should succeed");

        assert_eq!(papers.len(), 1);
        assert_eq!(app.openalex.calls(), 1);
    }

    #[tokio::test]
    async fn keeps_arxiv_results_without_calling_openalex() {
        let app = app(
            FakeSearch::ok(vec![paper("2401.1", "Primary Paper", Source::Arxiv)]),
            FakeSearch::ok(vec![paper("W3", "Duplicate Title", Source::OpenAlex)]),
        );

        let papers = app
            .search_papers("query", 10)
            .await
            .expect("primary source should succeed");

        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].source, Source::Arxiv);
        assert_eq!(app.openalex.calls(), 0);
    }

    #[tokio::test]
    async fn reports_error_when_all_sources_fail() {
        let app = app(FakeSearch::failing(), FakeSearch::failing());

        let error = app
            .search_papers("query", 10)
            .await
            .expect_err("both sources failed");

        assert!(matches!(
            error,
            Error::Sources(message)
                if message.contains("arXiv") && message.contains("OpenAlex")
        ));
    }

    #[tokio::test]
    #[ignore = "hits the live arXiv and OpenAlex APIs"]
    async fn live_search_falls_back_to_a_working_source() {
        let arxiv = ArxivHttpClient::new().expect("arXiv client should build");
        let openalex = OpenAlexHttpClient::new().expect("OpenAlex client should build");
        let app = App::new(arxiv, StubLlmProvider, openalex);

        let papers = app
            .search_papers("diffusion models", 3)
            .await
            .expect("at least one source should work");

        assert!(!papers.is_empty());
        assert!(
            papers
                .iter()
                .all(|p| matches!(p.source, Source::Arxiv | Source::OpenAlex))
        );
    }
}
