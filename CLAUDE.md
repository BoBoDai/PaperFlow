# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

PaperFlow 是一个学术论文检索与分析桌面应用：arXiv / OpenAlex 检索、兴趣信息流、中文查询转英文关键词、LLM 摘要、内置 PDF 阅读器与 AI 问答。技术栈为 **Tauri v2 + Rust workspace + React 19 / Vite 8 + Tailwind CSS v4**，Rust 负责领域逻辑与外部 IO，TypeScript 只负责渲染。

- 先读本文件了解边界和入口；需要完整调用链时读 `docs/architecture.md`。
- 改动分层、传输方式或技术栈前读 `docs/decisions.md`，不要重新讨论已定论的问题。
- Rust 是领域类型的唯一真相来源；`ui/src/types.ts` 是手写镜像，改字段必须同步。

## 常用命令

```bash
pnpm install              # 安装根目录（Tauri CLI）和 ui/ 依赖
pnpm app:dev              # 启动 Tauri 桌面窗口（Vite HMR + Rust 热重启）
pnpm app:build            # 打包 .dmg / .msi / .deb / .AppImage
pnpm dev                  # 只起 Vite（localhost:5173），无 Tauri IPC，仅调样式
pnpm typecheck            # tsc --noEmit
pnpm build                # typecheck + vite build

cargo test --workspace                  # 全部 Rust 单测
cargo test -p paperflow-usecases        # 单 crate
cargo test -p paperflow-usecases dedupe # 按测试名过滤
cargo test -p paperflow-adapters -- --ignored  # 在线集成测试（arXiv/OpenAlex/PDF）
cargo clippy --workspace --all-targets -- -D warnings
```

Vite 固定 5173 且 `strictPort: true`，端口占用会直接报错。纯浏览器里没有 Tauri IPC，显示「未在 Tauri 中运行」是预期行为。

## 架构（简版）

依赖方向自上而下，**下层不得反向依赖上层**：

```
ui/ (React + Vite)                渲染层，只通过 IPC 交互
      │ Tauri IPC
src-tauri/ (Tauri 壳)             装配适配器、注册 command、开窗口
      │
crates/usecases/                  编排层，传输无关
      │
crates/core/  ←  crates/adapters/
纯逻辑，零 IO       外部世界（arXiv / OpenAlex / LLM / PDF）
```

- `crates/core`：`Paper`、`FeedPage`、`Summary`、`Source`、`QaRequest` / `QaResponse` / `Answer`、`Error` / `Result`；领域内共享的纯逻辑（如 `Paper::dedup_key`）。
- `crates/adapters`：外部依赖的 trait 与实现。`PaperSearch`（arXiv / OpenAlex）、`LlmProvider`（OpenAI 兼容客户端）、`PdfFetcher`（下载 / 校验 / 缓存）、`PaperStore`（仅 trait）。
- `crates/usecases`：`App<A, L, O>` 泛型注入适配器，实现 `search_papers` / `browse_feed` / `summarize` / `ask_paper`；不放 `tauri::` / `axum::`。
- `src-tauri`：唯一装配点。`AppState` 持有 `App`、`SettingsManager`、`PdfFetcher`；command 全部是薄壳。
- `ui`：`App.tsx` 管页面与列表状态；`ReaderView` / `PdfPane` / `AiPanel` 管阅读器；`api/ipc.ts` 是前端调用 Rust 的唯一通道。

## 任务路由

先读“入口”一列，再按需展开同目录文件；测试位置见最后一列。

| 要改的东西 | 入口 | 相关 / 测试 |
|---|---|---|
| 检索、中文翻译 | `crates/usecases/src/search.rs` | `adapters/src/arxiv.rs`、`openalex.rs`；`usecases/src/tests/search.rs` |
| 兴趣信息流、分页 | `crates/usecases/src/feed.rs` | `usecases/src/tests/feed.rs` |
| 问答预算、历史裁剪 | `crates/usecases/src/qa.rs` | `usecases/src/tests/qa.rs` |
| LLM 服务商 / 兼容性 | `crates/adapters/src/llm/profiles.rs` | `llm/config.rs`、`llm/client.rs`；`llm/tests.rs` |
| LLM prompt / 输出校验 | `crates/adapters/src/llm/prompts.rs` | `llm/parsing.rs`、`llm/streaming.rs`；`llm/tests.rs` |
| 设置持久化 / 迁移 | `src-tauri/src/settings/` | `settings/tests.rs`；`credentials.rs` |
| PDF 下载 / 缓存 | `crates/adapters/src/pdf.rs` | `pdf/tests.rs`；`commands::download_pdf` |
| 阅读器 / AI 面板 | `ui/src/components/ReaderView.tsx` | `PdfPane.tsx`、`AiPanel.tsx`、`ui/src/lib/pdfjs.ts` |
| 新增 IPC 链路 | `.claude/skills/add-ipc-command/SKILL.md` | `commands.rs`、`src-tauri/src/lib.rs`、`ui/src/api/ipc.ts` |
| 样式 | `ui/src/styles.css` + 组件 utility classes | 保持现有深浅色 token，PDF `textLayer` 规则除外 |
| 发布前检查 | `.claude/skills/release-check/SKILL.md` | test / clippy / typecheck / build |

