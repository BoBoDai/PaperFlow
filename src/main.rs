//! PaperFlow - 全天候语音学术助理
//!
//! 从 arXiv 获取论文并用语音播报的 Rust 桌面应用

use directories::ProjectDirs;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::{Settings, Commands};
use crate::commands::{ChatCommand, FetchCommand, ListCommand, SpeakCommand};
use crate::modules::storage::Database;

mod config;
mod core;
mod modules;
mod ui;
mod commands;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("paperflow=info".parse().unwrap()))
        .init();

    // 加载配置
    let settings = Settings::load()?;

    // 确定数据库路径
    let db_path = ProjectDirs::from("com", "paperflow", "PaperFlow")
        .map(|dirs| dirs.data_dir().join("paperflow.db").to_string_lossy().to_string())
        .unwrap_or_else(|| "paperflow.db".to_string());

    info!("数据库路径: {}", db_path);

    // 连接数据库
    let db = Database::new(&db_path).await?;

    // 处理子命令
    match &settings.cli.command {
        Some(Commands::Chat) | None => {
            // 默认启动交互式对话模式
            ChatCommand::run(&settings, &db).await?;
        }
        Some(Commands::Fetch) => {
            FetchCommand::run(&settings, &db).await?;
        }
        Some(Commands::List) => {
            ListCommand::run(&db, false).await?;
        }
        Some(Commands::Speak { paper_id: Some(id) }) => {
            let api_key = std::env::var("MINIMAX_API_KEY")
                .expect("请设置 MINIMAX_API_KEY 环境变量");
            SpeakCommand::run(&db, id.as_str(), &api_key).await?;
        }
        Some(Commands::Speak { paper_id: None }) => {
            let api_key = std::env::var("MINIMAX_API_KEY")
                .expect("请设置 MINIMAX_API_KEY 环境变量");
            SpeakCommand::run_all(&db, &api_key).await?;
        }
        Some(Commands::Tui) => {
            // 启动 TUI
            info!("启动 TUI 界面...");
            let mut app = ui::tui::TuiApp::new();
            app.set_papers(db.list_papers(50).await?);
            app.run().await?;
        }
    }

    Ok(())
}
