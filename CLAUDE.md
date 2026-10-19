# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

PaperFlow 是一个学术论文检索与分析桌面应用（arXiv / Semantic Scholar 检索、中文查询转英文关键词、LLM 论文分析、后台监控）。技术栈为 **Tauri v2 + Rust workspace + React 19 / Vite 8**，Rust 负责核心逻辑，TypeScript 负责界面。

架构决策及其被否决的备选方案记录在 `docs/decisions.md`。改动分层、传输方式或技术栈前先读它，避免重新讨论已经定论的问题。

## 常用命令

```bash
pnpm install              # 一次装好根目录（Tauri CLI）和 ui/ workspace 的依赖
pnpm app:dev              # 启动 Tauri 桌面窗口（Vite HMR + Rust 热重启）
pnpm app:build            # 打包安装包（.dmg / .msi / .deb / .AppImage）
pnpm dev                  # 只起 Vite（localhost:5173），无 Tauri IPC，仅用于迭代样式/布局
pnpm typecheck            # 前端类型检查（tsc --noEmit）
pnpm build                # tsc --noEmit && vite build

cargo test                # 全部 Rust 单测（workspace）
cargo test -p paperflow-usecases        # 只测某一个 crate
cargo test -p paperflow-usecases dedupe # 按测试名过滤单个测试
cargo clippy              # Rust lint
```

注意：Vite 配置了 `strictPort: true` 且固定 5173 端口（Tauri `devUrl` 依赖它），端口占用时会直接报错而非自动换端口。在纯浏览器里运行时前端会显示「未在 Tauri 中运行」，这是预期行为。

## 分层架构

依赖方向自上而下，**下层不得反向依赖上层**：

```
ui/ (React + Vite)                渲染层，只通过 IPC 与后端交互
      │ Tauri IPC
src-tauri/ (Tauri 壳)             装配适配器、注册 command、开窗口
      │
crates/usecases/                  编排层，传输无关
      │
crates/core/  ←  crates/adapters/
纯逻辑，零 IO       外部世界（arXiv / LLM / 存储）
```

- `crates/core`：领域模型 `Paper`、`Summary`、`Source` 与 `Error`/`Result`。领域内共享的纯逻辑放这里（如 `Paper::dedup_key`）。
- `crates/adapters`：每个外部依赖一个 trait——`ArxivClient`、`LlmProvider`、`PaperStore`——外加占位实现。适配器只做外部格式 ↔ 领域类型的翻译。
- `crates/usecases`：`App<A, L>` 通过泛型注入适配器（不用 `dyn`，规避 `dyn` + `async fn` 的对象安全问题），提供 `search_papers` / `summarize` 等用例。`dedupe` 是纯函数，可直接单测。
- `src-tauri/src/lib.rs`：唯一的适配器装配点（目前是 `App::new(ArxivHttpClient, StubLlmProvider)`），接真实实现时只改这里。
- `src-tauri/src/commands.rs`：IPC command，全部是薄壳。
- `ui/src/api/ipc.ts`：前端调用 Rust 的唯一通道，组件不直接调用 `invoke`。
- `ui/src/types.ts`：手写的 Rust 领域类型镜像，**Rust 是唯一真相来源**；改 Rust 侧字段时必须同步修改这里（计划改用 ts-rs 生成）。

## 必须守住的纪律

1. **`crates/usecases` 中不许出现 `tauri::` 或 `axum::`。** 新增传输方式（Web/CLI/TUI）应该只是几个薄壳，而不是重构。同理，`crates/core` 中不许出现任何 HTTP 客户端、数据库或 GUI/IPC 类型。
2. **LLM 输出必须通过结构校验才能返回，禁止任何静默降级。** 解析失败就返回 `paperflow_core::Error::InvalidLlmOutput`，由调用方决定重试或报错；不许「解析失败就把原文/空字符串当结果」。合法 JSON 但字段形状不对同样算失败。
3. **IPC command 里不写业务逻辑。** 取参数 → 调用 usecase → 返回；错误统一转成 `String` 跨 IPC，不降级、不返回空结果假装成功。
4. **摘要缓存键是 `(paper_id, model, prompt_version)`。** 修改 prompt 时必须 bump `Summary::PROMPT_VERSION`，旧缓存会自动失效。
5. **跨源去重按归一化标题**（`Paper::dedup_key`），因为 arXiv 与 Semantic Scholar 对同一篇论文使用不同 id。

## 当前状态（骨架阶段）

以下都是刻意保留的占位实现，替换时保持上层不动：

- `ArxivHttpClient`：真实 arXiv Atom API 客户端；解析逻辑有单测，另有标记为 `#[ignore]` 的在线测试（`cargo test -p paperflow-adapters -- --ignored`）。
- `StubLlmProvider`：返回占位文本（`crates/adapters/src/llm.rs`）。
- `PaperStore`：只有 trait，没有 SQLite 实现（`crates/adapters/src/storage.rs`，表结构草案见该文件文档注释）。
- `ui/src/styles.css`：原生 CSS，样式方案尚未迁移到 Tailwind v4 + shadcn/ui。

## 新增一条 IPC 链路的完整改法

1. 在 `crates/usecases` 实现用例并写纯逻辑单测。
2. 在 `src-tauri/src/commands.rs` 添加薄壳 command，并在 `lib.rs` 的 `generate_handler!` 中注册。
3. 在 `ui/src/api/ipc.ts` 添加带类型的包装函数。
4. 若新增领域类型，同步更新 `ui/src/types.ts`。

## 提交规则和代码规范
1. 所有注释都用英语实现
2. 都在主分支上进行提交
3. 提交commit，为一行英文