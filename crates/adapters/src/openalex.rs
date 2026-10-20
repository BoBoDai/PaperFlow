//! OpenAlex client.
//!
//! Used as the fallback search source when the arXiv API is rate-limited or
//! returns no results. OpenAlex is free and needs no API key:
//! <https://docs.openalex.org/api-entities/works/search-works>.
//!
//! Abstracts arrive as an inverted index (`word -> positions`) and are
//! reconstructed here.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use paperflow_core::{Error, Paper, Result, Source};
use serde::Deserialize;

use crate::search::{PaperSearch, request_error};

const OPENALEX_API: &str = "https://api.openalex.org/works";

const MAX_RESULTS: usize = 100;

/// Keep the payload small; nested objects are still returned in full.
const SELECT_FIELDS: &str = "id,title,display_name,publication_date,publication_year,\
     authorships,abstract_inverted_index,primary_location,open_access,locations,primary_topic";

/// HTTP-backed client for the OpenAlex Works API.
pub struct OpenAlexHttpClient {
    http: reqwest::Client,
    endpoint: String,
}

impl OpenAlexHttpClient {
    pub fn new() -> Result<Self> {
        Self::with_endpoint(OPENALEX_API)
    }

    /// Build a client against a custom endpoint. Useful for tests or mirrors.
    pub fn with_endpoint(endpoint: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("PaperFlow/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| Error::Network(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            http,
            endpoint: endpoint.into(),
        })
    }
}

impl PaperSearch for OpenAlexHttpClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, MAX_RESULTS).to_string();
        tracing::debug!(query, limit, "querying OpenAlex");

        let response = self
            .http
            .get(&self.endpoint)
            .query(&[
                ("search", query),
                ("per-page", limit.as_str()),
                ("select", SELECT_FIELDS),
            ])
            .send()
            .await
            .map_err(|e| request_error("OpenAlex request failed", e))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::Network(
                "OpenAlex 限流（HTTP 429），请稍后重试".into(),
            ));
        }

        let response = response
            .error_for_status()
            .map_err(|e| request_error("OpenAlex returned an error status", e))?;

        let body = response
            .text()
            .await
            .map_err(|e| request_error("failed to read OpenAlex response", e))?;

        parse_works(&body)
    }
}

/// Parse a Works API response into [`Paper`] values.
pub fn parse_works(json: &str) -> Result<Vec<Paper>> {
    let response: WorksResponse = serde_json::from_str(json)
        .map_err(|e| Error::Parse(format!("invalid OpenAlex response: {e}")))?;

    let mut papers = Vec::with_capacity(response.results.len());
    for work in response.results {
        match work_to_paper(work) {
            Ok(paper) => papers.push(paper),
            Err(reason) => tracing::warn!(reason, "skipping unusable OpenAlex work"),
        }
    }

    Ok(papers)
}

fn work_to_paper(work: Work) -> std::result::Result<Paper, String> {
    let id = work
        .id
        .rsplit('/')
        .next()
        .unwrap_or(&work.id)
        .to_string();

    if id.is_empty() {
        return Err("OpenAlex work has no id".into());
    }

    let title = normalize(
        work.title
            .as_deref()
            .or(work.display_name.as_deref())
            .unwrap_or_default(),
    );

    if title.is_empty() {
        return Err(format!("OpenAlex work {id} has no title"));
    }

    let published = publication_date(&work, &id)?;
    let pdf_url = pdf_url(&work, &id);
    let categories = categories(work.primary_topic);

    Ok(Paper {
        id,
        title,
        authors: work
            .authorships
            .into_iter()
            .filter_map(|authorship| authorship.author.display_name)
            .map(|name| normalize(&name))
            .filter(|name| !name.is_empty())
            .collect(),
        abstract_text: work
            .abstract_inverted_index
            .map(reconstruct_abstract)
            .unwrap_or_default(),
        categories,
        published,
        pdf_url,
        source: Source::OpenAlex,
        venue: work
            .primary_location
            .and_then(|location| location.source)
            .and_then(|source| source.display_name)
            .map(|name| normalize(&name))
            .filter(|name| !name.is_empty()),
    })
}

