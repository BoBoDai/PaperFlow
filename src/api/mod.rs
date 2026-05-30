//! API Server - REST API for PaperFlow
//!
//! Runs on http://localhost:8080

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

mod handlers;
pub mod state;

pub use state::ApiState;

pub async fn run_server(state: ApiState) -> anyhow::Result<()> {
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/api/search", get(handlers::search_papers))
        .route("/api/quick-search", get(handlers::quick_search))
        .route("/api/translate", post(handlers::translate_query))
        .route("/api/translate-paper", post(handlers::translate_paper))
        .route("/api/papers", get(handlers::list_papers))
        .route("/api/summarize", post(handlers::summarize_paper))
        .route("/api/config", get(handlers::get_config))
        .route("/api/config", post(handlers::update_config))
        .route("/health", get(handlers::health))
        .layer(cors)
        .with_state(state);

    let addr = "127.0.0.1:8080";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("  API 服务已启动: http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
