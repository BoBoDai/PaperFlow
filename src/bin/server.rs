//! PaperFlow API Server
//!
//! Runs the REST API server for the PaperFlow Ink frontend

use directories::ProjectDirs;
use paperflow::api::{run_server, ApiState, state::ApiConfig};
use paperflow::core::AppConfig;
use paperflow::modules::storage::Database;
use std::path::PathBuf;
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

    // Config file path
    let config_path = ProjectDirs::from("com", "paperflow", "PaperFlow")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    // Load config from file (env var overrides file)
    let app_config = AppConfig::load(&config_path);
    let api_config = ApiConfig::from_app_config(&app_config);

    info!("配置文件: {}", config_path.display());
    info!("API Key: {}", if api_config.api_key.is_some() { "已配置" } else { "未配置 (设置 MINIMAX_API_KEY 或编辑配置文件)" });

    // Save config back (creates file if it doesn't exist)
    if let Err(e) = app_config.save(&config_path) {
        tracing::warn!("无法保存配置文件: {}", e);
    }

    // Create API state
    let state = ApiState {
        db,
        config: Arc::new(RwLock::new(api_config)),
        config_path,
    };

    // Run server
    run_server(state).await
}
