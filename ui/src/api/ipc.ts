/**
 * IPC 封装 —— 前端与 Rust 之间的唯一通道。
 *
 * 组件不直接调 `invoke`，都走这里。将来若要加 Web 版（换 HTTP），
 * 只改这个文件。
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  ConnectionTestResult,
  FeedPage,
  LlmModelList,
  LlmProbeInput,
  LlmSettingsInput,
  LlmSettingsView,
  Paper,
  ProviderProfileInfo,
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
