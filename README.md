# PaperFlow

学术论文检索与分析桌面应用。

- 自然语言搜索 arXiv / Semantic Scholar
- 中文查询自动转英文关键词，标题摘要自动中译
- LLM 分析论文的核心贡献与关键要点
- 后台定时监控新论文

## 架构

**Rust 管核心逻辑，TypeScript 管界面，Tauri 负责打包成单文件应用。**

```
┌─────────────────────────────────────────────┐
│  ui/           React + Vite                 │  渲染层
└──────────────────┬──────────────────────────┘
                   │ Tauri IPC（无端口、无 CORS、类型安全）
┌──────────────────▼──────────────────────────┐
│  src-tauri/    Tauri 壳 + IPC command       │  壳：只做转接
└──────────────────┬──────────────────────────┘
┌──────────────────▼──────────────────────────┐
│  crates/usecases/   编排，传输无关            │  ← 加新传输方式只改这里
└──────────────────┬──────────────────────────┘
┌──────────────────▼──────────────────────────┐
│  crates/core/       纯逻辑，零 IO            │  可纯单测
│  crates/adapters/   外部世界                 │  arXiv / LLM / 存储
└─────────────────────────────────────────────┘
```

### 两条必须守住的纪律

1. **`crates/usecases` 里不许出现 `tauri::` 或 `axum::`。**
   这样将来要加 Web 版、CLI、TUI，每个只是 5 行的薄壳，不是重构。

2. **LLM 输出必须过结构校验才能返回，不许有「解析失败就用原文」的兜底。**
   见 `paperflow_core::Error::InvalidLlmOutput`。静默降级是最糟的失败模式 ——
   用户看到垃圾，日志里一片安静。

## 目录

```
crates/
  core/         领域模型 + 纯逻辑（Paper、Summary、错误类型）
  adapters/     arXiv / LLM / 存储的 trait 与实现
  usecases/     编排层，传输无关
src-tauri/      Tauri 应用（main.rs 只做入口，commands.rs 是 IPC）
ui/             React + Vite 前端
docs/           技术文档
```

## 开发

前置：Rust（含 rustup）、Node 20+、pnpm、Xcode Command Line Tools。

```bash
pnpm install          # 装根目录（Tauri CLI）+ ui 的依赖
```

### 桌面应用（完整形态）

```bash
pnpm app:dev          # 启动 Tauri，窗口里带 HMR
pnpm app:build        # 打包出 .dmg / .msi / .deb / .AppImage
```

### 纯前端（浏览器里调 UI，迭代最快）

```bash
pnpm dev              # 只起 Vite，浏览器开 localhost:5173
```

注意：浏览器里没有 Tauri IPC，界面会显示「未在 Tauri 中运行」。
`ui/` 的样式和布局可以这样快速迭代，但数据要 `pnpm app:dev`。

### 检查

```bash
cargo test            # Rust 单测（core / usecases 的纯逻辑）
cargo clippy          # lint
pnpm typecheck        # 前端类型检查
```

## 当前状态

骨架已通：**`pnpm app:dev` 开窗、`pnpm app:build` 出安装包、IPC 链路贯通。**

尚未实现（下一步）：

- [x] `adapters/arxiv.rs` —— 真实 arXiv Atom API 客户端（HTTP + XML 解析）
- [ ] `adapters/llm.rs` —— 现在是 `StubLlmProvider`，返回占位文本
- [ ] `adapters/storage.rs` —— 只有 trait，没有 SQLite 实现
- [ ] 后台调度器（定时抓取 + 通知）
- [ ] `ui/src/types.ts` 是手写的 Rust 类型镜像，应该用 ts-rs 生成
- [ ] 前端样式方案（Tailwind v4 + shadcn/ui）
- [ ] 三平台各自验证一次 WebView 渲染差异
