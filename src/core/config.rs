//! 配置文件管理
//!
//! API Key 读取优先级（从高到低）:
//!   1. 环境变量 MINIMAX_API_KEY
//!   2. 项目目录下的 api-key 文件
//!   3. ~/.config/paperflow/config.toml

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 可持久化的应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_papers: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_speed: Option<f64>,
}

impl AppConfig {
    /// 从配置文件加载，如果文件不存在则返回默认值
    pub fn load(config_path: &PathBuf) -> Self {
        let mut config = match std::fs::read_to_string(config_path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        };

        // 从项目目录的 api-key 文件读取配置（.env 格式，覆盖 config.toml）
        if let Ok(cwd) = std::env::current_dir() {
            let api_key_file = cwd.join("api-key");
            if let Ok(content) = std::fs::read_to_string(&api_key_file) {
                let api_config = Self::parse_api_key_file(&content);
                // api-key 中的值覆盖 config.toml 中的值
                if api_config.api_key.is_some() {
                    config.api_key = api_config.api_key;
                }
                if api_config.llm_base_url.is_some() {
                    config.llm_base_url = api_config.llm_base_url;
                }
                if api_config.llm_model.is_some() {
                    config.llm_model = api_config.llm_model;
                }
                if api_config.llm_provider.is_some() {
                    config.llm_provider = api_config.llm_provider;
                }
            }
        }

        // 环境变量优先级最高
        if let Ok(env_key) = std::env::var("MINIMAX_API_KEY") {
            if !env_key.is_empty() {
                config.api_key = Some(env_key);
            }
        }

        config
    }

    /// 保存到配置文件
    pub fn save(&self, config_path: &PathBuf) -> anyhow::Result<()> {
        // 确保父目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        tracing::info!("配置已保存至: {}", config_path.display());
        Ok(())
    }

    /// 解析 api-key 文件（支持 .env 格式和纯文本 key）
    fn parse_api_key_file(content: &str) -> Self {
        let mut config = Self {
            api_key: None,
            llm_provider: None,
            llm_model: None,
            llm_base_url: None,
            max_papers: None,
            voice_speed: None,
        };

        for line in content.lines() {
            let trimmed = line.trim();

            // 跳过空行和注释
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // 去掉开头的 "export " 前缀
            let line = trimmed.strip_prefix("export ").unwrap_or(trimmed);

            // KEY=VALUE 格式
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if value.is_empty() || value.contains("${") {
                    // 跳过空值和未展开的变量
                    continue;
                }
                match key.to_uppercase().as_str() {
                    "OPENAI_API_KEY" | "MINIMAX_API_KEY" | "API_KEY" => {
                        config.api_key = Some(value.to_string());
                    }
                    "OPENAI_BASE_URL" | "BASE_URL" => {
                        config.llm_base_url = Some(value.to_string());
                    }
                    "LLM_MODEL" | "MODEL" => {
                        config.llm_model = Some(value.to_string());
                    }
                    "LLM_PROVIDER" | "PROVIDER" => {
                        config.llm_provider = Some(value.to_string());
                    }
                    _ => {}
                }
            } else if !trimmed.starts_with("export ") {
                // 纯文本 key（没有 = 号），作为 API key
                if config.api_key.is_none() {
                    config.api_key = Some(trimmed.to_string());
                }
            }
        }

        config
    }

    /// 合并另一个配置（用于从环境变量或 API 更新）
    pub fn merge(&mut self, other: &AppConfig) {
        if other.api_key.is_some() {
            self.api_key = other.api_key.clone();
        }
        if other.max_papers.is_some() {
            self.max_papers = other.max_papers;
        }
        if other.voice_speed.is_some() {
            self.voice_speed = other.voice_speed;
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        // Key: try OPENAI_API_KEY first, then MINIMAX_API_KEY
        let key = std::env::var("OPENAI_API_KEY").ok()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok());

        // Base URL: try OPENAI_BASE_URL first
        let base_url = std::env::var("OPENAI_BASE_URL").ok()
            .unwrap_or_else(|| "https://api.openai.com".to_string());

        // Provider: respect MINIMAX_API_KEY backward compat
        let provider = if std::env::var("MINIMAX_API_KEY").is_ok() && std::env::var("OPENAI_API_KEY").is_err() {
            "minimax".to_string()
        } else {
            "openai".to_string()
        };

        let model = if provider == "minimax" {
            "abab6.5s-chat".to_string()
        } else {
            "gpt-4o-mini".to_string()
        };

        Self {
            api_key: key,
            llm_provider: Some(provider),
            llm_model: Some(model),
            llm_base_url: Some(base_url),
            max_papers: Some(5),
            voice_speed: Some(5.0),
        }
    }
}
