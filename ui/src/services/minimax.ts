import { ArxivPaper } from './arxiv';

const API_BASE = 'http://localhost:8080';

export interface PaperSummary {
  short_summary: string;
  detailed_summary: string;
  key_points: string[];
}

export interface TranslateResult {
  text: string;
  translated: boolean;
}

/** Translate Chinese query to English via backend LLM */
export async function translateQuery(query: string, apiKey: string): Promise<TranslateResult> {
  // If no Chinese characters, return as-is
  if (!/[一-龥]/.test(query)) {
    return { text: query, translated: false };
  }

  try {
    const response = await fetch(`${API_BASE}/api/translate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text: query }),
    });

    if (!response.ok) {
      return { text: query, translated: false };
    }

    const data = await response.json();
    return {
      text: data.translated || query,
      translated: data.success === true,
    };
  } catch {
    return { text: query, translated: false };
  }
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

export interface PaperTranslation {
  title_cn: string;
  abstract_cn: string;
  success: boolean;
}

/** Translate paper title and abstract to Chinese */
export async function translatePaper(title: string, abstractText: string): Promise<PaperTranslation> {
  try {
    const response = await fetch(`${API_BASE}/api/translate-paper`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ title, abstract_text: abstractText }),
    });

    if (!response.ok) {
      return { title_cn: '', abstract_cn: '', success: false };
    }

    return response.json();
  } catch {
    return { title_cn: '', abstract_cn: '', success: false };
  }
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
