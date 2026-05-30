# 数据流

## 搜索数据流

```
用户输入
  │
  ├─ 中文？─── POST /api/translate ──→ LLM 翻译 ──→ 英文关键词
  │                                                │
  └─ 英文？─────────────────────────────────────────┘
                                                    │
                                            GET /api/search
                                                    │
                                          ┌─────────┴─────────┐
                                          │                   │
                                     ArxivClient      SemanticScholarClient
                                          │                   │
                                     arXiv API          S2 API
                                          │                   │
                                     Paper[]            Paper[]
                                          │                   │
                                          └────────┬──────────┘
                                                   │
                                           合并去重 (HashSet<id>)
                                                   │
                                           按日期降序排列
                                                   │
                                           Filter::filter_and_score()
                                                   │
                                             JSON → TUI
```

## 快捷查询数据流

```
按 r (robotics preset)
  │
  └─→ POST /api/quick-search?preset=robotics
        │
        ├─→ ArxivClient.search_multi_categories(["cs.RO","cs.AI","cs.CV","cs.LG"])
        │     │
        │     └─→ 每个分类串行查询，间隔 3 秒防限速
        │
        └─→ SemanticScholarClient.search("robotics", limit, None)
              │
              └─→ 并行执行 (tokio::join!)
                    │
                    └─→ 合并 → 去重 → 排序 → JSON

TUI 端同时做加载动画 (CategoryProgress[])
```

## 定时监控数据流

```
Server 启动
  │
  └─→ tokio::spawn(Scheduler::start(db))
        │
        ├─→ 立即执行一次 fetch_and_save()
        │     │
        │     ├─→ ArxivClient.search_by_category (每个分类)
        │     ├─→ SemanticScholarClient.search (每个关键词)
        │     └─→ db.save_paper() (新论文入库)
        │
        └─→ loop { tick(interval).await; fetch_and_save(); }
              (默认每 60 分钟)
```

## 翻译数据流

```
POST /api/translate {"text": "机器人抓取"}
  │
  └─→ handlers::translate_query()
        │
        ├─→ 检查 config.api_key 是否存在
        │     └─ 不存在 → 返回 success: false
        │
        ├─→ create_llm(&config)
        │     ├─ llm_provider = "openai" → OpenAiProvider
        │     └─ llm_provider = "minimax" → MiniMaxProvider
        │
        ├─→ llm.complete(prompt)
        │     │
        │     └─→ POST {base_url}/v1/chat/completions
        │           │
        │           └─→ 解析 ChatResponse.choices[0].message.content
        │
        └─→ 清理 <think> 标签 → 返回翻译结果
```

## 收藏保存数据流

```
POST /api/papers/save {id, title, authors, abstract_text, ...}
  │
  └─→ handlers::save_paper()
        │
        ├─→ 解析 published 日期 (RFC3339 → DateTime<Utc>)
        ├─→ 构造 Paper 结构体
        └─→ db.save_paper(&paper)
              │
              └─→ SQL: INSERT OR REPLACE INTO papers (...)
```
