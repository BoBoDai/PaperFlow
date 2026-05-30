# arXiv 客户端

`src/modules/arxiv/client.rs` 封装 arXiv Atom Feed API 的调用。

## API 说明

arXiv 提供免费的 REST API，基于 Atom XML 格式，无需认证。

```
GET https://export.arxiv.org/api/query
  ?search_query=cat:cs.RO
  &start=0
  &max_results=5
  &sortBy=submittedDate
  &sortOrder=descending
```

限速：无官方文档，实测约 1 req/s，超出返回 HTTP 429。

## ArxivClient

```rust
pub struct ArxivClient {
    http_client: Client,    // reqwest::Client (no_proxy)
    base_url: String,       // "https://export.arxiv.org/api/query"
}
```

### 禁用代理

```rust
Client::builder().no_proxy().build()
```

许多开发者环境配置了 HTTP 代理，arXiv 不需要代理。

## 核心方法

### search — 通用搜索

```rust
pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<Paper>>
```

构造 URL 并 GET 请求，返回的 XML 交给 `parse_feed()` 解析。

### search_by_category — 按分类搜索

```rust
pub async fn search_by_category(&self, category: &str, max_results: usize) -> Result<Vec<Paper>>
```

内部调用 `search(&format!("cat:{}", category), max_results)`。arXiv 的 `cat:` 前缀用于过滤分类。

### search_multi_categories — 多分类串行查询

```rust
pub async fn search_multi_categories(&self, categories: &[&str], max_per_category: usize) -> Result<Vec<Paper>>
```

依次查询每个分类，每次查询间 sleep 3 秒防止限速。使用 `HashSet<String>` 根据 ID 去重。

```rust
for (i, category) in categories.iter().enumerate() {
    if i > 0 {
        tokio::time::sleep(Duration::from_secs(3)).await;  // 防限速
    }
    match self.search_by_category(category, max_per_category).await {
        Ok(papers) => {
            for paper in papers {
                if seen_ids.insert(paper.id.clone()) {
                    all_papers.push(paper);
                }
            }
        }
        Err(e) => tracing::warn!("arXiv {} 查询失败: {}", category, e),
    }
}
// 按发布日期降序排列
all_papers.sort_by(|a, b| b.published.cmp(&a.published));
```

## XML 解析

使用正则表达式解析 Atom Feed XML：

- `<entry>...</entry>` 块提取 — 每篇论文一个 entry
- `<id>` — arXiv ID
- `<title>` — 标题（去除换行）
- `<summary>` — 摘要（去除换行）
- `<published>` — 发布日期（RFC3339 格式）
- `<name>` — 作者名
- `category term="..."` — 分类标签
- `link title="pdf" href="..."` — PDF 链接

为什么用正则而不是 XML 解析器？arXiv 的 Atom Feed 结构简单且稳定，正则表达式比引入 XML 依赖更轻量。
