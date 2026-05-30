# 搜索接口

## GET /api/search

普通关键词搜索，支持 arXiv 的 `cat:` 语法。

**参数**：`q`（查询词）、`max_results`（默认 5）

**实现**：`handlers::search_papers()`

```rust
pub async fn search_papers(
    State(state): State<ApiState>,
    Query(query): Query<SearchQuery>,
) -> Json<SearchResponse> {
    let max = query.max_results.unwrap_or(5);
    let client = ArxivClient::new();

    match client.search(&query.q, max).await {
        Ok(papers) => {
            let prefs = UserPreferences {
                keywords: vec![query.q.clone()],
                max_papers_per_fetch: max,
                ..Default::default()
            };
            let scored = Filter::filter_and_score(papers, &prefs);
            let responses: Vec<PaperResponse> = scored.into_iter().map(|p| p.into()).collect();
            Json(SearchResponse { papers: responses })
        }
        Err(e) => {
            tracing::error!("搜索失败: {}", e);
            Json(SearchResponse { papers: vec![] })
        }
    }
}
```

错误不抛给前端，返回空数组。前端根据空数组显示提示信息。

## GET /api/quick-search

预设快捷查询，同时查询 arXiv 和 Semantic Scholar。

**参数**：`preset`（robotics/ai/cv）、`max_results`

**实现**：`handlers::quick_search()`

```rust
// 并行查询两个数据源
let (arxiv_result, ss_result) = tokio::join!(
    arxiv_client.search_multi_categories(&preset.categories, per_category),
    ss_client.search(&query.preset, per_category, None),
);

// 合并去重
let mut seen_ids = HashSet::new();
for p in arxiv_result { if seen_ids.insert(p.id.clone()) { all_papers.push(p); } }
for p in ss_result { if seen_ids.insert(p.id.clone()) { all_papers.push(p); } }
```

`tokio::join!` 让 arXiv 和 Semantic Scholar 真正并行执行，等待两者都完成再合并结果。

## PaperResponse

```rust
pub struct PaperResponse {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,        // abstract_text 的别名
    pub published: String,      // "2025-05-28"
    pub categories: Vec<String>,
    pub pdf_url: String,
    pub source: String,         // "arxiv" | "semantic_scholar"
    pub venue: Option<String>,  // 期刊/会议名
}
```

前端用 `PaperResponse` 而非原始 `Paper` 结构体，字段命名更符合前端习惯（`pdf_url` → `pdfUrl` 在前端做转换）。
