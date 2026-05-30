//! API State

use crate::modules::storage::Database;
use crate::core::AppConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 共享的应用状态
#[derive(Clone)]
pub struct ApiState {
    pub db: Database,
    pub config: Arc<RwLock<ApiConfig>>,
    pub config_path: PathBuf,
}

/// 运行时 API 配置（从文件加载，可通过 API 修改）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiConfig {
    pub max_papers: usize,
    pub voice_speed: f64,
    pub api_key: Option<String>,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_base_url: String,
}

impl ApiConfig {
    /// 从 AppConfig（文件）创建
    pub fn from_app_config(app_config: &AppConfig) -> Self {
        Self {
            max_papers: app_config.max_papers.unwrap_or(5),
            voice_speed: app_config.voice_speed.unwrap_or(5.0),
            api_key: app_config.api_key.clone(),
            llm_provider: app_config.llm_provider.clone().unwrap_or_else(|| "openai".to_string()),
            llm_model: app_config.llm_model.clone().unwrap_or_else(|| "gpt-4o-mini".to_string()),
            llm_base_url: app_config.llm_base_url.clone().unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }

    /// 转换为 AppConfig 用于保存
    pub fn to_app_config(&self) -> AppConfig {
        AppConfig {
            api_key: self.api_key.clone(),
            llm_provider: Some(self.llm_provider.clone()),
            llm_model: Some(self.llm_model.clone()),
            llm_base_url: Some(self.llm_base_url.clone()),
            max_papers: Some(self.max_papers),
            voice_speed: Some(self.voice_speed),
        }
    }
}
