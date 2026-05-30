# 项目概述

PaperFlow 是一个终端里的学术论文检索和分析工具。用户用自然语言（中文或英文）搜索最新论文，系统自动翻译、检索、分析并以中英双语展示结果。

## 核心能力

1. **多源检索** — 同时查询 arXiv（预印本）和 Semantic Scholar（期刊/会议论文）
2. **智能翻译** — 中文查询自动翻译为英文关键词；论文标题和摘要自动中译
3. **LLM 分析** — 调用大模型生成论文的核心贡献、关键发现、方法概述
4. **定时监控** — 后台定期拉取最新论文并保存到本地数据库
5. **终端 TUI** — 基于 Ink/React 的命令行界面，全键盘操作

## 技术栈

| 层 | 技术 |
|---|------|
| 后端 | Rust + Axum + Tokio + SQLx (SQLite) + Reqwest |
| 前端 | TypeScript + Ink (React for terminal) |
| AI | OpenAI 兼容 API（MiniMax / DeepSeek / OpenAI）|
| 数据源 | arXiv API + Semantic Scholar API |

## 设计原则

- **终端优先** — 不依赖浏览器，SSH 进去就能用
- **键盘驱动** — 所有操作通过单键完成，不用鼠标
- **极简 UI** — Claude Code 风格，无框线，内容优先
- **可插拔** — LLM Provider 通过 trait 抽象，换模型只需改配置
