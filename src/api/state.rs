//! API State

use crate::modules::storage::Database;
use crate::core::UserPreferences;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ApiState {
    pub db: Database,
    pub config: Arc<RwLock<ApiConfig>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiConfig {
    pub max_papers: usize,
    pub voice_speed: f64,
    pub api_key: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_papers: 5,
            voice_speed: 5.0,
            api_key: std::env::var("MINIMAX_API_KEY").ok(),
        }
    }
}
