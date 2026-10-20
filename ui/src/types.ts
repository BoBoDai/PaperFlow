/**
 * 与 Rust 侧领域模型对应的类型。
 *
 * ⚠️ 目前是手写的镜像。Rust 那边改字段，这里不会自动跟进。
 * 下一步应该用 ts-rs 从 `paperflow-core` 直接生成，消掉这份重复。
 * 在那之前：Rust 是唯一真相来源，这里只做镜像。
 */

export type Source = 'arxiv' | 'semantic_scholar' | 'openalex';

/** 对应 `paperflow_core::Paper` */
export interface Paper {
  id: string;
  title: string;
  authors: string[];
  abstract_text: string;
  categories: string[];
  /** RFC3339 */
  published: string;
  pdf_url: string;
  source: Source;
  venue: string | null;
}

/** 对应 `paperflow_core::Summary` */
export interface Summary {
  short: string;
  detailed: string;
  key_points: string[];
  model: string;
  prompt_version: string;
}

/** 对应 `paperflow_core::FeedPage` */
export interface FeedPage {
  papers: Paper[];
  has_more: boolean;
}

/** 对应 `src-tauri::settings::LlmProviderConfig` */
export interface LlmProviderConfig {
  id: string;
  name: string;
  api_key: string;
  base_url: string;
  model: string;
}

/** 对应 `src-tauri::settings::LlmSettings` */
export interface LlmSettings {
  providers: LlmProviderConfig[];
  active_id: string;
}
