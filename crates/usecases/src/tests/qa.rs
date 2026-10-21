use super::support::*;
use crate::{QA_CONTEXT_BUDGET, QA_HISTORY_TURNS, QA_QUESTION_BUDGET};
use paperflow_core::{Error, QaRequest, QaRole, QaScope, QaTurn, Source};

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
    let app = app(FakeSearch::ok(Vec::new()), FakeSearch::ok(Vec::new()));
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
    let app = app(FakeSearch::ok(Vec::new()), FakeSearch::ok(Vec::new()));

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
    let app = app(FakeSearch::ok(Vec::new()), FakeSearch::ok(Vec::new()));

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
async fn summarize_stream_emits_deltas() {
    let app = app_with_llm(
        FakeSearch::ok(Vec::new()),
        FakeLlm::default(),
        FakeSearch::ok(Vec::new()),
    );

    let mut deltas = Vec::new();
    let summary = app
        .summarize_stream(&paper("p1", "Paper", Source::Arxiv), |delta| {
            deltas.push(delta)
        })
        .await
        .expect("summary stream should succeed");

    assert_eq!(summary.detailed, "detailed");
    assert_eq!(deltas.concat(), "detailed");
}

#[tokio::test]
async fn ask_paper_stream_emits_deltas() {
    let app = app_with_llm(
        FakeSearch::ok(Vec::new()),
        FakeLlm::answering("流式答案"),
        FakeSearch::ok(Vec::new()),
    );

    let mut deltas = Vec::new();
    let response = app
        .ask_paper_stream(qa_request(QaScope::CurrentPage, "page text"), |delta| {
            deltas.push(delta)
        })
        .await
        .expect("stream should succeed");

    assert_eq!(response.answer, "流式答案");
    assert_eq!(deltas.concat(), "流式答案");
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
