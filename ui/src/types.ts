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

/** 对应 `paperflow_core::SearchResponse` */
export interface SearchResponse {
  papers: Paper[];
  query: string;
}

/** 对应 `paperflow_core::FeedPage` */
export interface FeedPage {
  papers: Paper[];
  has_more: boolean;
}

export type ProviderProfileId = 'deepseek' | 'openai' | 'minimax' | 'custom';
export type MaxTokensField = 'max_tokens' | 'max_completion_tokens';
export type ThinkingFormat = 'thinking' | 'enable_thinking';
export type ThinkingMode = 'default' | 'disabled' | 'adaptive';

export interface LlmCompat {
  json_mode?: boolean | null;
  max_tokens_field?: MaxTokensField | null;
  thinking_mode?: ThinkingMode | null;
  thinking_format?: ThinkingFormat | null;
}

export interface EffectiveCapabilities {
  json_mode: boolean;
  max_tokens_field: MaxTokensField;
  thinking_mode: ThinkingMode;
  thinking_format: ThinkingFormat;
}

/** 对应 `paperflow_adapters::ProviderProfileInfo` */
export interface ProviderProfileInfo {
  id: ProviderProfileId;
  label: string;
  default_base_url: string;
  default_model: string;
  capabilities: EffectiveCapabilities;
  protocol: string;
}

/** 对应 `src-tauri::settings::LlmProviderView`（不含 key） */
export interface LlmProviderView {
  id: string;
  name: string;
  base_url: string;
  model: string;
  provider: ProviderProfileId;
  compat: LlmCompat;
  has_api_key: boolean;
}

/** 对应 `src-tauri::settings::LlmSettingsView` */
export interface LlmSettingsView {
  schema_version: number;
  providers: LlmProviderView[];
  active_id: string;
}

/** 对应 `src-tauri::settings::LlmProviderInput` */
export interface LlmProviderInput {
  id: string;
  name: string;
  base_url: string;
  model: string;
  provider: ProviderProfileId;
  compat: LlmCompat;
  api_key?: string | null;
  clear_api_key?: boolean;
}

export interface LlmSettingsInput {
  providers: LlmProviderInput[];
  active_id: string;
}

export interface LlmProbeInput {
  provider_id: string | null;
  base_url: string;
  model: string;
  provider: ProviderProfileId;
  compat: LlmCompat;
  api_key?: string | null;
}

export type LlmErrorKind =
  | 'missing_config'
  | 'bad_request'
  | 'unauthorized'
  | 'forbidden'
  | 'not_found'
  | 'rate_limited'
  | 'server'
  | 'timeout'
  | 'network'
  | 'unknown_model'
  | 'invalid_response';

export interface LlmEndpointError {
  kind: LlmErrorKind;
  message: string;
  status: number | null;
  request_id: string | null;
  provider_message: string | null;
}

export interface LlmModelList {
  models: string[];
  request_id: string | null;
}

export interface ConnectionTestResult {
  ok: boolean;
  latency_ms: number;
  model: string;
  request_id: string | null;
}
