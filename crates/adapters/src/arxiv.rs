//! arXiv client.
//!
//! Talks to the public arXiv Atom API:
//! <https://info.arxiv.org/help/api/user-manual.html>.
//!
//! This adapter stays thin: HTTP request in, Atom XML out, each `<entry>`
//! mapped to a [`Paper`]. A malformed or error response is surfaced as
//! [`Error::Parse`] rather than an empty result — the same
//! no-silent-degradation rule that applies to LLM output.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use paperflow_core::{Error, Paper, Result, Source};
use quick_xml::de::from_str;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::search::{PaperSearch, request_error};

const ARXIV_API: &str = "https://export.arxiv.org/api/query";

/// arXiv caps the result page size; stay well inside the API limit.
const MAX_RESULTS: usize = 100;

/// arXiv asks clients to make at most one request every three seconds.
const MIN_REQUEST_INTERVAL: Duration = Duration::from_secs(3);

/// HTTP-backed client for the arXiv Atom API.
pub struct ArxivHttpClient {
    http: reqwest::Client,
    endpoint: String,
    last_request: Mutex<Option<Instant>>,
}

impl ArxivHttpClient {
    pub fn new() -> Result<Self> {
        Self::with_endpoint(ARXIV_API)
    }

    /// Build a client against a custom endpoint. Useful for tests or mirrors.
    pub fn with_endpoint(endpoint: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("PaperFlow/", env!("CARGO_PKG_VERSION")))
            // arXiv normally answers in under a second. Keep this short so a
            // dead route does not delay the OpenAlex fallback.
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Network(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            http,
            endpoint: endpoint.into(),
            last_request: Mutex::new(None),
        })
    }

    /// Space requests out; arXiv returns HTTP 429 if clients call it faster
    /// than once every three seconds.
    async fn throttle(&self) {
        let mut last_request = self.last_request.lock().await;

        if let Some(previous) = *last_request {
            let elapsed = previous.elapsed();
            if elapsed < MIN_REQUEST_INTERVAL {
                tokio::time::sleep(MIN_REQUEST_INTERVAL - elapsed).await;
            }
        }

        *last_request = Some(Instant::now());
    }
}

impl PaperSearch for ArxivHttpClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Quote the free-text query so multi-word phrases stay together.
        let search_query = format!("all:\"{}\"", query.replace('"', " "));
        self.fetch(&search_query, 0, limit, false).await
    }

    async fn feed(&self, interests: &[String], offset: usize, limit: usize) -> Result<Vec<Paper>> {
        let Some(search_query) = build_feed_query(interests) else {
            return Ok(Vec::new());
        };

        self.fetch(&search_query, offset, limit, true).await
    }
}

impl ArxivHttpClient {
    async fn fetch(
        &self,
        search_query: &str,
        offset: usize,
        limit: usize,
        newest_first: bool,
    ) -> Result<Vec<Paper>> {
        let limit_num = limit.clamp(1, MAX_RESULTS);

        tracing::debug!(
            search_query,
            offset,
            limit = limit_num,
            newest_first,
            "querying arXiv"
        );

        let mut params = vec![
            ("search_query", search_query.to_string()),
            ("start", offset.to_string()),
            ("max_results", limit_num.to_string()),
        ];

        if newest_first {
            params.push(("sortBy", "submittedDate".to_string()));
            params.push(("sortOrder", "descending".to_string()));
        }

        self.throttle().await;

        let response = self
            .http
            .get(&self.endpoint)
            .query(&params)
            .send()
            .await
            .map_err(|e| request_error("arXiv request failed", e))?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::Network(
                "arXiv 限流（HTTP 429）：当前网络出口可能被 arXiv 限制，\
                 请稍后重试或切换代理节点"
                    .into(),
            ));
        }

        let response = response
            .error_for_status()
            .map_err(|e| request_error("arXiv returned an error status", e))?;

        let body = response
            .text()
            .await
            .map_err(|e| request_error("failed to read arXiv response", e))?;

        parse_feed(&body)
    }
}

fn build_feed_query(interests: &[String]) -> Option<String> {
    let terms: Vec<String> = interests
        .iter()
        .map(|interest| sanitize_term(interest))
        .filter(|interest| !interest.is_empty())
        .map(|interest| format!("all:\"{interest}\""))
        .collect();

    match terms.len() {
        0 => None,
        1 => Some(terms[0].clone()),
        _ => Some(format!("({})", terms.join(" OR "))),
    }
}

fn sanitize_term(raw: &str) -> String {
    raw.replace(['"', '\\'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse an Atom feed returned by the arXiv API.
pub fn parse_feed(xml: &str) -> Result<Vec<Paper>> {
    let feed: AtomFeed =
        from_str(xml).map_err(|e| Error::Parse(format!("invalid arXiv Atom feed: {e}")))?;

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
mod tests;
