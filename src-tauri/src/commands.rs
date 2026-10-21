//! IPC command —— 前端的唯一入口。
//!
//! 每个 command 都是薄壳：取参数 → 调用 usecase → 返回。
//! **不许在这里写业务逻辑**，否则 Tauri 就被焊死进架构里了。
//!
//! 错误一律转成 String 跨 IPC，且**不做任何降级** —— 失败就让前端知道，
//! 不许返回空结果假装成功。

use crate::settings::{LlmProbeInput, LlmSettingsInput, LlmSettingsView};
use crate::AppState;
use paperflow_adapters::{
    provider_catalog, ConnectionTestResult, LlmEndpointError, LlmErrorKind, LlmModelList,
    ProviderProfileInfo,
};
use paperflow_core::{FeedPage, Paper, QaRequest, QaResponse, SearchResponse, Summary};
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
) -> IpcResult<SearchResponse> {
    state
        .app
        .search_papers(&query, limit.unwrap_or(10))
        .await
        .map_err(to_ipc)
}

/// 兴趣信息流：按最新发表时间返回一页论文
#[tauri::command]
pub async fn browse_feed(
    state: State<'_, AppState>,
    interests: Vec<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> IpcResult<FeedPage> {
    state
        .app
        .browse_feed(
            &interests,
            offset.unwrap_or(0),
            limit.unwrap_or(paperflow_usecases::FEED_DEFAULT_LIMIT),
        )
        .await
        .map_err(to_ipc)
}

/// 分析论文
#[tauri::command]
pub async fn summarize_paper(state: State<'_, AppState>, paper: Paper) -> IpcResult<Summary> {
    state.app.summarize(&paper).await.map_err(to_ipc)
}

/// 流式论文分析：增量通过 Channel 推送，结构化摘要作为命令返回值。
#[tauri::command]
pub async fn summarize_paper_stream(
    state: State<'_, AppState>,
    paper: Paper,
    on_event: tauri::ipc::Channel<String>,
) -> IpcResult<Summary> {
    let channel = on_event.clone();

    state
        .app
        .summarize_stream(&paper, move |delta| {
            let _ = channel.send(delta);
        })
        .await
        .map_err(to_ipc)
}

/// 基于 PDF 上下文回答论文问题
#[tauri::command]
pub async fn ask_paper(
    state: State<'_, AppState>,
    request: QaRequest,
) -> IpcResult<QaResponse> {
    state.app.ask_paper(request).await.map_err(to_ipc)
}

/// 流式问答：增量通过 Channel 推送，完整响应作为命令返回值。
#[tauri::command]
pub async fn ask_paper_stream(
    state: State<'_, AppState>,
    request: QaRequest,
    on_event: tauri::ipc::Channel<String>,
) -> IpcResult<QaResponse> {
    let channel = on_event.clone();

    state
        .app
        .ask_paper_stream(request, move |delta| {
            let _ = channel.send(delta);
        })
        .await
        .map_err(to_ipc)
}

/// 下载 PDF，返回原始字节（前端收到 ArrayBuffer）
#[tauri::command]
pub async fn download_pdf(
    state: State<'_, AppState>,
    url: String,
) -> IpcResult<tauri::ipc::Response> {
    let bytes = state.pdf.fetch(&url).await.map_err(to_ipc)?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// 读取当前 LLM 设置（不含 API key）
#[tauri::command]
pub fn get_llm_settings(state: State<'_, AppState>) -> IpcResult<LlmSettingsView> {
    Ok(state.settings.current_view())
}

/// 保存 LLM 设置。API key 只写入本机凭据文件，不写日志、不回传。
#[tauri::command]
pub fn save_llm_settings(
    state: State<'_, AppState>,
    settings: LlmSettingsInput,
) -> IpcResult<LlmSettingsView> {
    let saved = state.settings.save(&settings)?;
    tracing::info!(
        providers = saved.providers.len(),
        active_id = %saved.active_id,
        "LLM settings updated"
    );
    Ok(saved)
}

/// 内置 provider 预设目录
#[tauri::command]
pub fn get_llm_provider_catalog() -> Vec<ProviderProfileInfo> {
    provider_catalog()
}

fn missing_config(message: String) -> LlmEndpointError {
    LlmEndpointError {
        kind: LlmErrorKind::MissingConfig,
        message,
        status: None,
        request_id: None,
        provider_message: None,
    }
}

/// 拉取 provider 可用模型；key 留空时使用已保存的 key
#[tauri::command]
pub async fn fetch_llm_models(
    state: State<'_, AppState>,
    input: LlmProbeInput,
) -> std::result::Result<LlmModelList, LlmEndpointError> {
    let config = state
        .settings
        .resolve_probe(&input)
        .map_err(missing_config)?;

    state.app.llm.fetch_models(&config).await
}

/// 测试 provider 连接（endpoint + auth + model）
#[tauri::command]
pub async fn test_llm_connection(
    state: State<'_, AppState>,
    input: LlmProbeInput,
) -> std::result::Result<ConnectionTestResult, LlmEndpointError> {
    let config = state
        .settings
        .resolve_probe(&input)
        .map_err(missing_config)?;

    state.app.llm.test_connection(&config).await
}

/// 自检：前端可以用它确认 IPC 链路通不通
#[tauri::command]
pub fn health() -> String {
    tracing::info!("前端已连接 —— IPC 链路就绪");
    format!("paperflow {} 就绪", env!("CARGO_PKG_VERSION"))
}
