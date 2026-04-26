import { ArxivPaper } from './arxiv';

const API_BASE = 'http://localhost:8080';

export interface PaperSummary {
  short_summary: string;
  detailed_summary: string;
  key_points: string[];
}

// Note: translateQuery still calls MiniMax directly since it's a simple API call
export async function translateQuery(query: string, apiKey: string): Promise<string> {
  // For now, just return the query as-is
  // The Rust backend will handle translation when searching
  return query;
}

export async function summarizePaper(paper: ArxivPaper, apiKey: string): Promise<PaperSummary> {
  const response = await fetch(`${API_BASE}/api/summarize`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      paper_id: paper.id,
      title: paper.title,
      authors: paper.authors,
      summary: paper.summary,
    }),
  });

  if (!response.ok) {
    throw new Error(`摘要生成 API 错误: ${response.status}`);
  }

  return response.json();
}

export async function verbalizeText(summary: string, apiKey: string): Promise<string> {
  // For now, return the summary as-is for TTS
  return summary;
}

export interface ApiConfig {
  max_papers: number;
  voice_speed: number;
  api_key?: string;
}

export async function getConfig(): Promise<ApiConfig> {
  const response = await fetch(`${API_BASE}/api/config`);
  if (!response.ok) {
    throw new Error(`获取配置 API 错误: ${response.status}`);
  }
  return response.json();
}

export async function updateConfig(config: Partial<ApiConfig>): Promise<ApiConfig> {
  const response = await fetch(`${API_BASE}/api/config`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(config),
  });

  if (!response.ok) {
    throw new Error(`更新配置 API 错误: ${response.status}`);
  }
  return response.json();
}
