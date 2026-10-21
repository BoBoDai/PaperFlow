use super::support::*;
use crate::dedupe;
use paperflow_core::{Error, Source};

#[test]
fn dedupe_removes_cross_source_duplicates() {
    let input = vec![
        paper(
            "arxiv:1706.03762",
            "Attention Is All You Need",
            Source::Arxiv,
        ),
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
    assert_eq!(app.arxiv.queries(), vec!["diffusion models".to_string()]);
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
    assert_eq!(app.arxiv.queries(), vec!["diffusion models".to_string()]);
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
