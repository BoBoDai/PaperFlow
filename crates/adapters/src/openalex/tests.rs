use super::*;
use chrono::Datelike;

const WORKS: &str = r#"{
  "results": [
{
  "id": "https://openalex.org/W4360884927",
  "title": "Diffusion Models in Vision: A Survey",
  "display_name": "Diffusion Models in Vision: A Survey",
  "publication_date": "2023-03-27",
  "publication_year": 2023,
  "authorships": [
    {"author": {"display_name": "Florinel-Alin Croitoru"}},
    {"author": {"display_name": "Vlad Hondru"}}
  ],
  "abstract_inverted_index": {
    "Diffusion": [0],
    "models": [1, 3],
    "are": [2],
    "popular": [4]
  },
  "primary_location": {
    "landing_page_url": "https://doi.org/10.1109/tpami.2023.3261988",
    "pdf_url": null,
    "source": {"display_name": "IEEE Transactions on Pattern Analysis and Machine Intelligence"}
  },
  "open_access": {"oa_url": "https://arxiv.org/pdf/2209.04747"},
  "locations": [
    {
      "landing_page_url": "http://arxiv.org/abs/2209.04747",
      "pdf_url": "https://arxiv.org/pdf/2209.04747",
      "source": {"display_name": "arXiv (Cornell University)"}
    }
  ],
  "primary_topic": {
    "display_name": "Generative Adversarial Networks and Image Synthesis",
    "subfield": {"display_name": "Computer Vision and Pattern Recognition"},
    "field": {"display_name": "Computer Science"}
  }
},
{
  "id": "https://openalex.org/W2",
  "display_name": "Year-only Work",
  "publication_year": 2020,
  "authorships": []
}
  ]
}"#;

#[test]
fn maps_complete_work() {
    let papers = parse_works(WORKS).expect("works should parse");
    assert_eq!(papers.len(), 2);

    let paper = &papers[0];
    assert_eq!(paper.id, "W4360884927");
    assert_eq!(paper.title, "Diffusion Models in Vision: A Survey");
    assert_eq!(paper.abstract_text, "Diffusion models are models popular");
    assert_eq!(paper.authors, ["Florinel-Alin Croitoru", "Vlad Hondru"]);
    assert_eq!(
        paper.categories,
        [
            "Generative Adversarial Networks and Image Synthesis",
            "Computer Vision and Pattern Recognition",
            "Computer Science"
        ]
    );
    assert_eq!(paper.published.year(), 2023);
    assert_eq!(paper.published.month(), 3);
    assert_eq!(paper.published.day(), 27);
    assert_eq!(paper.pdf_url, "https://arxiv.org/pdf/2209.04747");
    assert_eq!(
        paper.venue.as_deref(),
        Some("IEEE Transactions on Pattern Analysis and Machine Intelligence")
    );
    assert_eq!(paper.source, Source::OpenAlex);
}

#[test]
fn falls_back_to_year_and_openalex_url() {
    let papers = parse_works(WORKS).expect("works should parse");
    let paper = &papers[1];

    assert_eq!(paper.id, "W2");
    assert_eq!(paper.title, "Year-only Work");
    assert!(paper.abstract_text.is_empty());
    assert!(paper.categories.is_empty());
    assert!(paper.venue.is_none());
    assert_eq!(paper.published.year(), 2020);
    assert_eq!(paper.published.month(), 1);
    assert_eq!(paper.published.day(), 1);
    assert_eq!(paper.pdf_url, "https://openalex.org/W2");
}

#[test]
fn rejects_malformed_json() {
    assert!(matches!(parse_works("not json"), Err(Error::Parse(_))));
}

#[test]
fn feed_filter_joins_terms_and_caps_future_dates() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).expect("valid date");
    let filter = build_feed_filter(
        &[
            "large language models".into(),
            " diffusion models ".into(),
            "".into(),
        ],
        today,
    )
    .expect("filter should be built");

    assert_eq!(
        filter,
        "title_and_abstract.search:large language models|diffusion models,\
         to_publication_date:2026-10-20"
    );
}

#[test]
fn feed_filter_strips_separators() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).expect("valid date");
    let filter = build_feed_filter(&["a,b|c:d".into()], today).expect("filter should be built");

    assert_eq!(
        filter,
        "title_and_abstract.search:a b c d,to_publication_date:2026-10-20"
    );
}

#[test]
fn feed_filter_is_none_without_interests() {
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).expect("valid date");
    assert_eq!(build_feed_filter(&[], today), None);
    assert_eq!(build_feed_filter(&["  ".into()], today), None);
}

#[tokio::test]
#[ignore = "hits the live OpenAlex API"]
async fn live_search_returns_papers() {
    let client = OpenAlexHttpClient::new().expect("client should build");
    let papers = client
        .search("diffusion models", 3)
        .await
        .expect("live search should succeed");

    assert!(!papers.is_empty());
    assert!(papers.iter().all(|p| p.source == Source::OpenAlex));
    assert!(papers.iter().all(|p| !p.title.is_empty()));
}
