use super::support::*;
use crate::TRANSLATE_TEXT_BUDGET;
use paperflow_core::{Error, TranslateRequest};

fn request(text: &str) -> TranslateRequest {
    TranslateRequest { text: text.into() }
}

#[tokio::test]
async fn translate_rejects_empty_text() {
    let app = app(FakeSearch::ok(Vec::new()), FakeSearch::ok(Vec::new()));

    let error = app
        .translate_text(request("   "))
        .await
        .expect_err("empty text should fail");

    assert!(matches!(error, Error::InvalidInput(_)));
    assert_eq!(app.llm.text_translate_calls(), 0);
}

#[tokio::test]
async fn translate_truncates_long_text() {
    let app = app_with_llm(
        FakeSearch::ok(Vec::new()),
        FakeLlm::default(),
        FakeSearch::ok(Vec::new()),
    );

    let text = "x".repeat(TRANSLATE_TEXT_BUDGET + 10);
    let translation = app
        .translate_text(request(&text))
        .await
        .expect("translation should succeed");

    assert!(translation.truncated);
    assert_eq!(translation.text, "译文");
    assert_eq!(app.llm.text_translate_calls(), 1);
}

#[tokio::test]
async fn translate_streams_deltas() {
    let app = app_with_llm(
        FakeSearch::ok(Vec::new()),
        FakeLlm::default(),
        FakeSearch::ok(Vec::new()),
    );

    let mut deltas = Vec::new();
    let translation = app
        .translate_text_stream(request("hello"), |delta| deltas.push(delta))
        .await
        .expect("translation should succeed");

    assert_eq!(deltas, vec!["译文".to_string()]);
    assert_eq!(translation.text, "译文");
    assert!(!translation.truncated);
}

#[tokio::test]
async fn translate_propagates_llm_errors() {
    let app = app_with_llm(
        FakeSearch::ok(Vec::new()),
        FakeLlm::failing_text_translation(),
        FakeSearch::ok(Vec::new()),
    );

    let error = app
        .translate_text(request("hello"))
        .await
        .expect_err("llm failure should propagate");

    assert!(matches!(error, Error::Llm(_)));
}
