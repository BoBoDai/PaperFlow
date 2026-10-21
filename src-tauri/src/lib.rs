//! PaperFlow 桌面应用
//!
//! 这一层是**壳**：装配适配器、注册 IPC command、开窗口。
//! 业务逻辑一律在 `paperflow-usecases`，这里只做转接。

mod commands;
mod credentials;
mod settings;

use paperflow_adapters::{
    ArxivHttpClient, OpenAiCompatibleClient, OpenAlexHttpClient, PdfFetcher,
};
use paperflow_usecases::App;
use settings::SettingsManager;
use tauri::Manager;

/// 跨 IPC 共享的应用上下文
pub struct AppState {
    pub app: App<ArxivHttpClient, OpenAiCompatibleClient, OpenAlexHttpClient>,
    pub settings: SettingsManager,
    pub pdf: PdfFetcher,
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

            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|e| e.to_string())?;
            let settings = SettingsManager::load(config_dir);

            let pdf_cache_dir = app
                .path()
                .app_cache_dir()
                .map_err(|e| e.to_string())?
                .join("pdfs");
            let pdf = PdfFetcher::new(pdf_cache_dir).map_err(|e| e.to_string())?;
            let llm = OpenAiCompatibleClient::new(settings.shared())
                .map_err(|e| e.to_string())?;

            app.manage(AppState {
                app: App::new(arxiv, llm, openalex),
                settings,
                pdf,
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
            commands::get_llm_provider_catalog,
            commands::fetch_llm_models,
            commands::test_llm_connection,
            commands::ask_paper,
            commands::download_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("PaperFlow 启动失败");
}
