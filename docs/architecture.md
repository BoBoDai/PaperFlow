# PaperFlow 架构

本文回答“系统现在怎么跑”。为什么这样选、否决了哪些方案，见 `docs/decisions.md`；
日常命令、纪律和任务路由见 `CLAUDE.md`。

## 1. 分层与依赖

```
ui/ (React + Vite)                         渲染层，只通过 IPC 交互
      │ Tauri IPC（无本地 HTTP、无 CORS）
src-tauri/ (Tauri 壳)                      装配适配器、注册 command、开窗口
      │
crates/usecases/                           编排层，传输无关
      │
crates/core/  ←  crates/adapters/
纯逻辑，零 IO       外部世界（arXiv / OpenAlex / LLM / PDF）
```

依赖方向只能向下。`core` 不包含任何 HTTP 客户端、数据库或 GUI/IPC 类型；
`usecases` 不包含 `tauri::` / `axum::`；`src-tauri` 只做参数转接和错误转 `String`。

## 2. 启动与装配

`src-tauri/src/lib.rs::run()`：

1. 初始化 `tracing_subscriber`。
2. `SettingsManager::load(config_dir)` 读取 `settings.json` 和 `credentials.json`，完成旧格式迁移，得到共享的 `Arc<RwLock<LlmConfig>>`。
3. `PdfFetcher::new(app_cache_dir/pdfs)` 创建下载器。
4. `OpenAiCompatibleClient::new(settings.shared())` 注入同一个 `RwLock`。
5. `App::new(arxiv, llm, openalex)` 注入三个适配器，连同 `settings`、`pdf` 放进 `AppState`。
6. `generate_handler!` 注册全部 command。

`AppState` 是唯一的跨 IPC 上下文：

```rust
pub struct AppState {
    pub app: App<ArxivHttpClient, OpenAiCompatibleClient, OpenAlexHttpClient>,
    pub settings: SettingsManager,
    pub pdf: PdfFetcher,
}
```

## 3. 搜索链路

`ui` → `ipc.searchPapers()` → `commands::search_papers` → `App::search_papers`（`crates/usecases/src/search.rs`）。

1. 查询 trim；空查询直接返回空 `SearchResponse`。
2. `effective_query`：
   - 不含 CJK → 原样使用。
   - 含 CJK 且 LLM 已配置 → `llm.to_english_keywords`；失败直接报错。
   - 含 CJK 且 LLM 未配置 → 记录 warning，按中文原文检索。
3. `arxiv.search`：
   - 成功且非空 → 去重后返回。
   - 成功但为空 → 调 `openalex.search`；OpenAlex 失败时记录 warning，返回空列表（arXiv 没有报错，不算双源失败）。
   - arXiv 报错 → 调 `openalex.search`；OpenAlex 也报错 → `Error::Sources`。
4. 结果按 `Paper::dedup_key` 去重；`SearchResponse.query` 返回实际发送给数据源的查询。

在线测试通过 `#[ignore]` 隔离，不进入默认 `cargo test`。

## 4. 兴趣信息流

`ui` → `commands::browse_feed` → `App::browse_feed`（`crates/usecases/src/feed.rs`）。

1. `normalize_interests`：trim、去空、忽略大小写去重、最多 `MAX_FEED_INTERESTS = 8` 个。
2. 每个兴趣走 `effective_query`（中文会翻译；失败报错）。
3. `limit` clamp 到 `1..=FEED_MAX_LIMIT`；`offset = (offset / limit) * limit`，按页对齐。
4. arXiv `feed`：
   - 成功且非空 → `feed_page`。
   - 成功但为空且 `offset > 0` → 视为到底，返回空页，不再切源。
   - 成功但为空且 `offset == 0` → 调 OpenAlex；OpenAlex 失败 → `Error::Sources`。
   - arXiv 报错 → 调 OpenAlex；OpenAlex 也报错 → `Error::Sources`。
5. `feed_page`：`has_more = raw.len() >= limit`，在去重**之前**计算；随后 `dedupe`。