fn publication_date(work: &Work, id: &str) -> std::result::Result<DateTime<Utc>, String> {
    if let Some(raw) = &work.publication_date {
        if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
            return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
        }

        tracing::warn!(raw, id, "unparseable OpenAlex publication_date");
    }

    if let Some(year) = work.publication_year
        && let Some(date) = NaiveDate::from_ymd_opt(year, 1, 1)
    {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }

    Err(format!("OpenAlex work {id} has no usable publication date"))
}

fn pdf_url(work: &Work, id: &str) -> String {
    work.primary_location
        .as_ref()
        .and_then(|location| location.pdf_url.clone())
        .or_else(|| {
            work.open_access
                .as_ref()
                .and_then(|access| access.oa_url.clone())
        })
        .or_else(|| {
            work.locations
                .iter()
                .find_map(|location| location.pdf_url.clone())
        })
        .or_else(|| {
            work.locations
                .iter()
                .find_map(|location| location.landing_page_url.clone())
        })
        .or_else(|| {
            work.primary_location
                .as_ref()
                .and_then(|location| location.landing_page_url.clone())
        })
        .unwrap_or_else(|| format!("https://openalex.org/{id}"))
}

fn categories(topic: Option<Topic>) -> Vec<String> {
    let Some(topic) = topic else {
        return Vec::new();
    };

    let names = [
        topic.display_name,
        topic.subfield.and_then(|subfield| subfield.display_name),
        topic.field.and_then(|field| field.display_name),
    ];

    let mut categories = Vec::new();
    for name in names.into_iter().flatten() {
        let name = normalize(&name);
        if !name.is_empty() && !categories.contains(&name) {
            categories.push(name);
        }
    }

    categories
}

/// OpenAlex stores abstracts as `word -> word positions`.
fn reconstruct_abstract(index: HashMap<String, Vec<usize>>) -> String {
    let mut words: Vec<(usize, String)> = Vec::new();

    for (word, positions) in index {
        for position in positions {
            words.push((position, word.clone()));
        }
    }

    words.sort_by_key(|(position, _)| *position);
    words
        .into_iter()
        .map(|(_, word)| word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Deserialize)]
struct WorksResponse {
    #[serde(default)]
    results: Vec<Work>,
}

#[derive(Debug, Deserialize)]
struct Work {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    publication_date: Option<String>,
    #[serde(default)]
    publication_year: Option<i32>,
    #[serde(default)]
    authorships: Vec<Authorship>,
    #[serde(default)]
    abstract_inverted_index: Option<HashMap<String, Vec<usize>>>,
    #[serde(default)]
    primary_location: Option<Location>,
    #[serde(default)]
    open_access: Option<OpenAccess>,
    #[serde(default)]
    locations: Vec<Location>,
    #[serde(default)]
    primary_topic: Option<Topic>,
}

#[derive(Debug, Deserialize)]
struct Authorship {
    author: Author,
}

#[derive(Debug, Deserialize)]
struct Author {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Location {
    #[serde(default)]
    landing_page_url: Option<String>,
    #[serde(default)]
    pdf_url: Option<String>,
    #[serde(default)]
    source: Option<SourceInfo>,
}

#[derive(Debug, Deserialize)]
struct SourceInfo {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAccess {
    #[serde(default)]
    oa_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Topic {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    subfield: Option<TopicName>,
    #[serde(default)]
    field: Option<TopicName>,
}

#[derive(Debug, Deserialize)]
struct TopicName {
    #[serde(default)]
    display_name: Option<String>,
}

#[cfg(test)]
mod tests {
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
        assert_eq!(
            paper.abstract_text,
            "Diffusion models are models popular"
        );
        assert_eq!(
            paper.authors,
            ["Florinel-Alin Croitoru", "Vlad Hondru"]
        );
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
}
