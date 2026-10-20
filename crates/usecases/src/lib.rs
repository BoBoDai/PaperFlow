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
use paperflow_core::{
    contains_cjk, translate_query_locally, Error, FeedPage, Paper, Result, SearchResponse,
    Summary,
};

/// Default page size for the interest feed.
pub const FEED_DEFAULT_LIMIT: usize = 12;
/// Upper bound accepted from IPC callers.
pub const FEED_MAX_LIMIT: usize = 50;
/// Keep the generated source query short enough for a GET URL.
pub const MAX_FEED_INTERESTS: usize = 8;

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
    /// 中文查询先过本地学术术语表；术语表覆盖不了且 LLM 已配置时才调用
    /// LLM。两者都不可用时明确报错，不静默拿中文原文检索。
    /// arXiv 是主源；它失败或没有结果时回退到 OpenAlex。两个源都失败才
    /// 返回 [`Error::Sources`]，绝不假装成功返回空列表。
    ///
    /// 返回值包含实际发送给数据源的英文查询，前端可以展示给用户。
    pub async fn search_papers(&self, query: &str, limit: usize) -> Result<SearchResponse> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(SearchResponse {
                papers: Vec::new(),
                query: String::new(),
            });
        }

        let search_query = self.english_query(query).await?;

        let papers = match self.arxiv.search(&search_query, limit).await {
            Ok(papers) if !papers.is_empty() => dedupe(papers),
            Ok(_) => {
                tracing::info!(
                    query = %search_query,
                    "arXiv returned no results; falling back to OpenAlex"
                );

                match self.openalex.search(&search_query, limit).await {
                    Ok(papers) => dedupe(papers),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            query = %search_query,
                            "OpenAlex fallback failed after an empty arXiv result"
                        );
                        Vec::new()
                    }
                }
            }
            Err(arxiv_error) => {
                tracing::warn!(
                    error = %arxiv_error,
                    query = %search_query,
                    "arXiv failed; falling back to OpenAlex"
                );

                match self.openalex.search(&search_query, limit).await {
                    Ok(papers) => dedupe(papers),
                    Err(openalex_error) => {
                        return Err(Error::Sources(format!(
                            "arXiv: {arxiv_error}; OpenAlex: {openalex_error}"
                        )));
                    }
                }
            }
        };

        Ok(SearchResponse {
            papers,
            query: search_query,
        })
    }

    /// Translate a Chinese query to English, preferring the local glossary so
    /// common academic searches work before an API key is configured.
    async fn english_query(&self, query: &str) -> Result<String> {
        if !contains_cjk(query) {
            return Ok(query.to_string());
        }

        let local = translate_query_locally(query);
        if !local.is_empty() && !contains_cjk(&local) {
            tracing::info!(original = query, translated = %local, "translated query locally");
            return Ok(local);
        }

        if self.llm.is_configured() {
            let translated = self.llm.to_english_keywords(query).await?;
            tracing::info!(original = query, translated = %translated, "translated query with LLM");
            Ok(translated)
        } else {
            Err(Error::Config(
                "无法自动翻译该中文查询：请在设置中配置 LLM API key，或改用英文关键词".into(),
            ))
        }
    }

    /// 兴趣信息流：按最新发表时间返回一页论文。
    ///
    /// arXiv 是主源；首页为空或失败时回退 OpenAlex。`has_more` 在去重前
    /// 根据原始页大小计算，避免去重后误判「到底了」。
    pub async fn browse_feed(
        &self,
        interests: &[String],
        offset: usize,
        limit: usize,
    ) -> Result<FeedPage> {
        let interests = normalize_interests(interests);
        if interests.is_empty() {
            return Ok(FeedPage {
                papers: Vec::new(),
                has_more: false,
            });
        }

        let interests = self.translate_interests(&interests).await?;
        let limit = limit.clamp(1, FEED_MAX_LIMIT);
        let offset = (offset / limit) * limit;

        let page = match self.arxiv.feed(&interests, offset, limit).await {
            Ok(papers) if !papers.is_empty() => feed_page(papers, limit),
            // An empty page past the first one means the feed is exhausted;
            // do not restart another source at an arbitrary offset.
            Ok(_) if offset > 0 => FeedPage {
                papers: Vec::new(),
                has_more: false,
            },
            Ok(_) => match self.openalex.feed(&interests, offset, limit).await {
                Ok(papers) => feed_page(papers, limit),
                Err(openalex_error) => {
                    return Err(Error::Sources(format!(
                        "arXiv returned no results; OpenAlex: {openalex_error}"
                    )));
                }
            },
            Err(arxiv_error) => {
                tracing::warn!(
                    error = %arxiv_error,
                    offset,
                    "arXiv feed failed; falling back to OpenAlex"
                );

                match self.openalex.feed(&interests, offset, limit).await {
                    Ok(papers) => feed_page(papers, limit),
                    Err(openalex_error) => {
                        return Err(Error::Sources(format!(
                            "arXiv: {arxiv_error}; OpenAlex: {openalex_error}"
                        )));
                    }
                }
            }
        };

        tracing::debug!(
            papers = page.papers.len(),
            has_more = page.has_more,
            "feed page ready"
        );

        Ok(page)
    }

    async fn translate_interests(&self, interests: &[String]) -> Result<Vec<String>> {
        let mut translated = Vec::with_capacity(interests.len());

        for interest in interests {
            translated.push(self.english_query(interest).await?);
        }

        Ok(translated)
    }

    /// 分析论文
    pub async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        self.llm.summarize(paper).await
    }
}

