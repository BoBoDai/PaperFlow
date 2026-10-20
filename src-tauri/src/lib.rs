//! PaperFlow 桌面应用
//!
//! 这一层是**壳**：装配适配器、注册 IPC command、开窗口。
//! 业务逻辑一律在 `paperflow-usecases`，这里只做转接。

mod commands;

use paperflow_adapters::{ArxivHttpClient, OpenAlexHttpClient, StubLlmProvider};
use paperflow_usecases::App;

/// 跨 IPC 共享的应用上下文
pub struct AppState {
    pub app: App<ArxivHttpClient, StubLlmProvider, OpenAlexHttpClient>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "paperflow=debug,tauri=info".into()),
        )
        .init();

    // 适配器的装配点。接真实实现时，只改这两行。
    let arxiv = ArxivHttpClient::new().expect("初始化 arXiv 客户端失败");
    let openalex = OpenAlexHttpClient::new().expect("初始化 OpenAlex 客户端失败");
    let app = App::new(arxiv, StubLlmProvider, openalex);

    tauri::Builder::default()
        .manage(AppState { app })
        .invoke_handler(tauri::generate_handler![
            commands::search_papers,
            commands::summarize_paper,
            commands::health,
        ])
        .run(tauri::generate_context!())
        .expect("PaperFlow 启动失败");
}
