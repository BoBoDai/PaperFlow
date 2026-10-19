//! arXiv client.
//!
//! Talks to the public arXiv Atom API:
//! <https://info.arxiv.org/help/api/user-manual.html>.
//!
//! This adapter stays thin: HTTP request in, Atom XML out, each `<entry>`
//! mapped to a [`Paper`]. A malformed or error response is surfaced as
//! [`Error::Parse`] rather than an empty result — the same
//! no-silent-degradation rule that applies to LLM output.

use std::time::Duration;

use chrono::{DateTime, Utc};
use paperflow_core::{Error, Paper, Result, Source};
use quick_xml::de::from_str;
use serde::Deserialize;

const ARXIV_API: &str = "https://export.arxiv.org/api/query";

/// arXiv caps the result page size; stay well inside the API limit.
const MAX_RESULTS: usize = 100;

/// Search papers by keyword.
#[allow(async_fn_in_trait)]
pub trait ArxivClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>>;
}

/// HTTP-backed client for the arXiv Atom API.
pub struct ArxivHttpClient {
    http: reqwest::Client,
    endpoint: String,
}

impl ArxivHttpClient {
    pub fn new() -> Result<Self> {
        Self::with_endpoint(ARXIV_API)
    }

    /// Build a client against a custom endpoint. Useful for tests or mirrors.
    pub fn with_endpoint(endpoint: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "PaperFlow/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/BoBoDai/PaperFlow)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Network(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            http,
            endpoint: endpoint.into(),
        })
    }
}

impl ArxivClient for ArxivHttpClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, MAX_RESULTS).to_string();
        // Quote the free-text query so multi-word phrases stay together.
        let search_query = format!("all:\"{}\"", query.replace('"', " "));
        tracing::debug!(search_query, limit, "querying arXiv");

        let response = self
            .http
            .get(&self.endpoint)
            .query(&[
                ("search_query", search_query.as_str()),
                ("start", "0"),
                ("max_results", limit.as_str()),
            ])
            .send()
            .await
            .map_err(|e| Error::Network(format!("arXiv request failed: {e}")))?
            .error_for_status()
            .map_err(|e| Error::Network(format!("arXiv returned an error status: {e}")))?;

        let body = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("failed to read arXiv response: {e}")))?;

        parse_feed(&body)
    }
}

/// Parse an Atom feed returned by the arXiv API.
pub fn parse_feed(xml: &str) -> Result<Vec<Paper>> {
    let feed: AtomFeed = from_str(xml)
        .map_err(|e| Error::Parse(format!("invalid arXiv Atom feed: {e}")))?;

    // arXiv reports API-level failures as a normal HTTP 200 entry whose title
    // is literally "Error"; the summary carries the reason.
    if let Some(error) = feed
        .entries
        .iter()
        .find(|entry| normalize(&entry.title).eq_ignore_ascii_case("error"))
    {
        return Err(Error::Parse(format!(
            "arXiv API error: {}",
            normalize(&error.summary)
        )));
    }

    feed.entries.into_iter().map(entry_to_paper).collect()
}

fn entry_to_paper(entry: AtomEntry) -> Result<Paper> {
    let id = arxiv_id(&entry.id);

    let title = normalize(&entry.title);
    if title.is_empty() {
        return Err(Error::Parse(format!("arXiv entry {id} has no title")));
    }

    let published = DateTime::parse_from_rfc3339(entry.published.trim())
        .map_err(|e| Error::Parse(format!("arXiv entry {id} has an invalid date: {e}")))?
        .with_timezone(&Utc);

    let pdf_url = entry
        .links
        .iter()
        .find(|link| link.title.as_deref() == Some("pdf") || link.href.contains("/pdf/"))
        .map(|link| link.href.clone())
        .unwrap_or_else(|| format!("https://arxiv.org/pdf/{id}"));

    Ok(Paper {
        id,
        title,
        authors: entry
            .authors
            .into_iter()
            .map(|author| normalize(&author.name))
            .filter(|name| !name.is_empty())
            .collect(),
        abstract_text: normalize(&entry.summary),
        categories: entry
            .categories
            .into_iter()
            .map(|category| category.term)
            .collect(),
        published,
        pdf_url,
        source: Source::Arxiv,
        venue: None,
    })
}

/// Extract a stable arXiv id from an Atom `<id>` URL.
///
/// Drops the version suffix so the same paper keeps one id across revisions.
fn arxiv_id(raw: &str) -> String {
    let id = raw.split("/abs/").nth(1).unwrap_or(raw).trim();

    match id.rsplit_once('v') {
        Some((base, version))
            if !base.is_empty()
                && !version.is_empty()
                && version.chars().all(|c| c.is_ascii_digit()) =>
        {
            base.to_string()
        }
        _ => id.to_string(),
    }
}

/// Atom text nodes keep the line breaks used to wrap long titles/abstracts.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Deserialize)]
struct AtomFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<AtomEntry>,
}

#[derive(Debug, Deserialize)]
struct AtomEntry {
    id: String,
    title: String,
    summary: String,
    published: String,
    #[serde(rename = "author", default)]
    authors: Vec<AtomAuthor>,
    #[serde(rename = "category", default)]
    categories: Vec<AtomCategory>,
    #[serde(rename = "link", default)]
    links: Vec<AtomLink>,
}

#[derive(Debug, Deserialize)]
struct AtomAuthor {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AtomCategory {
    #[serde(rename = "@term")]
    term: String,
}

#[derive(Debug, Deserialize)]
struct AtomLink {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@title", default)]
    title: Option<String>,
}

#[cfg(test)]
mod tests {
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
}
