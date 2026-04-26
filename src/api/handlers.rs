//! API Handlers

use super::state::{ApiConfig, ApiState};
use crate::modules::arxiv::ArxivClient;
use crate::modules::llm::{LlmConfig, LlmProvider, MiniMaxProvider};
use crate::modules::filter::Filter;
use crate::core::{UserPreferences, Paper};
use axum::{extract::State, Json};
use axum::extract::Query;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub max_results: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PaperResponse {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,
    pub published: String,
    pub categories: Vec<String>,
    pub pdf_url: String,
}

impl From<Paper> for PaperResponse {
    fn from(p: Paper) -> Self {
        Self {
            id: p.id,
            title: p.title,
            authors: p.authors,
            summary: p.abstract_text,
            published: p.published.format("%Y-%m-%d").to_string(),
            categories: p.categories,
            pdf_url: p.pdf_url,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub papers: Vec<PaperResponse>,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeRequest {
    pub paper_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct SummarizeResponse {
    pub short_summary: String,
    pub detailed_summary: String,
    pub key_points: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConfigRequest {
    pub max_papers: Option<usize>,
    pub voice_speed: Option<f64>,
    pub api_key: Option<String>,
}

/// Health check
pub async fn health() -> &'static str {
    "OK"
}

/// Search papers from arXiv
pub async fn search_papers(
    State(state): State<ApiState>,
    Query(query): Query<SearchQuery>,
) -> Json<SearchResponse> {
    let max = query.max_results.unwrap_or(5);
    let client = ArxivClient::new();

    match client.search(&query.q, max).await {
        Ok(papers) => {
            let config = state.config.read().await;
            let prefs = UserPreferences {
                keywords: vec![query.q.clone()],
                max_papers_per_fetch: max,
                ..Default::default()
            };
            let scored = Filter::filter_and_score(papers, &prefs);
            let responses: Vec<PaperResponse> = scored.into_iter().map(|p| p.into()).collect();
            Json(SearchResponse { papers: responses })
        }
        Err(_) => Json(SearchResponse { papers: vec![] }),
    }
}

/// List saved papers
pub async fn list_papers(State(state): State<ApiState>) -> Json<SearchResponse> {
    match state.db.list_papers(50).await {
        Ok(papers) => {
            let responses: Vec<PaperResponse> = papers.into_iter().map(|p| p.into()).collect();
            Json(SearchResponse { papers: responses })
        }
        Err(_) => Json(SearchResponse { papers: vec![] }),
    }
}

/// Generate summary for a paper
pub async fn summarize_paper(
    State(state): State<ApiState>,
    Json(req): Json<SummarizeRequest>,
) -> Json<SummarizeResponse> {
    let config = state.config.read().await;
    let api_key = match &config.api_key {
        Some(key) => key.clone(),
        None => {
            return Json(SummarizeResponse {
                short_summary: String::new(),
                detailed_summary: "API key not configured".to_string(),
                key_points: vec![],
            });
        }
    };

    let llm_config = LlmConfig {
        provider_type: "minimax".to_string(),
        api_key,
        model: "MiniMax-01".to_string(),
        base_url: Some("https://api.minimax.chat/v1".to_string()),
    };
    let llm = MiniMaxProvider::new(llm_config);

    let paper = Paper {
        id: req.paper_id,
        title: req.title,
        authors: req.authors,
        abstract_text: req.summary,
        categories: vec![],
        published: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        pdf_url: String::new(),
        relevance_score: None,
        summary: None,
        is_read: false,
    };

    match llm.summarize(&paper).await {
        Ok(summary) => Json(SummarizeResponse {
            short_summary: summary.short_summary,
            detailed_summary: summary.detailed_summary,
            key_points: summary.key_points,
        }),
        Err(_) => Json(SummarizeResponse {
            short_summary: String::new(),
            detailed_summary: "Failed to generate summary".to_string(),
            key_points: vec![],
        }),
    }
}

/// Get current config
pub async fn get_config(State(state): State<ApiState>) -> Json<ApiConfig> {
    let config = state.config.read().await;
    Json(config.clone())
}

/// Update config
pub async fn update_config(
    State(state): State<ApiState>,
    Json(req): Json<UpdateConfigRequest>,
) -> Json<ApiConfig> {
    let mut config = state.config.write().await;
    if let Some(max) = req.max_papers {
        config.max_papers = max;
    }
    if let Some(speed) = req.voice_speed {
        config.voice_speed = speed;
    }
    if let Some(key) = req.api_key {
        config.api_key = Some(key);
    }
    let result = config.clone();
    Json(result)
}
