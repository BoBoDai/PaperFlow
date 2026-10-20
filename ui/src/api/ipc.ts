/**
 * IPC 封装 —— 前端与 Rust 之间的唯一通道。
 *
 * 组件不直接调 `invoke`，都走这里。将来若要加 Web 版（换 HTTP），
 * 只改这个文件。
 */

import { invoke } from '@tauri-apps/api/core';
import type { LlmSettings, Paper, Summary } from '../types';

/** 在普通浏览器里跑（没经过 Tauri）时，invoke 会抛错 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function health(): Promise<string> {
  return invoke<string>('health');
}

export async function searchPapers(query: string, limit = 10): Promise<Paper[]> {
  return invoke<Paper[]>('search_papers', { query, limit });
}

export async function summarizePaper(paper: Paper): Promise<Summary> {
  return invoke<Summary>('summarize_paper', { paper });
}

export async function getLlmSettings(): Promise<LlmSettings> {
  return invoke<LlmSettings>('get_llm_settings');
}

export async function saveLlmSettings(settings: LlmSettings): Promise<LlmSettings> {
  return invoke<LlmSettings>('save_llm_settings', { settings });
}
