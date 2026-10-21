# PaperFlow

学术论文检索与分析桌面应用。

- 自然语言搜索 arXiv / OpenAlex
- 兴趣频道推荐流：按兴趣浏览最新论文，类似论文版信息流
- 中文查询由 LLM 自动转英文关键词
- LLM 分析论文的核心贡献与关键要点
- 内置 PDF 阅读器 + AI 阅读助手（选区解释 / 当前页 / 全文问答）
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
│  crates/adapters/   外部世界                 │  arXiv / OpenAlex / LLM / 存储
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

## 推荐与兴趣

左侧边栏提供「推荐 / 搜索 / 兴趣」三个入口。推荐页按兴趣浏览最新论文：

- 默认兴趣：`large language models`、`diffusion models`、`computer vision`、
  `reinforcement learning`
- 「全部兴趣」合并所有兴趣；也可以点单个兴趣查看独立频道
- 按最新发表时间倒序，滚动到底部自动加载下一页，也可手动「加载更多」
- arXiv 为主源，失败时自动回退 OpenAlex；两个源都失败会明确报错
- 兴趣在左侧增删，最多 8 个；建议使用英文主题（数据源以英文论文为主）
- 兴趣保存在浏览器 `localStorage`（键 `paperflow.interests.v1`），暂不写入后端配置

## 阅读论文

点击论文卡片先显示 AI 总结；详情页点「开始阅读」打开内置 PDF 阅读器：

- PDF 由 Rust 下载并缓存（HTTPS、公网地址、32MB 上限、`%PDF-` 校验），避开 WebView CORS
- 支持翻页、页码跳转、缩放
- 选中 PDF 文本 → 「解释选中内容」
- 问答支持「当前页」和「全文」两种上下文；全文超过约 4 万字符时取开头 + 结尾，并提示已截断
- 多轮追问保留最近 6 轮，回答为简体中文
- 扫描版 PDF、出版商登录页/订阅页等无法直接阅读时会给出明确错误

PDF.js 的 worker、cmaps 和 standard fonts 在构建时复制到 `ui/dist/`。

## LLM 配置

右上角「设置」是一个多服务商管理面板，可配置多个任意 OpenAI 兼容端点：

- 左侧列表管理服务商，右侧编辑名称 / Base URL / API Key / 模型
- 预设：DeepSeek（`https://api.deepseek.com` / `deepseek-chat`）、
  OpenAI（`https://api.openai.com/v1` / `gpt-4o-mini`）、
  MiniMax（国内 `https://api.minimaxi.com/v1` / `MiniMax-M2`；
  国际版可改为 `https://api.minimax.io/v1`）
- 「设为当前」决定中文查询翻译和论文分析使用哪个服务商
- 保存后立即生效，无需重启

配置保存在系统应用配置目录的 `settings.json`（macOS 为
`~/Library/Application Support/com.paperflow.desktop/`，Unix 权限 0600），
不会进入仓库，也不会写入日志；旧版单服务商配置会自动迁移。

- 英文查询直接检索 arXiv / OpenAlex
- 中文查询由当前服务商翻译成英文关键词；未配置 key 时直接按中文原文检索
- 搜索后结果上方会显示实际使用的英文关键词
- 论文分析（右侧「LLM 分析」）使用当前服务商

## 当前状态

骨架已通：**`pnpm app:dev` 开窗、`pnpm app:build` 出安装包、IPC 链路贯通。**

尚未实现（下一步）：

- [x] `adapters/arxiv.rs` —— 真实 arXiv Atom API 客户端（HTTP + XML 解析）
- [x] `adapters/openalex.rs` —— OpenAlex 回退源，arXiv 限流或无结果时自动接管
- [x] `adapters/llm.rs` —— DeepSeek 客户端（OpenAI 兼容）+ 多服务商设置页
- [x] 兴趣推荐流 —— 侧边栏 + 兴趣频道（按最新发表排序，arXiv/OpenAlex 回退）
- [x] 内置 PDF 阅读器 + AI 辅助阅读（选区解释 / 当前页 / 全文问答）
- [ ] `adapters/storage.rs` —— 只有 trait，没有 SQLite 实现
- [ ] 后台调度器（定时抓取 + 通知）
- [ ] `ui/src/types.ts` 是手写的 Rust 类型镜像，应该用 ts-rs 生成
- [ ] 前端样式方案（Tailwind v4 + shadcn/ui）
- [ ] 三平台各自验证一次 WebView 渲染差异
