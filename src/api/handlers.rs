//! API Handlers

use super::state::{ApiConfig, ApiState};
use crate::modules::arxiv::ArxivClient;
use crate::modules::llm::{LlmConfig, LlmProvider, MiniMaxProvider, OpenAiProvider};
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

#[derive(Debug, Deserialize)]
pub struct QuickSearchQuery {
    pub preset: String,
    pub max_results: Option<usize>,
}

/// 预设查询配置
struct PresetConfig {
    categories: Vec<&'static str>,
    label: &'static str,
}

fn get_preset(preset: &str) -> Option<PresetConfig> {
    match preset {
        "robotics" => Some(PresetConfig {
            categories: vec!["cs.RO", "cs.AI", "cs.CV", "cs.LG"],
            label: "机器人",
        }),
        "ai" => Some(PresetConfig {
            categories: vec!["cs.AI", "cs.LG", "cs.CL"],
            label: "AI/ML",
        }),
        "cv" => Some(PresetConfig {
            categories: vec!["cs.CV", "cs.AI"],
            label: "计算机视觉",
        }),
        _ => None,
    }
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
    pub source: String,
    pub venue: Option<String>,
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
            source: p.source,
            venue: p.venue,
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
            let _config = state.config.read().await;
            let prefs = UserPreferences {
                keywords: vec![query.q.clone()],
                max_papers_per_fetch: max,
                ..Default::default()
            };
            let scored = Filter::filter_and_score(papers, &prefs);
            let responses: Vec<PaperResponse> = scored.into_iter().map(|p| p.into()).collect();
            Json(SearchResponse { papers: responses })
        }
        Err(e) => {
            tracing::error!("搜索失败: {}", e);
            Json(SearchResponse { papers: vec![] })
        }
    }
}

