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
    contains_cjk, Error, FeedPage, Paper, QaRequest, QaResponse, QaRole, QaScope, QaTurn,
    Result, SearchResponse, Summary,
};

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

    /// 检索论文，并做跨源去重。
    ///
    /// 中文查询在 LLM 已配置时转成英文关键词；未配置时直接按原文检索。
    /// LLM 已配置但调用失败仍然报错，不静默吞掉失败。
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

        let search_query = self.effective_query(query).await?;

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

    /// Translate a Chinese query with the configured LLM. Without an LLM,
    /// fall back to the original text so search still works.
    async fn effective_query(&self, query: &str) -> Result<String> {
        if !contains_cjk(query) {
            return Ok(query.to_string());
        }

        if !self.llm.is_configured() {
            tracing::warn!(
                query,
                "LLM is not configured; searching the original Chinese query"
            );
            return Ok(query.to_string());
        }

        let translated = self.llm.to_english_keywords(query).await?;
        tracing::info!(original = query, translated = %translated, "translated query with LLM");
        Ok(translated)
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
            translated.push(self.effective_query(interest).await?);
        }

        Ok(translated)
    }

    /// 基于 PDF 上下文回答论文问题。
    pub async fn ask_paper(&self, request: QaRequest) -> Result<QaResponse> {
        let (request, context_truncated, context_chars) = prepare_qa(request)?;
        let answer = self.llm.ask_paper(&request).await?;

        Ok(QaResponse {
            answer: answer.answer,
            model: answer.model,
            prompt_version: answer.prompt_version,
            scope: request.scope,
            page: request.page,
            context_truncated,
            context_chars,
        })
    }

    /// 分析论文
    pub async fn summarize(&self, paper: &Paper) -> Result<Summary> {
        self.llm.summarize(paper).await
    }
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

fn prepare_qa(mut request: QaRequest) -> Result<(QaRequest, bool, usize)> {
    let question = request.question.trim().to_string();
    if question.is_empty() {
        return Err(Error::InvalidInput("问题不能为空".into()));
    }
    if char_count(&question) > QA_QUESTION_BUDGET {
        return Err(Error::InvalidInput(format!(
            "问题过长（最多 {QA_QUESTION_BUDGET} 字）"
        )));
    }
    request.question = question;

    let context = request.context.trim().to_string();
    if context.is_empty() {
        return Err(Error::InvalidInput(
            "没有可用的 PDF 文本。该 PDF 可能是扫描版，或文本尚未提取完成。".into(),
        ));
    }

    let budget = match request.scope {
        QaScope::Selection => QA_SELECTION_BUDGET,
        QaScope::CurrentPage | QaScope::FullPaper => QA_CONTEXT_BUDGET,
    };

    let (context, truncated) = if request.scope == QaScope::FullPaper && char_count(&context) > budget
    {
        let head: String = context.chars().take(budget * 3 / 4).collect();
        let tail: String = context
            .chars()
            .rev()
            .take(budget / 4)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        (
            format!("{head}\n\n[…上下文过长，中间内容已省略…]\n\n{tail}"),
            true,
        )
    } else if char_count(&context) > budget {
        (context.chars().take(budget).collect(), true)
    } else {
        (context, false)
    };

    let context_chars = char_count(&context);
    request.context = context;
    request.history = sanitize_history(request.history);

    Ok((request, truncated, context_chars))
}

