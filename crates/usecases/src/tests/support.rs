use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use paperflow_adapters::{LlmProvider, PaperSearch, ReviewProvider};
use paperflow_core::{
    Answer, Error, OpinionRequest, Paper, PersonaAnswer, PersonaChatRequest, PersonaId,
    PersonaOpinion, PersonaReview, QaRequest, Result, Source, Stance, Summary,
};

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
    review_fail: Option<PersonaId>,
    opinion_fail: Option<PersonaId>,
    review_stance: Stance,
    opinion_stance: Stance,
    review_calls: AtomicUsize,
    chat_calls: AtomicUsize,
    opinion_calls: AtomicUsize,
    last_chat: Mutex<Option<PersonaChatRequest>>,
    opinion_requests: Mutex<Vec<OpinionRequest>>,
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
            review_fail: None,
            opinion_fail: None,
            review_stance: Stance::Support,
            opinion_stance: Stance::Support,
            review_calls: AtomicUsize::new(0),
            chat_calls: AtomicUsize::new(0),
            opinion_calls: AtomicUsize::new(0),
            last_chat: Mutex::new(None),
            opinion_requests: Mutex::new(Vec::new()),
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

    pub(super) fn failing_review(persona: PersonaId) -> Self {
        Self {
            review_fail: Some(persona),
            ..Self::default()
        }
    }

    pub(super) fn failing_opinion(persona: PersonaId) -> Self {
        Self {
            opinion_fail: Some(persona),
            ..Self::default()
        }
    }

    pub(super) fn review_calls(&self) -> usize {
        self.review_calls.load(Ordering::SeqCst)
    }

    pub(super) fn chat_calls(&self) -> usize {
        self.chat_calls.load(Ordering::SeqCst)
    }

    pub(super) fn opinion_calls(&self) -> usize {
        self.opinion_calls.load(Ordering::SeqCst)
    }

    pub(super) fn last_chat_request(&self) -> Option<PersonaChatRequest> {
        self.last_chat
            .lock()
            .ok()
            .and_then(|request| request.as_ref().cloned())
    }

    pub(super) fn recorded_opinion_requests(&self) -> Vec<OpinionRequest> {
        self.opinion_requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

impl ReviewProvider for FakeLlm {
    async fn review_persona(
        &self,
        _paper: &Paper,
        persona: PersonaId,
        _context: Option<&str>,
    ) -> Result<PersonaReview> {
        self.review_calls.fetch_add(1, Ordering::SeqCst);

        if self.review_fail == Some(persona) {
            return Err(Error::Llm("review failed".into()));
        }

        Ok(PersonaReview {
            persona,
            stance: self.review_stance,
            confidence: 80,
            summary: "summary".into(),
            strengths: vec!["strength".into()],
            concerns: vec!["concern".into()],
            riskiest_claim: "claim".into(),
            would_change_mind: "evidence".into(),
            model: "fake".into(),
        })
    }

    async fn persona_chat_stream(
        &self,
        request: &PersonaChatRequest,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> Result<PersonaAnswer> {
        self.chat_calls.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut last) = self.last_chat.lock() {
            *last = Some(request.clone());
        }

        if self.fail_answer {
            return Err(Error::Llm("chat failed".into()));
        }

        let answer = format!("{} 的回答", request.persona.label());
        on_delta(answer.clone());

        Ok(PersonaAnswer {
            answer,
            model: "fake".into(),
            prompt_version: PersonaAnswer::PROMPT_VERSION.into(),
        })
    }

    async fn persona_opinion(&self, request: &OpinionRequest) -> Result<PersonaOpinion> {
        self.opinion_calls.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut requests) = self.opinion_requests.lock() {
            requests.push(request.clone());
        }

        if self.opinion_fail == Some(request.persona) {
            return Err(Error::Llm("opinion failed".into()));
        }

        Ok(PersonaOpinion {
            persona: request.persona,
            stance: self.opinion_stance,
            confidence: 72,
            reason: "理由".into(),
        })
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
