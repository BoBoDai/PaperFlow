//! Semantic Scholar API 客户端
//!
//! 免费 API，无需认证，速率限制 100 req/5min
//! API: https://api.semanticscholar.org/graph/v1/paper/search

use chrono::{DateTime, Utc, TimeZone};
use reqwest::Client;
use serde::Deserialize;

use crate::core::{Error, Paper, Result};

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: Vec<PaperData>,
}

#[derive(Debug, Deserialize)]
struct PaperData {
    #[serde(rename = "paperId")]
    paper_id: String,
    title: Option<String>,
    #[serde(default)]
    authors: Vec<AuthorData>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    year: Option<i32>,
    venue: Option<String>,
    #[serde(rename = "externalIds")]
    external_ids: Option<ExternalIds>,
    url: Option<String>,
    #[serde(rename = "citationCount")]
    citation_count: Option<i32>,
    #[serde(rename = "publicationDate")]
    publication_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorData {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ExternalIds {
    #[serde(rename = "ArXiv")]
    arxiv: Option<String>,
    #[serde(rename = "DOI")]
    doi: Option<String>,
}

#[derive(Clone)]
pub struct SemanticScholarClient {
    http_client: Client,
    base_url: String,
}

impl SemanticScholarClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::builder().no_proxy().build().unwrap_or_else(|_| Client::new()),
            base_url: "https://api.semanticscholar.org/graph/v1".to_string(),
        }
    }

    /// Search papers by query, with optional venue filter
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        venue_filter: Option<&[&str]>,
    ) -> Result<Vec<Paper>> {
        let fields = "title,authors,abstract,year,venue,externalIds,url,citationCount,publicationDate";
        let url = format!(
            "{}/paper/search?query={}&limit={}&fields={}",
            self.base_url, query, limit.min(100), fields
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(format!("Semantic Scholar: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(Error::Network(format!("Semantic Scholar HTTP {}", status.as_u16())));
        }

        let body: SearchResponse = response.json().await
            .map_err(|e| Error::Parse(format!("Semantic Scholar parse: {}", e)))?;

        let papers: Vec<Paper> = body.data.into_iter()
            .filter_map(|d| self.convert(d))
            .collect();

        // Apply venue filter if specified
        let filtered = if let Some(venues) = venue_filter {
            papers.into_iter()
                .filter(|p| {
                    if let Some(ref v) = p.venue {
                        venues.iter().any(|vf| v.to_lowercase().contains(&vf.to_lowercase()))
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            papers
        };

        Ok(filtered)
    }

    /// Search for recent papers in robotics venues
    pub async fn search_robotics_venues(&self, limit: usize) -> Result<Vec<Paper>> {
        let venues = &["ICRA", "IROS", "RSS", "CoRL", "Robotics", "Science Robotics", "T-RO", "IJRR", "RAL"][..];
        self.search("robotics", limit, Some(venues)).await
    }

    fn convert(&self, d: PaperData) -> Option<Paper> {
        let title = d.title?.trim().to_string();
        if title.is_empty() { return None; }

        let id = d.external_ids.as_ref()
            .and_then(|e| e.arxiv.clone())
            .unwrap_or_else(|| d.paper_id.clone());

        let authors: Vec<String> = d.authors.into_iter()
            .map(|a| a.name)
            .collect();

        let abstract_text = d.abstract_text.unwrap_or_default();

        let published = if let Some(ref date_str) = d.publication_date {
            DateTime::parse_from_rfc3339(date_str)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        } else if let Some(year) = d.year {
            Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).single()
        } else {
            None
        }.unwrap_or_else(Utc::now);

        let pdf_url = d.url.unwrap_or_else(|| {
            if id.starts_with("arXiv:") || id.contains('.') {
                format!("https://arxiv.org/abs/{}", id)
            } else {
                format!("https://api.semanticscholar.org/{}", id)
            }
        });

        let source_venue = d.venue.clone();

        Some(Paper {
            id,
            title,
            authors,
            abstract_text,
            categories: vec![],
            published,
            updated: published,
            pdf_url,
            relevance_score: d.citation_count.map(|c| (c as f64).min(10.0)),
            summary: None,
            source: "semantic_scholar".to_string(),
            venue: source_venue,
            is_read: false,
        })
    }
}

impl Default for SemanticScholarClient {
    fn default() -> Self {
        Self::new()
    }
}
