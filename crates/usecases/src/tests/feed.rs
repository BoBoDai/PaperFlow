use super::support::*;
use crate::{FEED_MAX_LIMIT, MAX_FEED_INTERESTS};
use paperflow_core::{Error, Source};

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

    let normalized = crate::feed::normalize_interests(&input);

    assert_eq!(normalized.len(), MAX_FEED_INTERESTS);
    assert_eq!(normalized[0], "topic 0");
    assert!(normalized.iter().all(|interest| !interest.starts_with(' ')));
}
