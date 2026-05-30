# 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                     Terminal TUI                        │
│                 ui/ (TypeScript + Ink)                  │
│  ┌─────────┐ ┌──────────┐ ┌────────┐ ┌─────────────┐  │
│  │ Welcome │ │  Search  │ │  List  │ │   Detail     │  │
│  └─────────┘ └──────────┘ └────────┘ └─────────────┘  │
│                         │                               │
│                   HTTP API (localhost:8080)              │
└─────────────────────────┼───────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────┐
│                    Rust Backend                         │
│                   Axum HTTP Server                       │
│  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌────────────┐  │
│  │ Handlers │ │  Config  │ │ Filter │ │  Scheduler  │  │
│  └────┬─────┘ └──────────┘ └───┬────┘ └──────┬─────┘  │
│       │                        │              │         │
│  ┌────┴────────────────────────┴──────────────┴─────┐  │
│  │                  Modules                          │  │
│  │  ┌───────┐ ┌──────────────┐ ┌─────┐ ┌─────────┐  │  │
│  │  │ arXiv │ │Semantic Sch. │ │ LLM │ │ Storage │  │  │
│  │  └───┬───┘ └──────┬───────┘ └──┬──┘ └────┬────┘  │  │
│  └──────┼─────────────┼────────────┼─────────┼──────┘  │
│         │             │            │         │          │
│    arXiv API    S2 API    OpenAI API    SQLite DB       │
└─────────┼─────────────┼────────────┼─────────┼──────────┘
```

## 模块职责

| 模块 | 路径 | 职责 |
|------|------|------|
| `api` | `src/api/` | HTTP 路由和请求处理，共享状态管理 |
| `core` | `src/core/` | 核心数据类型、错误类型、配置管理 |
| `arxiv` | `src/modules/arxiv/` | arXiv Atom Feed API 客户端 |
| `semantic_scholar` | `src/modules/semantic_scholar/` | Semantic Scholar REST API 客户端 |
| `llm` | `src/modules/llm/` | LLM Provider trait + OpenAI/MiniMax 实现 |
| `filter` | `src/modules/filter/` | 论文关键词匹配和相关性评分 |
| `scheduler` | `src/modules/scheduler/` | 后台定时拉取论文任务 |
| `storage` | `src/modules/storage/` | SQLite 数据库读写 |
| `speech` | `src/modules/speech/` | 语音识别和合成（预留） |

## 请求生命周期

以「用户输入中文搜索」为例：

```
1. TUI: 用户输入 "机器人抓取" 按 Enter
2. App.tsx: handleSearch("机器人抓取")
3. 检测中文 → setMode('translating')
4. POST /api/translate {"text":"机器人抓取"}
5. handlers.rs: translate_query()
6. create_llm() → OpenAiProvider.complete("translate...")
7. LLM 返回 "robot grasping"
8. GET /api/search?q=robot+grasping&max_results=5
9. handlers.rs: search_papers()
10. ArxivClient.search("robot grasping", 5)
11. arXiv API → 返回 Atom XML → 解析为 Paper[]
12. Paper[] 转为 JSON → 返回 TUI
13. TUI: setPapers() → setMode('list')
14. 用户在列表选择论文 → setMode('detail')
15. POST /api/translate-paper + POST /api/summarize (并行)
16. 详情页展示中英双语 + LLM 智能分析
```
