//! PaperFlow API Server
//!
//! Runs the REST API server for the PaperFlow Ink frontend

use directories::ProjectDirs;
use paperflow::api::{run_server, ApiState, state::ApiConfig};
use paperflow::modules::storage::Database;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("paperflow=info".parse().unwrap()))
        .init();

    // Database path
    let db_path = ProjectDirs::from("com", "paperflow", "PaperFlow")
        .map(|dirs| dirs.data_dir().join("paperflow.db").to_string_lossy().to_string())
        .unwrap_or_else(|| "paperflow.db".to_string());

    info!("数据库路径: {}", db_path);

    // Connect to database
    let db = Database::new(&db_path).await?;

    // Create API state
    let state = ApiState {
        db,
        config: Arc::new(RwLock::new(ApiConfig::default())),
    };

    // Run server
    run_server(state).await
}