## 必须守住的纪律

1. **分层边界。** `core` 不出现 HTTP / 数据库 / GUI / IPC 类型；`usecases` 不出现 `tauri::` 或 `axum::`；IPC command 不写业务逻辑，错误统一转 `String`，不返回空结果假装成功。
2. **LLM 输出必须结构校验，禁止静默降级。** 失败返回 `Error::InvalidLlmOutput`；摘要按固定纯文本段（`一句话：` / `详细分析：` / `要点：`）解析，模型省略标题时用首行 / 全文兜底并 warning，空输出才报错；关键词翻译严格校验 JSON；问答为非空纯文本。任何 prompt 改动都要 bump 对应 `PROMPT_VERSION`（摘要缓存键是 `(paper_id, model, prompt_version)`）。
3. **检索源按 arXiv → OpenAlex 回退。** arXiv 报错才回退；arXiv 为空也查 OpenAlex。两个源都报错必须返回 `Error::Sources`。结果用 `Paper::dedup_key` 按归一化标题跨源去重，`Paper.source` 标明实际来源。
4. **中文查询优先经 LLM 转英文关键词。** 已配置 LLM 时翻译失败照常报错；未配置时记录 warning 并直接按中文原文检索。`SearchResponse.query` 是实际发送给数据源的查询，前端据此提示。
5. **LLM 配置运行时可变，key 只写不读。** 多服务商存在 `settings.json`（`providers` + `active_id`），key 单独存 `credentials.json`（0600），绝不写日志或经 IPC 返回。客户端每次请求读共享 `RwLock<LlmConfig>`。MiniMax 默认不发 `response_format` / `thinking`（M3.1 不能禁用 thinking），解析前剥离 `<think>`；高级兼容性可覆盖。
6. **信息流分页语义。** offset 按 limit 对齐；`has_more` 按去重前原始页大小计算；offset > 0 的空页视为到底，不再切源；首页 arXiv 为空且 OpenAlex 失败返回 `Error::Sources`。兴趣是前端偏好，存 `localStorage` 键 `paperflow.interests.v1`，最多 8 个；搜索、信息流、分析使用独立 loading 状态。
7. **PDF 只经 Rust 下载。** 仅 HTTPS 公网地址，拒绝私网 / 本地主机，限制 32 MiB，校验 `%PDF-`，缓存到 app cache。前端通过 `download_pdf` 拿 `ArrayBuffer`，PDF.js 不做跨域 fetch。
8. **问答有硬预算且走流式。** 选区 8k、当前页 40k、全文 30k 头 + 10k 尾；历史最多 6 轮 / 12k 字符；prompt 不含 `pdf_url`。SSE 增量经 Tauri `Channel<String>` 推送，完整响应作为 command 返回值。
9. **前端样式统一用 Tailwind v4 utility classes。** `styles.css` 只保留 `@theme` token、base 样式和 PDF.js `textLayer` 必需规则；不要新增组件级 CSS。

## 文档与技能索引

- `docs/architecture.md`：分层、启动装配、搜索 / 信息流 / 摘要 / 问答 / PDF 的完整调用链、错误与回退、设置迁移、测试地图。
- `docs/decisions.md`：已定方向与被否决方案（GPUI、dora、本地 HTTP 等），改架构前先读。
- `.claude/skills/add-ipc-command/`：新增一条 IPC 链路的固定步骤。
- `.claude/skills/llm-provider/`：新增 / 调试 LLM 服务商。
- `.claude/skills/pdf-reader/`：PDF 下载、渲染、文本层、问答上下文。
- `.claude/skills/release-check/`：提交前的自动检查清单。

## 新增一条 IPC 链路

1. 在 `crates/usecases` 实现用例并写单测（纯逻辑，不碰 Tauri）。
2. 在 `src-tauri/src/commands.rs` 加薄壳，并在 `src-tauri/src/lib.rs` 的 `generate_handler!` 注册。
3. 在 `ui/src/types.ts` 同步类型（如新增领域类型），再在 `ui/src/api/ipc.ts` 加带类型的包装函数。
4. 组件只调用 `ipc.ts`，不直接 `invoke`。
5. 跑验证命令；涉及 UI 时再 `pnpm app:dev` 手动走一遍。

## 当前状态

- 已实现：真实 arXiv / OpenAlex 客户端、多服务商 LLM、兴趣流、PDF 阅读器与流式问答。
- 未实现：`PaperStore` 的 SQLite 实现、后台定时调度 / 通知、ts-rs 类型生成、三平台 WebView 逐一验证。
- 刻意不做：全文 RAG / embedding（当前是字符预算截断）、扫描版 PDF OCR、聊天持久化。

## 提交规则和代码规范

1. 所有代码注释用英语。
2. 都在 `main` 分支提交。
3. commit message 为一行英文。
