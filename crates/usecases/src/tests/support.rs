use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Answer, Error, Paper, QaRequest, Result, Source, Summary};

use crate::App;

pub(super) fn paper(id: &str, title: &str, source: Source) -> Paper {
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
pub(super) struct FeedRequest {
    pub(super) interests: Vec<String>,
    pub(super) offset: usize,
    pub(super) limit: usize,
}

pub(super) struct FakeSearch {
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
    pub(super) fn ok(papers: Vec<Paper>) -> Self {
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

    pub(super) fn failing() -> Self {
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

    pub(super) fn with_feed(feed_papers: Vec<Paper>) -> Self {
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

    pub(super) fn failing_feed() -> Self {
        let mut fake = Self::with_feed(Vec::new());
        fake.feed_fail = true;
        fake
    }

    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub(super) fn queries(&self) -> Vec<String> {
        self.queries
            .lock()
            .map(|queries| queries.clone())
            .unwrap_or_default()
    }

    pub(super) fn feed_calls(&self) -> usize {
        self.feed_calls.load(Ordering::SeqCst)
    }

    pub(super) fn feed_requests(&self) -> Vec<FeedRequest> {
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

pub(super) struct FakeLlm {
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
    pub(super) fn translating(translation: &str) -> Self {
        Self {
            translation: translation.into(),
            ..Self::default()
        }
    }

    pub(super) fn failing_translation() -> Self {
        Self {
            fail_translation: true,
            ..Self::default()
        }
    }

    pub(super) fn unconfigured() -> Self {
        Self {
            configured: false,
            ..Self::default()
        }
    }

    pub(super) fn answering(answer: &str) -> Self {
        Self {
            answer: answer.into(),
            ..Self::default()
        }
    }

    pub(super) fn failing_answer() -> Self {
        Self {
            fail_answer: true,
            ..Self::default()
        }
    }

    pub(super) fn translate_calls(&self) -> usize {
        self.translate_calls.load(Ordering::SeqCst)
    }

    pub(super) fn answer_calls(&self) -> usize {
        self.answer_calls.load(Ordering::SeqCst)
    }

    pub(super) fn last_qa(&self) -> Option<QaRequest> {
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

    async fn summarize_stream(
        &self,
        paper: &Paper,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Summary> {
        let summary = self.summarize(paper).await?;
        on_delta(summary.detailed.clone());
        Ok(summary)
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

    async fn ask_paper_stream(
        &self,
        request: &QaRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<Answer> {
        let answer = self.ask_paper(request).await?;
        on_delta(answer.answer.clone());
        Ok(answer)
    }
}

pub(super) fn app(arxiv: FakeSearch, openalex: FakeSearch) -> App<FakeSearch, FakeLlm, FakeSearch> {
    app_with_llm(arxiv, FakeLlm::default(), openalex)
}

pub(super) fn app_with_llm(
    arxiv: FakeSearch,
    llm: FakeLlm,
    openalex: FakeSearch,
) -> App<FakeSearch, FakeLlm, FakeSearch> {
    App::new(arxiv, llm, openalex)
}

pub(super) fn papers(count: usize) -> Vec<Paper> {
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
