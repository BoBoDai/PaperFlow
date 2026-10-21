use super::*;
use chrono::{Datelike, Timelike};

const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:opensearch="http://a9.com/-/spec/opensearch/1.1/"
  xmlns:arxiv="http://arxiv.org/schemas/atom"
  xmlns="http://www.w3.org/2005/Atom">
  <title>arXiv Query</title>
  <entry>
<id>http://arxiv.org/abs/2405.03150v2</id>
<title>Video Diffusion Models:
  A Survey</title>
<summary>Diffusion generative models have recently become a
  powerful technique.</summary>
<published>2024-05-06T04:01:42Z</published>
<link href="https://arxiv.org/abs/2405.03150v2" rel="alternate" type="text/html"/>
<link href="https://arxiv.org/pdf/2405.03150v2" rel="related" type="application/pdf" title="pdf"/>
<category term="cs.CV" scheme="http://arxiv.org/schemas/atom"/>
<category term="cs.LG" scheme="http://arxiv.org/schemas/atom"/>
<arxiv:primary_category term="cs.CV"/>
<author><name>Andrew Melnik</name></author>
<author><name>Michal Ljubljanac</name></author>
  </entry>
</feed>"#;

#[test]
fn parses_entry_into_paper() {
    let papers = parse_feed(FEED).expect("feed should parse");
    assert_eq!(papers.len(), 1);

    let paper = &papers[0];
    assert_eq!(paper.id, "2405.03150");
    assert_eq!(paper.title, "Video Diffusion Models: A Survey");
    assert_eq!(
        paper.abstract_text,
        "Diffusion generative models have recently become a powerful technique."
    );
    assert_eq!(paper.authors, ["Andrew Melnik", "Michal Ljubljanac"]);
    assert_eq!(paper.categories, ["cs.CV", "cs.LG"]);
    assert_eq!(paper.pdf_url, "https://arxiv.org/pdf/2405.03150v2");
    assert_eq!(paper.source, Source::Arxiv);
    assert_eq!(paper.published.year(), 2024);
    assert_eq!(paper.published.month(), 5);
    assert_eq!(paper.published.day(), 6);
    assert_eq!(paper.published.hour(), 4);
    assert_eq!(paper.published.minute(), 1);
}

#[test]
fn keeps_old_style_ids_and_strips_version() {
    assert_eq!(
        arxiv_id("http://arxiv.org/abs/hep-th/9901001v2"),
        "hep-th/9901001"
    );
    assert_eq!(arxiv_id("http://arxiv.org/abs/2405.03150"), "2405.03150");
    assert_eq!(arxiv_id("2405.03150v3"), "2405.03150");
}

#[test]
fn surfaces_api_error_entries() {
    let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
<id>http://arxiv.org/api/errors#bad_query</id>
<title>Error</title>
<summary>Malformed query.</summary>
<published>2024-05-06T04:01:42Z</published>
<author><name>arXiv API</name></author>
  </entry>
</feed>"#;

    let err = parse_feed(xml).expect_err("error entry must fail");
    assert!(matches!(err, Error::Parse(message) if message.contains("Malformed query.")));
}

#[test]
fn rejects_malformed_xml() {
    assert!(matches!(parse_feed("<feed>"), Err(Error::Parse(_))));
}

#[test]
fn feed_query_joins_interests_with_or() {
    assert_eq!(
        build_feed_query(&[
            "large language models".into(),
            " diffusion models ".into(),
            "".into(),
        ]),
        Some(r#"(all:"large language models" OR all:"diffusion models")"#.to_string())
    );
}

#[test]
fn feed_query_sanitizes_and_handles_single_interest() {
    assert_eq!(
        build_feed_query(&["diffusion models".into()]),
        Some(r#"all:"diffusion models""#.to_string())
    );
    assert_eq!(
        build_feed_query(&[r#"a "quoted" \ topic"#.into()]),
        Some(r#"all:"a quoted topic""#.to_string())
    );
}

#[test]
fn feed_query_is_none_without_interests() {
    assert_eq!(build_feed_query(&[]), None);
    assert_eq!(build_feed_query(&[" ".into()]), None);
}

#[tokio::test]
#[ignore = "hits the live arXiv API"]
async fn live_search_returns_papers() {
    let client = ArxivHttpClient::new().expect("client should build");
    let papers = client
        .search("diffusion models", 3)
        .await
        .expect("live search should succeed");

    assert!(!papers.is_empty());
    assert!(papers.iter().all(|p| !p.title.is_empty()));
    assert!(papers.iter().all(|p| p.source == Source::Arxiv));
}