/// Quick search by preset (robotics, ai, cv)
pub async fn quick_search(
    State(state): State<ApiState>,
    Query(query): Query<QuickSearchQuery>,
) -> Json<SearchResponse> {
    let max = query.max_results.unwrap_or(10);
    let per_category = (max / 4).max(2); // distribute across categories

    let preset = get_preset(&query.preset);
    if preset.is_none() {
        tracing::warn!("未知的预设: {}", query.preset);
        return Json(SearchResponse { papers: vec![] });
    }
    let preset = preset.unwrap();

    let client = ArxivClient::new();
    tracing::info!("快捷查询: {} → {:?}", query.preset, preset.categories);
    match client
        .search_multi_categories(&preset.categories, per_category)
        .await
    {
        Ok(papers) => {
            tracing::info!("快捷查询返回 {} 篇论文", papers.len());
            let prefs = UserPreferences {
                keywords: vec![query.preset.clone()],
                max_papers_per_fetch: max,
                ..Default::default()
            };
            let scored = Filter::filter_and_score(papers, &prefs);
            // Truncate to max
            let limited: Vec<Paper> = scored.into_iter().take(max).collect();
            let responses: Vec<PaperResponse> = limited.into_iter().map(|p| p.into()).collect();
            Json(SearchResponse { papers: responses })
        }
        Err(e) => {
            tracing::error!("快捷查询失败: {}", e);
            Json(SearchResponse { papers: vec![] })
        }
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
    if config.api_key.is_none() {
        return Json(SummarizeResponse {
            short_summary: String::new(),
            detailed_summary: "API key not configured".to_string(),
            key_points: vec![],
        });
    }

    let llm = create_llm(&config);

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
        source: "arxiv".to_string(),
        venue: None,
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

#[derive(Debug, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct TranslateResponse {
    pub translated: String,
    pub original: String,
    pub success: bool,
}

/// Create LLM provider based on config
fn create_llm(config: &ApiConfig) -> Box<dyn LlmProvider> {
    let llm_config = LlmConfig {
        provider_type: config.llm_provider.clone(),
        api_key: config.api_key.clone().unwrap_or_default(),
        model: config.llm_model.clone(),
        base_url: Some(config.llm_base_url.clone()),
    };

    if config.llm_provider == "minimax" {
        Box::new(MiniMaxProvider::new(llm_config))
    } else {
        Box::new(OpenAiProvider::new(llm_config))
    }
}

/// Translate Chinese queries to English for arXiv search
pub async fn translate_query(
    State(state): State<ApiState>,
    Json(req): Json<TranslateRequest>,
) -> Json<TranslateResponse> {
    let config = state.config.read().await;
    if config.api_key.is_none() {
        return Json(TranslateResponse {
            translated: req.text.clone(),
            original: req.text,
            success: false,
        });
    }

    let llm = create_llm(&config);

    let prompt = format!(
        "Translate this Chinese academic query into concise English keywords for searching academic papers. Return ONLY the English keywords, no explanation, no punctuation.\n\nChinese: {}",
        req.text
    );

    match llm.complete(&prompt).await {
        Ok(translated) => {
            // Strip <think>...</think> tags if present (MiniMax-M2 reasoning)
            let cleaned = translated
                .split("</think>")
                .last()
                .unwrap_or(&translated)
                .trim()
                .to_string();
            Json(TranslateResponse {
                translated: cleaned,
                original: req.text,
                success: true,
            })
        }
        Err(e) => {
            tracing::warn!("Translation failed: {}", e);
            Json(TranslateResponse {
                translated: req.text.clone(),
                original: req.text,
                success: false,
            })
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TranslatePaperRequest {
    pub title: String,
    pub abstract_text: String,
}

#[derive(Debug, Serialize)]
pub struct TranslatePaperResponse {
    pub title_cn: String,
    pub abstract_cn: String,
    pub success: bool,
}

/// Translate paper title and abstract to Chinese
pub async fn translate_paper(
    State(state): State<ApiState>,
    Json(req): Json<TranslatePaperRequest>,
) -> Json<TranslatePaperResponse> {
    let config = state.config.read().await;
    if config.api_key.is_none() {
        return Json(TranslatePaperResponse {
            title_cn: String::new(),
            abstract_cn: String::new(),
            success: false,
        });
    }

    let llm = create_llm(&config);

    let prompt = format!(
        "Translate the following academic paper title and abstract into Chinese. Keep technical terms accurate. Return in JSON format: {{\"title\": \"中文标题\", \"abstract\": \"中文摘要\"}}\n\nTitle: {}\n\nAbstract: {}",
        req.title,
        req.abstract_text
    );

    match llm.complete(&prompt).await {
        Ok(response) => {
            let cleaned = response.split("</think>").last().unwrap_or(&response);
            let title_cn = extract_json_field(cleaned, "title");
            let abstract_cn = extract_json_field(cleaned, "abstract");

            Json(TranslatePaperResponse {
                title_cn,
                abstract_cn,
                success: true,
            })
        }
        Err(e) => {
            tracing::warn!("Paper translation failed: {}", e);
            Json(TranslatePaperResponse {
                title_cn: String::new(),
                abstract_cn: String::new(),
                success: false,
            })
        }
    }
}

/// Extract a JSON field value from LLM response (handles imperfect JSON)
fn extract_json_field(text: &str, field: &str) -> String {
    // Try to find "{field}": "value" pattern
    let pattern = format!("\"{}\"", field);
    if let Some(start) = text.find(&pattern) {
        let after_key = &text[start + pattern.len()..];
        // Find the colon and value
        if let Some(colon_pos) = after_key.find(':') {
            let after_colon = after_key[colon_pos + 1..].trim();
            // Extract quoted string
            if let Some(quote_start) = after_colon.find('"') {
                let in_quotes = &after_colon[quote_start + 1..];
                if let Some(quote_end) = in_quotes.find('"') {
                    return in_quotes[..quote_end].to_string();
                }
            }
            // No quotes, take until comma or brace
            let end = after_colon.find(|c| c == ',' || c == '}').unwrap_or(after_colon.len());
            return after_colon[..end].trim().trim_matches('"').to_string();
        }
    }
    // Fallback: return the whole cleaned text (for title)
    if field == "title" {
        text.lines().next().unwrap_or("").trim().trim_matches('"').to_string()
    } else {
        String::new()
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

    // Persist to config file
    let app_config = config.to_app_config();
    if let Err(e) = app_config.save(&state.config_path) {
        tracing::error!("保存配置失败: {}", e);
    }

    let result = config.clone();
    Json(result)
}
