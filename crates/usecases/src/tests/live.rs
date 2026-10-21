use super::support::*;
use crate::App;
use paperflow_adapters::{ArxivHttpClient, OpenAlexHttpClient};
use paperflow_core::Source;

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
