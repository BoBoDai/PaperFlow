# 存储层

`src/modules/storage/db.rs` — SQLite 数据库管理，基于 sqlx。

## Database 结构

```rust
pub struct Database {
    pool: Pool<Sqlite>,  // sqlx 连接池
}
```

`clone()` 只是增加 `Arc` 引用计数，多个 handler 和 scheduler 可以安全共享。

## 表结构

### papers

```sql
CREATE TABLE IF NOT EXISTS papers (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    authors TEXT NOT NULL,       -- JSON: ["Zhang", "Li"]
    abstract_text TEXT NOT NULL,
    categories TEXT NOT NULL,    -- JSON: ["cs.RO", "cs.AI"]
    published INTEGER NOT NULL,  -- Unix timestamp
    updated INTEGER NOT NULL,
    pdf_url TEXT NOT NULL,
    relevance_score REAL,
    is_read INTEGER NOT NULL DEFAULT 0
);
```

`authors` 和 `categories` 存储为 JSON 字符串，读时 `serde_json::from_str` 还原。

### summaries

```sql
CREATE TABLE IF NOT EXISTS summaries (
    id TEXT PRIMARY KEY,
    paper_id TEXT NOT NULL,
    short_summary TEXT NOT NULL,
    detailed_summary TEXT NOT NULL,
    key_points TEXT NOT NULL,    -- JSON
    generated_at INTEGER NOT NULL,
    provider TEXT NOT NULL
);
```

### preferences

```sql
CREATE TABLE IF NOT EXISTS preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- 单行表
    keywords TEXT NOT NULL,
    categories TEXT NOT NULL,
    fetch_interval_minutes INTEGER DEFAULT 60,
    max_papers_per_fetch INTEGER DEFAULT 5,
    voice_speed REAL DEFAULT 1.0,
    llm_provider TEXT DEFAULT 'openai',
    ...
);
```

`CHECK (id = 1)` 确保只有一行用户偏好配置。用 `INSERT OR REPLACE` 更新。

## Row → Struct 映射

```rust
impl From<PaperRow> for Paper {
    fn from(row: PaperRow) -> Self {
        Paper {
            id: row.id,
            title: row.title,
            authors: serde_json::from_str(&row.authors).unwrap_or_default(),
            categories: serde_json::from_str(&row.categories).unwrap_or_default(),
            published: DateTime::from_timestamp(row.published, 0).unwrap_or_default(),
            source: "arxiv".to_string(),    // 数据库不存 source，默认 arxiv
            venue: None,
            ...
        }
    }
}
```

注意 `source` 和 `venue` 不在数据库 schema 中——它们是运行时从 API 响应直接获取的，存入数据库时默认值。

## 关键方法

| 方法 | SQL | 用途 |
|------|-----|------|
| `save_paper` | `INSERT OR REPLACE` | 保存论文（幂等）|
| `get_paper` | `SELECT ... WHERE id = ?` | 检查论文是否存在 |
| `list_papers` | `SELECT ... ORDER BY published DESC LIMIT ?` | 列出全部论文 |
| `list_unread_papers` | `SELECT ... WHERE is_read = 0 ...` | 列出未读论文 |
| `mark_paper_read` | `UPDATE ... SET is_read = 1` | 标记已读 |