fn sanitize_history(history: Vec<QaTurn>) -> Vec<QaTurn> {
    let mut turns: Vec<QaTurn> = history
        .into_iter()
        .filter_map(|turn| {
            let content = turn.content.trim().to_string();
            if content.is_empty() {
                return None;
            }

            let budget = match turn.role {
                QaRole::User => 2_000,
                QaRole::Assistant => 4_000,
            };

            Some(QaTurn {
                role: turn.role,
                content: content.chars().take(budget).collect(),
            })
        })
        .collect();

    if turns.len() > QA_HISTORY_TURNS {
        turns = turns.split_off(turns.len() - QA_HISTORY_TURNS);
    }

    let mut total: usize = turns.iter().map(|turn| char_count(&turn.content)).sum();
    while total > QA_HISTORY_BUDGET && turns.len() > 1 {
        let removed = turns.remove(0);
        total -= char_count(&removed.content);
    }

    turns
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
    use paperflow_core::{Answer, Source};

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
        answer: String,
        fail_answer: bool,
        answer_calls: AtomicUsize,
        last_qa: Mutex<Option<QaRequest>>,
    }

    impl Default for FakeLlm {
        fn default() -> Self {
            Self {
                translation: String::new(),
                fail_translation: false,
                translate_calls: AtomicUsize::new(0),
                configured: true,
                answer: "答案".into(),
                fail_answer: false,
                answer_calls: AtomicUsize::new(0),
                last_qa: Mutex::new(None),
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

        fn answering(answer: &str) -> Self {
            Self {
                answer: answer.into(),
                ..Self::default()
            }
        }

        fn failing_answer() -> Self {
            Self {
                fail_answer: true,
                ..Self::default()
            }
        }

        fn translate_calls(&self) -> usize {
            self.translate_calls.load(Ordering::SeqCst)
        }

        fn answer_calls(&self) -> usize {
            self.answer_calls.load(Ordering::SeqCst)
        }

        fn last_qa(&self) -> Option<QaRequest> {
            self.last_qa
                .lock()
                .ok()
                .and_then(|request| request.as_ref().cloned())
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

        async fn ask_paper(&self, request: &QaRequest) -> Result<Answer> {
            self.answer_calls.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut last) = self.last_qa.lock() {
                *last = Some(request.clone());
            }

            if self.fail_answer {
                return Err(Error::Llm("qa failed".into()));
            }

            Ok(Answer {
                answer: self.answer.clone(),
                model: "fake".into(),
                prompt_version: Answer::PROMPT_VERSION.into(),
            })
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
    async fn translates_chinese_queries_with_llm() {
        let app = app_with_llm(
            FakeSearch::ok(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("diffusion models"),
            FakeSearch::ok(Vec::new()),
        );

        let response = app
            .search_papers("最新的扩散模型论文", 10)
            .await
            .expect("search should succeed");

        assert_eq!(app.llm.translate_calls(), 1);
        assert_eq!(response.query, "diffusion models");
        assert_eq!(
            app.arxiv.queries(),
            vec!["diffusion models".to_string()]
        );
    }

    #[tokio::test]
    async fn searches_original_chinese_query_without_llm() {
        let app = app_with_llm(
            FakeSearch::ok(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::unconfigured(),
            FakeSearch::ok(Vec::new()),
        );

        let response = app
            .search_papers("扩散模型", 10)
            .await
            .expect("search should fall back to the original query");

        assert_eq!(app.llm.translate_calls(), 0);
        assert_eq!(response.query, "扩散模型");
        assert_eq!(app.arxiv.queries(), vec!["扩散模型".to_string()]);
        assert_eq!(response.papers.len(), 1);
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
            .search_papers("扩散模型", 10)
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
    async fn browse_feed_translates_chinese_interests_with_llm() {
        let app = app_with_llm(
            FakeSearch::with_feed(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::translating("diffusion models"),
            FakeSearch::with_feed(Vec::new()),
        );

        let page = app
            .browse_feed(&["扩散模型".into()], 0, 12)
            .await
            .expect("feed should succeed");

        assert_eq!(page.papers.len(), 1);
        assert_eq!(app.llm.translate_calls(), 1);
        assert_eq!(
            app.arxiv.feed_requests()[0].interests,
            vec!["diffusion models".to_string()]
        );
    }

    #[tokio::test]
    async fn browse_feed_uses_original_chinese_interest_without_llm() {
        let app = app_with_llm(
            FakeSearch::with_feed(vec![paper("2401.1", "Paper", Source::Arxiv)]),
            FakeLlm::unconfigured(),
            FakeSearch::with_feed(Vec::new()),
        );

        let page = app
            .browse_feed(&["扩散模型".into()], 0, 12)
            .await
            .expect("feed should fall back to the original interest");

        assert_eq!(app.llm.translate_calls(), 0);
        assert_eq!(
            app.arxiv.feed_requests()[0].interests,
            vec!["扩散模型".to_string()]
        );
        assert_eq!(page.papers.len(), 1);
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

    fn qa_request(scope: QaScope, context: &str) -> QaRequest {
        QaRequest {
            paper: paper("p1", "Paper", Source::Arxiv),
            question: "这篇论文的核心贡献是什么？".into(),
            scope,
            context: context.into(),
            page: Some(2),
            history: Vec::new(),
        }
    }

    #[tokio::test]
    async fn ask_paper_rejects_empty_question() {
        let app = app(
            FakeSearch::ok(Vec::new()),
            FakeSearch::ok(Vec::new()),
        );
        let mut request = qa_request(QaScope::CurrentPage, "page text");
        request.question = "   ".into();

        let error = app
            .ask_paper(request)
            .await
            .expect_err("empty question should fail");

        assert!(matches!(error, Error::InvalidInput(_)));
    }

    #[tokio::test]
    async fn ask_paper_rejects_empty_context() {
        let app = app(
            FakeSearch::ok(Vec::new()),
            FakeSearch::ok(Vec::new()),
        );

        let error = app
            .ask_paper(qa_request(QaScope::CurrentPage, "   "))
            .await
            .expect_err("empty context should fail");

        assert!(matches!(error, Error::InvalidInput(_)));
    }

    #[tokio::test]
    async fn ask_paper_truncates_full_context_head_and_tail() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::answering("答案"),
            FakeSearch::ok(Vec::new()),
        );

        let context = format!("HEAD{}TAIL", "x".repeat(50_000));
        let response = app
            .ask_paper(qa_request(QaScope::FullPaper, &context))
            .await
            .expect("qa should succeed");

        assert!(response.context_truncated);
        assert!(response.context_chars <= QA_CONTEXT_BUDGET + 100);

        let sent = app.llm.last_qa().expect("request should be recorded");
        assert!(sent.context.contains("HEAD"));
        assert!(sent.context.contains("TAIL"));
        assert!(sent.context.contains("中间内容已省略"));
    }

    #[tokio::test]
    async fn ask_paper_keeps_only_recent_history() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::default(),
            FakeSearch::ok(Vec::new()),
        );

        let mut request = qa_request(QaScope::CurrentPage, "page text");
        request.history = (0..10)
            .map(|index| QaTurn {
                role: if index % 2 == 0 {
                    QaRole::User
                } else {
                    QaRole::Assistant
                },
                content: format!("turn {index}"),
            })
            .collect();

        app.ask_paper(request).await.expect("qa should succeed");

        let sent = app.llm.last_qa().expect("request should be recorded");
        assert_eq!(sent.history.len(), QA_HISTORY_TURNS);
        assert_eq!(sent.history[0].content, "turn 4");
        assert_eq!(sent.history[5].content, "turn 9");
    }

    #[tokio::test]
    async fn ask_paper_caps_question_by_characters() {
        let app = app(
            FakeSearch::ok(Vec::new()),
            FakeSearch::ok(Vec::new()),
        );

        let mut request = qa_request(QaScope::CurrentPage, "page text");
        request.question = "问".repeat(QA_QUESTION_BUDGET + 1);

        let error = app
            .ask_paper(request)
            .await
            .expect_err("question is too long");

        assert!(matches!(error, Error::InvalidInput(_)));
    }

    #[tokio::test]
    async fn ask_paper_returns_scope_and_page() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::answering("答案"),
            FakeSearch::ok(Vec::new()),
        );

        let response = app
            .ask_paper(qa_request(QaScope::CurrentPage, "page text"))
            .await
            .expect("qa should succeed");

        assert_eq!(response.answer, "答案");
        assert_eq!(response.scope, QaScope::CurrentPage);
        assert_eq!(response.page, Some(2));
        assert_eq!(app.llm.answer_calls(), 1);
    }

    #[tokio::test]
    async fn ask_paper_propagates_llm_errors() {
        let app = app_with_llm(
            FakeSearch::ok(Vec::new()),
            FakeLlm::failing_answer(),
            FakeSearch::ok(Vec::new()),
        );

        let error = app
            .ask_paper(qa_request(QaScope::CurrentPage, "page text"))
            .await
            .expect_err("llm should fail");

        assert!(matches!(error, Error::Llm(_)));
    }

    #[tokio::test]
    #[ignore = "hits the live arXiv and OpenAlex APIs"]
    async fn live_chinese_query_uses_llm_translation() {
        let arxiv = ArxivHttpClient::new().expect("arXiv client should build");
        let openalex = OpenAlexHttpClient::new().expect("OpenAlex client should build");
        // The LLM is faked so the test does not need an API key; the search
        // sources are real.
        let app = App::new(arxiv, FakeLlm::translating("diffusion models"), openalex);

        let response = app
            .search_papers("扩散模型", 3)
            .await
            .expect("LLM translation should be enough");

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