兴趣本身是前端偏好，存在 `localStorage` 的 `paperflow.interests.v1`，不写入后端配置。

## 5. 摘要与分析

- `App::summarize` 直接委托 `LlmProvider::summarize`。
- `App::summarize_stream` 委托 `summarize_stream`，增量通过回调推送；command 层把回调转成 `tauri::ipc::Channel<String>`。
- 摘要 prompt 要求简体中文、固定纯文本段：
  - `一句话：`
  - `详细分析：`
  - `要点：`（每条以 `- ` 开头）
- `parse_summary_text` 按固定段解析；模型省略标题时用首行 / 全文兜底并记录 warning；最终 `short` 为空才返回 `Error::InvalidLlmOutput`；`key_points` 允许为空。
- `Summary::PROMPT_VERSION` 是摘要缓存键的一部分；改 prompt 必须 bump。

## 6. PDF 阅读与问答

### 下载

`ui` → `ipc.downloadPdf()` → `commands::download_pdf` → `PdfFetcher::fetch`：

- 仅 HTTPS；拒绝 userinfo、单标签主机、localhost / `.local` / `.internal`。
- DNS 解析后校验公网 IP；重定向每跳重新校验。
- 流式读取，上限 32 MiB；前 1 KiB 必须含 `%PDF-`。
- 缓存到 `{app_cache_dir}/pdfs/{fnv1a64(url)}.pdf`，临时文件 + rename。
- command 返回 `tauri::ipc::Response::new(bytes)`，前端收到 `ArrayBuffer`。
- 前端 `downloadPdf` 做 in-flight 去重，避免重复请求。

### 渲染

- `ui/src/lib/pdfjs.ts` 使用 legacy build + `?url` worker；`cmaps`、`standard_fonts` 由 Vite 静态复制。
- `ReaderView` 持有 `PDFDocumentProxy`，`PdfPane` 负责 canvas 与文本层，选区通过 `window.getSelection()` 获取。
- PDF.js 的 `textLayer` 结构规则保留在 `styles.css`，其余样式用 Tailwind。

### 问答

`App::ask_paper(_stream)`（`crates/usecases/src/qa.rs`）先 `prepare_qa`：

- 问题 trim；空或超过 `QA_QUESTION_BUDGET = 2000` 字符 → `InvalidInput`。
- 上下文 trim；空 → 提示可能是扫描版或文本尚未提取。
- 预算：选区 `8000`；当前页 / 全文 `40000`。
- 全文超预算时取前 3/4 + 省略标记 + 后 1/4；返回 `context_truncated` 与 `context_chars`。
- 历史：单条 user ≤ 2000、assistant ≤ 4000；最多 6 轮；总计 ≤ 12000 字符。
- prompt 不包含 `pdf_url`。
- 流式：`ask_paper_stream` → `LlmProvider::ask_paper_stream` → SSE → `Channel<String>`；最终 `QaResponse` 作为 command 返回值。
- 答案为非空纯文本；空回答 → `Error::InvalidLlmOutput`。

## 7. LLM 适配器

`crates/adapters/src/llm/`：

| 文件 | 职责 |
|---|---|
| `profiles.rs` | `ProviderProfileId`（DeepSeek / OpenAI / MiniMax / Custom）、默认 endpoint/model、能力与 `LlmCompat` 覆盖 |
| `config.rs` | `LlmConfig`（Debug 隐藏 key）、base URL 校验 |
| `client.rs` | `OpenAiCompatibleClient`：chat、SSE stream、`GET /models`、连接测试 |
| `streaming.rs` | SSE 事件切分、`<think>` 过滤、跨 chunk 标签保持 |
| `prompts.rs` | 摘要 / 关键词 / 问答 prompt 构造 |
| `parsing.rs` | 严格解析摘要、关键词、问答、模型列表 |
| `error.rs` | HTTP 状态 / 错误体 / request id 到 `LlmErrorKind` 的映射 |

请求能力由 `EffectiveCapabilities` 决定：