fn normalize_interests(interests: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();

    interests
        .iter()
        .map(|interest| interest.trim())
        .filter(|interest| !interest.is_empty())
        .filter(|interest| seen.insert(interest.to_lowercase()))
        .take(MAX_FEED_INTERESTS)
        .map(|interest| interest.to_string())
        .collect()
}

fn feed_page(raw: Vec<Paper>, limit: usize) -> FeedPage {
    // Compute before dedupe so duplicate titles do not make a full page look
    // like the end of the feed.
    let has_more = raw.len() >= limit;
    FeedPage {
        papers: dedupe(raw),
        has_more,
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
    use std::sync::Mutex;

    use chrono::Utc;
    use paperflow_adapters::{ArxivHttpClient, OpenAlexHttpClient};
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

    #[derive(Clone, Debug, PartialEq)]
    struct FeedRequest {
        interests: Vec<String>,
        offset: usize,
        limit: usize,
    }

    struct FakeSearch {
        papers: Vec<Paper>,
        fail: bool,
        calls: AtomicUsize,
        queries: Mutex<Vec<String>>,
        feed_papers: Vec<Paper>,
        feed_fail: bool,
        feed_calls: AtomicUsize,
        feed_requests: Mutex<Vec<FeedRequest>>,
    }

    impl FakeSearch {
        fn ok(papers: Vec<Paper>) -> Self {
            Self {
                papers: papers.clone(),
                fail: false,
                calls: AtomicUsize::new(0),
                queries: Mutex::new(Vec::new()),
                feed_papers: papers,
                feed_fail: false,
                feed_calls: AtomicUsize::new(0),
                feed_requests: Mutex::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                papers: Vec::new(),
                fail: true,
                calls: AtomicUsize::new(0),
                queries: Mutex::new(Vec::new()),
                feed_papers: Vec::new(),
                feed_fail: true,
                feed_calls: AtomicUsize::new(0),
                feed_requests: Mutex::new(Vec::new()),
            }
        }

        fn with_feed(feed_papers: Vec<Paper>) -> Self {
            Self {
                papers: Vec::new(),
                fail: false,
                calls: AtomicUsize::new(0),
                queries: Mutex::new(Vec::new()),
                feed_papers,
                feed_fail: false,
                feed_calls: AtomicUsize::new(0),
                feed_requests: Mutex::new(Vec::new()),
            }
        }

        fn failing_feed() -> Self {
            let mut fake = Self::with_feed(Vec::new());
            fake.feed_fail = true;
            fake
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn queries(&self) -> Vec<String> {
            self.queries
                .lock()
                .map(|queries| queries.clone())
                .unwrap_or_default()
        }

        fn feed_calls(&self) -> usize {
            self.feed_calls.load(Ordering::SeqCst)
        }

        fn feed_requests(&self) -> Vec<FeedRequest> {
            self.feed_requests
                .lock()
                .map(|requests| requests.clone())
                .unwrap_or_default()
        }
    }

    impl PaperSearch for FakeSearch {
        async fn search(&self, query: &str, _limit: usize) -> Result<Vec<Paper>> {
            self.calls.fetch_add(1, Ordering::SeqCst);

            if let Ok(mut queries) = self.queries.lock() {
                queries.push(query.to_string());
            }

            if self.fail {
                Err(Error::Network("source unavailable".into()))
            } else {
                Ok(self.papers.clone())
            }
        }

        async fn feed(&self, interests: &[String], offset: usize, limit: usize) -> Result<Vec<Paper>> {
            self.feed_calls.fetch_add(1, Ordering::SeqCst);

            if let Ok(mut requests) = self.feed_requests.lock() {
                requests.push(FeedRequest {
                    interests: interests.to_vec(),
                    offset,
                    limit,
                });
            }

            if self.feed_fail {
                Err(Error::Network("feed source unavailable".into()))
            } else {
                Ok(self.feed_papers.clone())
            }
        }
    }

    struct FakeLlm {
        translation: String,
        fail_translation: bool,
        translate_calls: AtomicUsize,
        configured: bool,
    }

    impl Default for FakeLlm {
        fn default() -> Self {
            Self {
                translation: String::new(),
                fail_translation: false,
                translate_calls: AtomicUsize::new(0),
                configured: true,
            }
        }
    }

    impl FakeLlm {
        fn translating(translation: &str) -> Self {
            Self {
                translation: translation.into(),
                ..Self::default()
            }
        }

        fn failing_translation() -> Self {
            Self {
                fail_translation: true,
                ..Self::default()
            }
        }

        fn unconfigured() -> Self {
            Self {
                configured: false,
                ..Self::default()
            }
        }

        fn translate_calls(&self) -> usize {
            self.translate_calls.load(Ordering::SeqCst)
        }
    }

    impl LlmProvider for FakeLlm {
        fn is_configured(&self) -> bool {
            self.configured
        }

        async fn summarize(&self, paper: &Paper) -> Result<Summary> {
            Ok(Summary {
                short: format!("summary of {}", paper.title),
                detailed: "detailed".into(),
                key_points: vec!["point".into()],
                model: "fake".into(),
                prompt_version: Summary::PROMPT_VERSION.into(),
            })
        }

        async fn to_english_keywords(&self, _chinese_query: &str) -> Result<String> {
            self.translate_calls.fetch_add(1, Ordering::SeqCst);

            if self.fail_translation {
                Err(Error::Llm("translation failed".into()))
            } else {
                Ok(self.translation.clone())
            }
        }
    }

    fn app(arxiv: FakeSearch, openalex: FakeSearch) -> App<FakeSearch, FakeLlm, FakeSearch> {
        app_with_llm(arxiv, FakeLlm::default(), openalex)
    }

    fn app_with_llm(
        arxiv: FakeSearch,
        llm: FakeLlm,
        openalex: FakeSearch,
    ) -> App<FakeSearch, FakeLlm, FakeSearch> {
        App::new(arxiv, llm, openalex)
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

        let response = app
            .search_papers("diffusion", 10)
            .await
            .expect("fallback should succeed");

        assert_eq!(response.papers.len(), 1);
        assert_eq!(response.papers[0].source, Source::OpenAlex);
        assert_eq!(response.query, "diffusion");
        assert_eq!(app.arxiv.calls(), 1);
        assert_eq!(app.openalex.calls(), 1);
    }

    #[tokio::test]
    async fn falls_back_when_arxiv_is_empty() {
        let app = app(
            FakeSearch::ok(Vec::new()),
            FakeSearch::ok(vec![paper("W2", "OpenAlex Only", Source::OpenAlex)]),
        );

        let response = app
            .search_papers("rare query", 10)
            .await
            .expect("fallback should succeed");

        assert_eq!(response.papers.len(), 1);
        assert_eq!(app.openalex.calls(), 1);
    }

    #[tokio::test]
    async fn keeps_arxiv_results_without_calling_openalex() {
        let app = app(
            FakeSearch::ok(vec![paper("2401.1", "Primary Paper", Source::Arxiv)]),
            FakeSearch::ok(vec![paper("W3", "Duplicate Title", Source::OpenAlex)]),
        );

        let response = app
            .search_papers("query", 10)
            .await
            .expect("primary source should succeed");

        assert_eq!(response.papers.len(), 1);
        assert_eq!(response.papers[0].source, Source::Arxiv);
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
    async fn translates_common_chinese_queries_locally() {
        let app = app_with_llm(
            FakeSearch::ok(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("should not be used"),
            FakeSearch::ok(Vec::new()),
        );

        let response = app
            .search_papers("最新的扩散模型论文", 10)
            .await
            .expect("search should succeed");

        assert_eq!(app.llm.translate_calls(), 0);
        assert_eq!(response.query, "diffusion models");
        assert_eq!(
            app.arxiv.queries(),
            vec!["diffusion models".to_string()]
        );
    }

    #[tokio::test]
    async fn uses_llm_for_unknown_chinese_queries() {
        let app = app_with_llm(
            FakeSearch::ok(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("niche topic"),
            FakeSearch::ok(Vec::new()),
        );

        let response = app
            .search_papers("冷门未知术语", 10)
            .await
            .expect("search should succeed");

        assert_eq!(app.llm.translate_calls(), 1);
        assert_eq!(response.query, "niche topic");
        assert_eq!(app.arxiv.queries(), vec!["niche topic".to_string()]);
    }

    #[tokio::test]
    async fn reports_config_error_for_unknown_chinese_without_llm() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::unconfigured(),
            FakeSearch::ok(Vec::new()),
        );

        let error = app
            .search_papers("冷门未知术语", 10)
            .await
            .expect_err("translation requires an LLM");

        assert!(matches!(error, Error::Config(_)));
        assert_eq!(app.arxiv.calls(), 0);
        assert_eq!(app.openalex.calls(), 0);
    }

    #[tokio::test]
    async fn does_not_translate_english_queries() {
        let app = app_with_llm(
            FakeSearch::ok(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("should not be used"),
            FakeSearch::ok(Vec::new()),
        );

        app.search_papers("diffusion models", 10)
            .await
            .expect("search should succeed");

        assert_eq!(app.llm.translate_calls(), 0);
        assert_eq!(
            app.arxiv.queries(),
            vec!["diffusion models".to_string()]
        );
    }

    #[tokio::test]
    async fn reports_translation_failure_without_searching() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::failing_translation(),
            FakeSearch::ok(Vec::new()),
        );

        let error = app
            .search_papers("冷门未知术语", 10)
            .await
            .expect_err("translation failed");

        assert!(matches!(error, Error::Llm(_)));
        assert_eq!(app.arxiv.calls(), 0);
        assert_eq!(app.openalex.calls(), 0);
    }

    fn papers(count: usize) -> Vec<Paper> {
        (0..count)
            .map(|index| {
                paper(
                    &format!("p{index}"),
                    &format!("Paper {index}"),
                    Source::Arxiv,
                )
            })
            .collect()
    }

    #[tokio::test]
    async fn browse_feed_uses_arxiv_first_and_normalizes_interests() {
        let app = app(
            FakeSearch::with_feed(papers(2)),
            FakeSearch::with_feed(vec![paper("W1", "Fallback", Source::OpenAlex)]),
        );

        let page = app
            .browse_feed(
                &[
                    " diffusion models ".into(),
                    "Diffusion Models".into(),
                    "".into(),
                    "computer vision".into(),
                ],
                0,
                20,
            )
            .await
            .expect("feed should succeed");

        assert_eq!(page.papers.len(), 2);
        assert!(!page.has_more);
        assert_eq!(app.arxiv.feed_calls(), 1);
        assert_eq!(app.openalex.feed_calls(), 0);
        assert_eq!(
            app.arxiv.feed_requests()[0],
            FeedRequest {
                interests: vec!["diffusion models".into(), "computer vision".into()],
                offset: 0,
                limit: 20,
            }
        );
    }

    #[tokio::test]
    async fn browse_feed_aligns_offset_and_clamps_limit() {
        let app = app(
            FakeSearch::with_feed(papers(1)),
            FakeSearch::with_feed(Vec::new()),
        );

        app.browse_feed(&["llm".into()], 30, 20)
            .await
            .expect("feed should succeed");

        let request = &app.arxiv.feed_requests()[0];
        assert_eq!(request.offset, 20);
        assert_eq!(request.limit, 20);

        app.browse_feed(&["llm".into()], 0, 10_000)
            .await
            .expect("feed should succeed");

        assert_eq!(app.arxiv.feed_requests()[1].limit, FEED_MAX_LIMIT);
    }

    #[tokio::test]
    async fn browse_feed_skips_sources_without_interests() {
        let app = app(
            FakeSearch::with_feed(Vec::new()),
            FakeSearch::with_feed(Vec::new()),
        );

        let page = app
            .browse_feed(&["  ".into(), "".into()], 0, 12)
            .await
            .expect("empty interests should not fail");

        assert!(page.papers.is_empty());
        assert!(!page.has_more);
        assert_eq!(app.arxiv.feed_calls(), 0);
        assert_eq!(app.openalex.feed_calls(), 0);
    }

    #[tokio::test]
    async fn browse_feed_falls_back_when_arxiv_fails() {
        let app = app(
            FakeSearch::failing_feed(),
            FakeSearch::with_feed(vec![paper("W1", "Fallback Paper", Source::OpenAlex)]),
        );

        let page = app
            .browse_feed(&["diffusion".into()], 0, 12)
            .await
            .expect("fallback should succeed");

        assert_eq!(page.papers.len(), 1);
        assert_eq!(page.papers[0].source, Source::OpenAlex);
        assert_eq!(app.openalex.feed_calls(), 1);
    }

    #[tokio::test]
    async fn browse_feed_falls_back_when_first_page_is_empty() {
        let app = app(
            FakeSearch::with_feed(Vec::new()),
            FakeSearch::with_feed(vec![paper("W2", "OpenAlex Only", Source::OpenAlex)]),
        );

        let page = app
            .browse_feed(&["rare".into()], 0, 12)
            .await
            .expect("fallback should succeed");

        assert_eq!(page.papers.len(), 1);
        assert_eq!(app.openalex.feed_calls(), 1);
    }

    #[tokio::test]
    async fn browse_feed_treats_empty_later_page_as_exhausted() {
        let app = app(
            FakeSearch::with_feed(Vec::new()),
            FakeSearch::with_feed(vec![paper("W3", "Should Not Appear", Source::OpenAlex)]),
        );

        let page = app
            .browse_feed(&["llm".into()], 12, 12)
            .await
            .expect("later page should be empty");

        assert!(page.papers.is_empty());
        assert!(!page.has_more);
        assert_eq!(app.openalex.feed_calls(), 0);
    }

    #[tokio::test]
    async fn browse_feed_reports_error_when_all_sources_fail() {
        let app = app(FakeSearch::failing_feed(), FakeSearch::failing_feed());

        let error = app
            .browse_feed(&["llm".into()], 0, 12)
            .await
            .expect_err("both sources failed");

        assert!(matches!(
            error,
            Error::Sources(message)
                if message.contains("arXiv") && message.contains("OpenAlex")
        ));
    }

    #[tokio::test]
    async fn browse_feed_reports_error_when_openalex_fails_after_empty_arxiv() {
        let app = app(
            FakeSearch::with_feed(Vec::new()),
            FakeSearch::failing_feed(),
        );

        let error = app
            .browse_feed(&["llm".into()], 0, 12)
            .await
            .expect_err("OpenAlex failed");

        assert!(matches!(
            error,
            Error::Sources(message) if message.contains("OpenAlex")
        ));
    }

    #[tokio::test]
    async fn browse_feed_keeps_has_more_before_deduping() {
        let mut duplicate_papers = papers(2);
        duplicate_papers[1].title = duplicate_papers[0].title.clone();

        let app = app(
            FakeSearch::with_feed(duplicate_papers),
            FakeSearch::with_feed(Vec::new()),
        );

        let page = app
            .browse_feed(&["llm".into()], 0, 2)
            .await
            .expect("feed should succeed");

        assert_eq!(page.papers.len(), 1);
        assert!(page.has_more);
    }

    #[tokio::test]
    async fn browse_feed_translates_common_chinese_interests_locally() {
        let app = app_with_llm(
            FakeSearch::with_feed(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("should not be used"),
            FakeSearch::with_feed(Vec::new()),
        );

        let page = app
            .browse_feed(&["扩散模型".into()], 0, 12)
            .await
            .expect("feed should succeed");

        assert_eq!(page.papers.len(), 1);
        assert_eq!(app.llm.translate_calls(), 0);
        assert_eq!(
            app.arxiv.feed_requests()[0].interests,
            vec!["diffusion models".to_string()]
        );
    }

    #[tokio::test]
    async fn browse_feed_uses_llm_for_unknown_chinese_interests() {
        let app = app_with_llm(
            FakeSearch::with_feed(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("niche topic"),
            FakeSearch::with_feed(Vec::new()),
        );

        app.browse_feed(&["冷门未知术语".into()], 0, 12)
            .await
            .expect("feed should succeed");

        assert_eq!(app.llm.translate_calls(), 1);
        assert_eq!(
            app.arxiv.feed_requests()[0].interests,
            vec!["niche topic".to_string()]
        );
    }

    #[tokio::test]
    async fn browse_feed_errors_for_unknown_chinese_without_llm() {
        let app = app_with_llm(
            FakeSearch::with_feed(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::unconfigured(),
            FakeSearch::with_feed(Vec::new()),
        );

        let error = app
            .browse_feed(&["冷门未知术语".into()], 0, 12)
            .await
            .expect_err("translation requires an LLM");

        assert!(matches!(error, Error::Config(_)));
        assert_eq!(app.arxiv.feed_calls(), 0);
    }

    #[test]
    fn normalize_interests_trims_dedupes_and_caps() {
        let input: Vec<String> = (0..MAX_FEED_INTERESTS + 2)
            .map(|index| format!(" topic {index} "))
            .chain(["Topic 0".to_string(), "".to_string()])
            .collect();

        let normalized = normalize_interests(&input);

        assert_eq!(normalized.len(), MAX_FEED_INTERESTS);
        assert_eq!(normalized[0], "topic 0");
        assert!(normalized.iter().all(|interest| !interest.starts_with(' ')));
    }

    #[tokio::test]
    #[ignore = "hits the live arXiv and OpenAlex APIs"]
    async fn live_chinese_query_is_translated_locally() {
        let arxiv = ArxivHttpClient::new().expect("arXiv client should build");
        let openalex = OpenAlexHttpClient::new().expect("OpenAlex client should build");
        // No LLM key is configured for this test.
        let app = App::new(arxiv, FakeLlm::unconfigured(), openalex);

        let response = app
            .search_papers("扩散模型", 3)
            .await
            .expect("local glossary translation should be enough");

        assert_eq!(response.query, "diffusion models");
        assert!(!response.papers.is_empty());
    }

    #[tokio::test]
    #[ignore = "hits the live arXiv and OpenAlex APIs"]
    async fn live_feed_returns_newest_papers() {
        let arxiv = ArxivHttpClient::new().expect("arXiv client should build");
        let openalex = OpenAlexHttpClient::new().expect("OpenAlex client should build");
        let app = App::new(arxiv, FakeLlm::default(), openalex);

        let page = app
            .browse_feed(&["diffusion models".into()], 0, 3)
            .await
            .expect("at least one source should work");

        assert!(!page.papers.is_empty());
        assert!(
            page.papers
                .iter()
                .all(|p| matches!(p.source, Source::Arxiv | Source::OpenAlex))
        );
    }

    #[tokio::test]
    #[ignore = "hits the live arXiv and OpenAlex APIs"]
    async fn live_search_falls_back_to_a_working_source() {
        let arxiv = ArxivHttpClient::new().expect("arXiv client should build");
        let openalex = OpenAlexHttpClient::new().expect("OpenAlex client should build");
        let app = App::new(arxiv, FakeLlm::default(), openalex);

        let response = app
            .search_papers("diffusion models", 3)
            .await
            .expect("at least one source should work");

        assert!(!response.papers.is_empty());
        assert!(
            response
                .papers
                .iter()
                .all(|p| matches!(p.source, Source::Arxiv | Source::OpenAlex))
        );
    }
}
