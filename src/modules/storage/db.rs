//! SQLite 数据库管理

use sqlx::{Pool, Sqlite, SqlitePool};
use std::path::Path;

use crate::core::{Error, Paper, Summary, UserPreferences, Result};

/// SQLite 数据库管理器
#[derive(Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// 创建数据库连接
    pub async fn new(db_path: &str) -> Result<Self> {
        let database_url = if db_path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            // 确保目录存在
            let path = Path::new(db_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| Error::Io(e.to_string()))?;
            }
            format!("sqlite:{}?mode=rwc", db_path)
        };

        let pool = SqlitePool::connect(&database_url).await
            .map_err(|e| Error::Storage(e.to_string()))?;

        // 初始化表结构
        Self::init_schema(&pool).await?;

        Ok(Self { pool })
    }

    /// 初始化表结构
    async fn init_schema(pool: &Pool<Sqlite>) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS papers (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                authors TEXT NOT NULL,
                abstract_text TEXT NOT NULL,
                categories TEXT NOT NULL,
                published INTEGER NOT NULL,
                updated INTEGER NOT NULL,
                pdf_url TEXT NOT NULL,
                relevance_score REAL,
                is_read INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS summaries (
                id TEXT PRIMARY KEY,
                paper_id TEXT NOT NULL,
                short_summary TEXT NOT NULL,
                detailed_summary TEXT NOT NULL,
                key_points TEXT NOT NULL,
                generated_at INTEGER NOT NULL,
                provider TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS preferences (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                keywords TEXT NOT NULL,
                categories TEXT NOT NULL,
                exclude_categories TEXT NOT NULL,
                fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
                max_papers_per_fetch INTEGER NOT NULL DEFAULT 5,
                voice_speed REAL NOT NULL DEFAULT 1.0,
                voice_volume REAL NOT NULL DEFAULT 1.0,
                llm_provider TEXT NOT NULL DEFAULT 'minimax',
                speech_provider TEXT NOT NULL DEFAULT 'groq_whisper',
                synthesizer_provider TEXT NOT NULL DEFAULT 'system_say'
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_papers_published ON papers(published DESC)"
        )
        .execute(pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_papers_is_read ON papers(is_read)"
        )
        .execute(pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(())
    }

    /// 保存论文
    pub async fn save_paper(&self, paper: &Paper) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO papers (id, title, authors, abstract_text, categories, published, updated, pdf_url, relevance_score, is_read)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&paper.id)
        .bind(&paper.title)
        .bind(serde_json::to_string(&paper.authors).unwrap_or_default())
        .bind(&paper.abstract_text)
        .bind(serde_json::to_string(&paper.categories).unwrap_or_default())
        .bind(paper.published.timestamp())
        .bind(paper.updated.timestamp())
        .bind(&paper.pdf_url)
        .bind(paper.relevance_score)
        .bind(paper.is_read)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    /// 批量保存论文
    pub async fn save_papers(&self, papers: &[Paper]) -> Result<()> {
        for paper in papers {
            self.save_paper(paper).await?;
        }
        Ok(())
    }

    /// 获取论文
    pub async fn get_paper(&self, id: &str) -> Result<Option<Paper>> {
        let row: Option<PaperRow> = sqlx::query_as(
            "SELECT * FROM papers WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    /// 列出未读论文
    pub async fn list_unread_papers(&self, limit: usize) -> Result<Vec<Paper>> {
        let rows: Vec<PaperRow> = sqlx::query_as(
            "SELECT * FROM papers WHERE is_read = 0 ORDER BY published DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// 列出所有论文
    pub async fn list_papers(&self, limit: usize) -> Result<Vec<Paper>> {
        let rows: Vec<PaperRow> = sqlx::query_as(
            "SELECT * FROM papers ORDER BY published DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// 标记论文为已读
    pub async fn mark_paper_read(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE papers SET is_read = 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    /// 保存摘要
    pub async fn save_summary(&self, summary: &Summary) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO summaries (id, paper_id, short_summary, detailed_summary, key_points, generated_at, provider)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&summary.id)
        .bind(&summary.paper_id)
        .bind(&summary.short_summary)
        .bind(&summary.detailed_summary)
        .bind(serde_json::to_string(&summary.key_points).unwrap_or_default())
        .bind(summary.generated_at.timestamp())
        .bind(&summary.provider)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    /// 获取论文摘要
    pub async fn get_summary(&self, paper_id: &str) -> Result<Option<Summary>> {
        let row: Option<SummaryRow> = sqlx::query_as(
            "SELECT * FROM summaries WHERE paper_id = ?"
        )
        .bind(paper_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    /// 获取用户偏好
    pub async fn get_preferences(&self) -> Result<Option<UserPreferences>> {
        let row: Option<PreferencesRow> = sqlx::query_as(
            "SELECT * FROM preferences WHERE id = 1"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    /// 保存用户偏好
    pub async fn save_preferences(&self, prefs: &UserPreferences) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO preferences (id, keywords, categories, exclude_categories, fetch_interval_minutes, max_papers_per_fetch, voice_speed, voice_volume, llm_provider, speech_provider, synthesizer_provider)
            VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(serde_json::to_string(&prefs.keywords).unwrap_or_default())
        .bind(serde_json::to_string(&prefs.categories).unwrap_or_default())
        .bind(serde_json::to_string(&prefs.exclude_categories).unwrap_or_default())
        .bind(prefs.fetch_interval_minutes as i64)
        .bind(prefs.max_papers_per_fetch as i64)
        .bind(prefs.voice_speed)
        .bind(prefs.voice_volume)
        .bind(&prefs.llm_provider)
        .bind(&prefs.speech_provider)
        .bind(&prefs.synthesizer_provider)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }
}

// === 数据库行结构 ===

#[derive(sqlx::FromRow)]
struct PaperRow {
    id: String,
    title: String,
    authors: String,
    abstract_text: String,
    categories: String,
    published: i64,
    updated: i64,
    pdf_url: String,
    relevance_score: Option<f64>,
    is_read: bool,
}

impl From<PaperRow> for Paper {
    fn from(row: PaperRow) -> Self {
        Paper {
            id: row.id,
            title: row.title,
            authors: serde_json::from_str(&row.authors).unwrap_or_default(),
            abstract_text: row.abstract_text,
            categories: serde_json::from_str(&row.categories).unwrap_or_default(),
            published: chrono::DateTime::from_timestamp(row.published, 0).unwrap_or_default(),
            updated: chrono::DateTime::from_timestamp(row.updated, 0).unwrap_or_default(),
            pdf_url: row.pdf_url,
            relevance_score: row.relevance_score,
            summary: None,
            is_read: row.is_read,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: String,
    paper_id: String,
    short_summary: String,
    detailed_summary: String,
    key_points: String,
    generated_at: i64,
    provider: String,
}

impl From<SummaryRow> for Summary {
    fn from(row: SummaryRow) -> Self {
        Summary {
            id: row.id,
            paper_id: row.paper_id,
            short_summary: row.short_summary,
            detailed_summary: row.detailed_summary,
            key_points: serde_json::from_str(&row.key_points).unwrap_or_default(),
            generated_at: chrono::DateTime::from_timestamp(row.generated_at, 0).unwrap_or_default(),
            provider: row.provider,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PreferencesRow {
    keywords: String,
    categories: String,
    exclude_categories: String,
    fetch_interval_minutes: i64,
    max_papers_per_fetch: i64,
    voice_speed: f64,
    voice_volume: f64,
    llm_provider: String,
    speech_provider: String,
    synthesizer_provider: String,
}

impl From<PreferencesRow> for UserPreferences {
    fn from(row: PreferencesRow) -> Self {
        UserPreferences {
            keywords: serde_json::from_str(&row.keywords).unwrap_or_default(),
            categories: serde_json::from_str(&row.categories).unwrap_or_default(),
            exclude_categories: serde_json::from_str(&row.exclude_categories).unwrap_or_default(),
            fetch_interval_minutes: row.fetch_interval_minutes as u64,
            max_papers_per_fetch: row.max_papers_per_fetch as usize,
            voice_speed: row.voice_speed,
            voice_volume: row.voice_volume,
            llm_provider: row.llm_provider,
            speech_provider: row.speech_provider,
            synthesizer_provider: row.synthesizer_provider,
        }
    }
}