- `json_mode`：是否发送 `response_format`。
- `max_tokens_field`：`max_tokens` 或 `max_completion_tokens`。
- `thinking_mode` / `thinking_format`：`thinking: {type: disabled|adaptive}` 或 `enable_thinking`。

MiniMax 默认 `json_mode = false`、`thinking_mode = Default`（不发 thinking 字段），并在解析前剥离 `<think>`。高级设置可以逐项覆盖。

SSE 处理：

- 按 `\n\n` / `\r\n\r\n` 切事件；只处理 `data:` 行；`[DONE]` 结束。
- 增量先交给 `AnswerStream`，过滤思考块后再回调；跨 chunk 的 `<thi...` 会暂存。
- 若整个响应没有 SSE 事件（部分网关忽略 `stream=true`），回退解析普通 completion。
- 空最终答案 → `InvalidLlmOutput`。

模型发现 / 连接测试：

- `fetch_models` 请求 `{base_url}/models`，按模型 id 排序返回。
- `test_connection` 发送最小 chat 请求，返回延迟、模型名、request id。
- 未填写新 key 时，`resolve_probe` 使用该服务商已保存的 key。

## 8. 设置与凭据

`src-tauri/src/settings/`：

- `types.rs`：持久化类型（`LlmProviderConfig`、`LlmSettings`）与 IPC 视图 / 输入。
- `manager.rs`：`SettingsManager::load/save/current_view/resolve_probe`，保存后更新共享 `RwLock<LlmConfig>`。
- `normalize.rs`：trim、ID 去重、base URL 校验、空字段回填默认值；`Load` 宽松、`Save` 严格。
- `migration.rs`：旧版单服务商 `{api_key, base_url, model}` 和未版本化多服务商格式迁移到 schema v2。
- `src-tauri/src/credentials.rs`：key 存 `credentials.json`，Unix 0600；只提供 set/get/has/remove/retain，不经 IPC 返回。

保存语义：

- 非空 `api_key` 覆盖旧 key；空/缺省保持；`clear_api_key = true` 删除。
- 删除服务商时 `credentials.retain(provider_ids)` 清掉孤立 key。
- 配置损坏 / 迁移失败时保留原文件并 warning，不静默丢 key。

## 9. 错误模型与回退

`paperflow_core::Error`：

- `Network` / `Parse`：数据源
- `Llm` / `InvalidLlmOutput`：模型调用与输出校验
- `Storage` / `Config` / `Sources` / `InvalidInput`

原则：

- 适配器把外部错误翻译成 `Error`，并尽量保留 source 链。
- 多源失败返回 `Error::Sources("arXiv: ...; OpenAlex: ...")`，让用户看到两个原因。
- 不允许“失败返回空列表”伪装成功；唯一例外是搜索里 arXiv 为空 + OpenAlex 失败的语义（见第 3 节）。
- IPC 层只 `map_err(to_ipc)`，不重试、不降级。

## 10. 前端结构

- `App.tsx`：侧边栏、搜索 / 信息流列表、详情、摘要、阅读器 overlay；各区域独立 loading / error。
- `SettingsPanel.tsx`：多服务商列表、编辑、模型拉取、连接测试、高级兼容性。
- `ReaderView.tsx`：下载 PDF、持有 document、页码 / 缩放 / 选区、AI 面板。
- `PdfPane.tsx`：canvas + text layer；`AiPanel.tsx`：分析 / 问答 tab、选区解释、流式气泡。
- `ReviewView.tsx`：三脑评审全屏页（MAGI 终端风）、三列评审、群聊、最终表决。
- `api/ipc.ts`：唯一调用 `invoke` 的地方；组件不直接 import `@tauri-apps/api`。
- `types.ts`：手写 Rust 类型镜像；Rust 字段改动必须同步。
- `styles.css`：Tailwind v4 入口，只有 `@theme` token、base 和 PDF.js `textLayer`。

## 11. 测试地图

