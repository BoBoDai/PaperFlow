//! 存储适配器
//!
//! 真正的实现用 SQLite（Drizzle 那一侧的对应物是 sqlx 或 rusqlite）。
//! 表结构要点见 `docs/`：
//!
//! ```sql
//! papers(id PK, title, authors, abstract, categories, published,
//!        updated, pdf_url, source, venue, relevance_score, is_read, saved_at)
//!
//! summaries(paper_id, model, prompt_version, short, detailed, key_points,
//!           created_at, PRIMARY KEY (paper_id, model, prompt_version))
//!           -- ↑ 模型和 prompt 版本进主键：换模型或改 prompt，旧结果自动失效
//!
//! subscriptions(id PK, kind, value, max_per_run, min_score, enabled)
//! papers_fts  -- FTS5 虚拟表，索引 title + abstract
//! ```

use paperflow_core::{Paper, Result, Summary};

/// 论文持久化
#[allow(async_fn_in_trait)]
pub trait PaperStore {
    async fn save(&self, paper: &Paper) -> Result<()>;
    async fn list_saved(&self, limit: usize) -> Result<Vec<Paper>>;
    async fn mark_read(&self, id: &str) -> Result<()>;

    /// 按 (paper_id, model, prompt_version) 取缓存的摘要
    async fn get_summary(&self, paper_id: &str, model: &str, prompt_version: &str)
    -> Result<Option<Summary>>;

    async fn put_summary(&self, paper_id: &str, summary: &Summary) -> Result<()>;
}
