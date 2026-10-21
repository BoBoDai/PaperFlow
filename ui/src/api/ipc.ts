/**
 * IPC 封装 —— 前端与 Rust 之间的唯一通道。
 *
 * 组件不直接调 `invoke`，都走这里。将来若要加 Web 版（换 HTTP），
 * 只改这个文件。
 */

import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  ConnectionTestResult,
  FeedPage,
  LlmModelList,
  LlmProbeInput,
  LlmSettingsInput,
  LlmSettingsView,
  Paper,
  ProviderProfileInfo,
  QaRequest,
  QaResponse,
  SearchResponse,
  Summary,
} from '../types';

/** 在普通浏览器里跑（没经过 Tauri）时，invoke 会抛错 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function health(): Promise<string> {
  return invoke<string>('health');
}

export async function searchPapers(
  query: string,
  limit = 10,
): Promise<SearchResponse> {
  return invoke<SearchResponse>('search_papers', { query, limit });
}

export async function browseFeed(
  interests: string[],
  offset = 0,
  limit = 12,
): Promise<FeedPage> {
  return invoke<FeedPage>('browse_feed', { interests, offset, limit });
}

export async function summarizePaper(paper: Paper): Promise<Summary> {
  return invoke<Summary>('summarize_paper', { paper });
}

export async function summarizePaperStream(
  paper: Paper,
  onDelta: (delta: string) => void,
): Promise<Summary> {
  const channel = new Channel<string>();
  channel.onmessage = onDelta;

  return invoke<Summary>('summarize_paper_stream', {
    paper,
    onEvent: channel,
  });
}

export async function getLlmSettings(): Promise<LlmSettingsView> {
  return invoke<LlmSettingsView>('get_llm_settings');
}

export async function saveLlmSettings(
  settings: LlmSettingsInput,
): Promise<LlmSettingsView> {
  return invoke<LlmSettingsView>('save_llm_settings', { settings });
}

export async function getLlmProviderCatalog(): Promise<ProviderProfileInfo[]> {
  return invoke<ProviderProfileInfo[]>('get_llm_provider_catalog');
}

export async function fetchLlmModels(
  input: LlmProbeInput,
): Promise<LlmModelList> {
  return invoke<LlmModelList>('fetch_llm_models', { input });
}

export async function testLlmConnection(
  input: LlmProbeInput,
): Promise<ConnectionTestResult> {
  return invoke<ConnectionTestResult>('test_llm_connection', { input });
}

export async function askPaper(request: QaRequest): Promise<QaResponse> {
  return invoke<QaResponse>('ask_paper', { request });
}

export async function askPaperStream(
  request: QaRequest,
  onDelta: (delta: string) => void,
): Promise<QaResponse> {
  const channel = new Channel<string>();
  channel.onmessage = onDelta;

  return invoke<QaResponse>('ask_paper_stream', { request, onEvent: channel });
}

const pdfRequests = new Map<string, Promise<ArrayBuffer>>();

export async function downloadPdf(url: string): Promise<ArrayBuffer> {
  const existing = pdfRequests.get(url);
  if (existing) return existing;

  const request = invoke<ArrayBuffer>('download_pdf', { url }).finally(() => {
    pdfRequests.delete(url);
  });
  pdfRequests.set(url, request);
  return request;
}