| 范围 | 位置 | 类型 |
|---|---|---|
| 领域纯逻辑 | `crates/core/src/*.rs` | 单元测试 |
| arXiv / OpenAlex / PDF / LLM | `crates/adapters/src/*/tests.rs` | 单元测试；部分 `#[ignore]` 在线测试 |
| 搜索 / 信息流 / 问答 / 去重 | `crates/usecases/src/tests/` | `FakeSearch` + `FakeLlm` 单测；3 个在线测试 |
| 三脑评审 / 群聊 / 表决 | `crates/usecases/src/tests/{review,magi}.rs` | 聚合规则、失败槽位、预算、隔离性 |
| 设置 / 凭据 | `src-tauri/src/settings/tests.rs`、`credentials.rs` | 临时目录单测 |
| 前端 | 无单测 | `pnpm typecheck` + `pnpm build` + 手动冒烟 |

## 12. 三脑评审与群聊（实验性）

入口在论文详情面板「开始阅读」旁，全屏 `ReviewView` overlay；没有导航项。

### 初始评审

- `App::review_paper`（`crates/usecases/src/review.rs`）用 `tokio::join!` 并行调用三个人格：
  - `Rigor`（严谨）：方法、统计、claim 与实验是否匹配。
  - `Repro`（复现）：数据 / 代码、超参、算力、评估协议。
  - `Insight`（洞见）：新颖性、与已有工作的差异、实际价值。
- 每条走 `ReviewProvider::review_persona`，严格 JSON 解析（`parse_persona_review`）；缺字段、越界 confidence、非 JSON 一律 `InvalidLlmOutput`。
- 失败槽位保留为 `PersonaOutcome::Failed { persona, message }`，不丢弃；有失败时 verdict 为 `Incomplete`。
- 聚合规则（纯函数 `aggregate_review`）：任一失败 → `Incomplete`；3:0 非 borderline → `Consensus`；2:1 非 borderline → `Majority`；其余（含全 borderline）→ `Split`。
- 仅摘要时 confidence 上限 `MAGI_ABSTRACT_CONFIDENCE_CAP = 70`，并在 UI 标注 `CONFIDENCE ≤ 70`。

### 群聊

- 前端维护一份共享 `transcript`，按 `Rigor → Repro → Insight` 顺序串行调用 `persona_chat_stream`；后发言者能看到前面的发言。
- 每次请求带 `own_review`（自己的初评）和 `panel_reviews`（另外两人的初评），prompt 层过滤掉自己，避免重复注入。
- transcript 预算 `MAGI_TRANSCRIPT_BUDGET = 12000` 字符，按整轮从旧到新裁剪；问题预算 2000 字符，上下文复用 `QA_CONTEXT_BUDGET = 40000`（头 3/4 + 尾 1/4）。
- 流式仍走 `Channel<String>`；三个脑各用一次独立 command，前端按列渲染，不做多路复用。

### 最终表决

- 「形成结论」调用一次 `finalize_opinions`；后端 `tokio::join!` 三个 `persona_opinion`，每个只收到共享 transcript + 自己的初评 + 另外两人的**初评**，不收彼此的表态。
- 返回 `OpinionBatch { outcomes, verdict }`；失败槽位同样带 `persona`，聚合规则与初评共用。
- 这是对群聊趋同的保险：讨论可以互相影响，最终判断必须独立。

### 材料与视觉

- 默认标题 + 摘要；「加载全文重评」复用 `download_pdf` + PDF.js `extractFullText`，再带 `context` 重跑评审。
- 页面为深色等宽终端：三列 1px 网格、UNIT 01/02/03、状态灯、票型条、`magi-scanlines` 扫描线；不使用 EVA logo 或 MAGI 原名作为产品文案。
- `ReviewResult::PROMPT_VERSION` / `PersonaAnswer::PROMPT_VERSION` 当前为 `v1`；改 prompt 必须 bump。

## 13. 当前边界

- `PaperStore` 只有 trait，没有 SQLite 实现；摘要缓存键已定义。
- 后台调度 / 系统通知未实现。
- ts-rs 未接入，类型镜像仍是手写。
- 全文问答是字符预算截断，不是 RAG；扫描版 PDF 无 OCR。
- 三平台 WebView 尚未逐一验证。
