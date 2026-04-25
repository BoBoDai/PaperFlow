//! 配置加载和管理
//!
//! 优先级: CLI 参数 > 环境变量 > config.toml > 默认值

use clap::Parser;
use directories::ProjectDirs;
use std::path::PathBuf;

use crate::core::{Error, UserPreferences};

/// CLI 参数
#[derive(Debug, Parser, Clone)]
#[command(name = "paperflow")]
#[command(about = "全天候语音学术助理", long_about = None)]
pub struct CliArgs {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// 是否启用语音播报
    #[arg(short, long)]
    pub speak: bool,

    /// 抓取间隔（分钟）
    #[arg(short, long)]
    pub interval: Option<u64>,

    /// MiniMax API Key
    #[arg(long)]
    pub minimax_api_key: Option<String>,

    /// Groq API Key
    #[arg(long)]
    pub groq_api_key: Option<String>,

    /// LLM Provider
    #[arg(short, long, default_value = "minimax")]
    pub llm_provider: String,

    /// 语音识别 Provider
    #[arg(long, default_value = "groq_whisper")]
    pub speech_provider: String,

    /// 语音合成 Provider
    #[arg(long, default_value = "system_say")]
    pub synthesizer_provider: String,

    /// 关注的关键字（逗号分隔）
    #[arg(short, long)]
    pub keywords: Option<String>,

    /// 最大论文数
    #[arg(short, long, default_value = "5")]
    pub max_papers: usize,

    /// 子命令
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Commands {
    /// 交互式对话模式
    Chat,
    /// 抓取最新论文
    Fetch,
    /// 列出论文
    List,
    /// 语音播报
    Speak { paper_id: Option<String> },
    /// 启动 TUI 界面
    Tui,
}

/// 应用配置
#[derive(Debug, Clone)]
pub struct Settings {
    pub cli: CliArgs,
    pub preferences: UserPreferences,
}

impl Settings {
    /// 加载配置
    pub fn load() -> Result<Self, Error> {
        let cli = CliArgs::parse();

        // 从环境变量获取 API keys
        let minimax_api_key = cli.minimax_api_key.clone()
            .or_else(|| std::env::var("MINIMAX_API_KEY").ok());
        let groq_api_key = cli.groq_api_key.clone()
            .or_else(|| std::env::var("GROQ_API_KEY").ok());

        let preferences = UserPreferences {
            keywords: cli.keywords.as_ref()
                .map(|k| k.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            max_papers_per_fetch: cli.max_papers,
            ..Default::default()
        };

        Ok(Self {
            cli,
            preferences,
        })
    }

    /// 获取数据目录
    pub fn data_dir() -> Option<PathBuf> {
        ProjectDirs::from("com", "paperflow", "PaperFlow")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }
}
