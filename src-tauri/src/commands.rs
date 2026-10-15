//! IPC command —— 前端的唯一入口。
//!
//! 每个 command 都是薄壳：取参数 → 调用 usecase → 返回。
//! **不许在这里写业务逻辑**，否则 Tauri 就被焊死进架构里了。
//!
//! 错误一律转成 String 跨 IPC，且**不做任何降级** —— 失败就让前端知道，
//! 不许返回空结果假装成功。

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

/// 自检：前端可以用它确认 IPC 链路通不通
#[tauri::command]
pub fn health() -> String {
    format!("paperflow {} 就绪", env!("CARGO_PKG_VERSION"))
}
