//! PaperFlow 桌面应用
//!
//! 这一层是**壳**：装配适配器、注册 IPC command、开窗口。
//! 业务逻辑一律在 `paperflow-usecases`，这里只做转接。

mod commands;
mod settings;

use paperflow_adapters::{ArxivHttpClient, OpenAiCompatibleClient, OpenAlexHttpClient};
use paperflow_usecases::App;
use settings::SettingsManager;
use tauri::Manager;

/// 跨 IPC 共享的应用上下文
pub struct AppState {
    pub app: App<ArxivHttpClient, OpenAiCompatibleClient, OpenAlexHttpClient>,
    pub settings: SettingsManager,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "paperflow=debug,tauri=info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            // 适配器的装配点。
            let arxiv = ArxivHttpClient::new().map_err(|e| e.to_string())?;
            let openalex = OpenAlexHttpClient::new().map_err(|e| e.to_string())?;

            let settings_path = app
                .path()
                .app_config_dir()
                .map_err(|e| e.to_string())?
                .join("settings.json");
            let settings = SettingsManager::load(settings_path);
            let llm = OpenAiCompatibleClient::new(settings.shared())
                .map_err(|e| e.to_string())?;

            app.manage(AppState {
                app: App::new(arxiv, llm, openalex),
                settings,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_papers,
            commands::browse_feed,
            commands::summarize_paper,
            commands::health,
            commands::get_llm_settings,
            commands::save_llm_settings,
        ])
        .run(tauri::generate_context!())
        .expect("PaperFlow 启动失败");
}
