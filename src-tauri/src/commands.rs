//! IPC command —— 前端的唯一入口。
//!
//! 每个 command 都是薄壳：取参数 → 调用 usecase → 返回。
//! **不许在这里写业务逻辑**，否则 Tauri 就被焊死进架构里了。
//!
//! 错误一律转成 String 跨 IPC，且**不做任何降级** —— 失败就让前端知道，
//! 不许返回空结果假装成功。

use crate::settings::LlmSettings;
use crate::AppState;
use paperflow_core::{Paper, Summary};
use tauri::State;

type IpcResult<T> = std::result::Result<T, String>;

fn to_ipc(e: paperflow_core::Error) -> String {
    tracing::error!(error = %e, "usecase 失败");
    e.to_string()
}

/// 检索论文
#[tauri::command]
pub async fn search_papers(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> IpcResult<Vec<Paper>> {
    state
        .app
        .search_papers(&query, limit.unwrap_or(10))
        .await
        .map_err(to_ipc)
}

/// 分析论文
#[tauri::command]
pub async fn summarize_paper(state: State<'_, AppState>, paper: Paper) -> IpcResult<Summary> {
    state.app.summarize(&paper).await.map_err(to_ipc)
}

/// 读取当前 LLM 设置（多服务商列表 + 当前使用的服务商）
#[tauri::command]
pub fn get_llm_settings(state: State<'_, AppState>) -> IpcResult<LlmSettings> {
    Ok(state.settings.current())
}

/// 保存 LLM 设置。API key 只写入本机配置文件，不写日志。
#[tauri::command]
pub fn save_llm_settings(
    state: State<'_, AppState>,
    settings: LlmSettings,
) -> IpcResult<LlmSettings> {
    let saved = state.settings.save(&settings)?;
    tracing::info!(
        providers = saved.providers.len(),
        active_id = %saved.active_id,
        "LLM settings updated"
    );
    Ok(saved)
}

/// 自检：前端可以用它确认 IPC 链路通不通
#[tauri::command]
pub fn health() -> String {
    tracing::info!("前端已连接 —— IPC 链路就绪");
    format!("paperflow {} 就绪", env!("CARGO_PKG_VERSION"))
}
