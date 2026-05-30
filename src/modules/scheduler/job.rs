//! 定时任务调度器
//!
//! 后台定期查询最新论文并保存到数据库

use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

use crate::core::{Paper, UserPreferences};
use crate::modules::arxiv::ArxivClient;
use crate::modules::semantic_scholar::SemanticScholarClient;
use crate::modules::storage::Database;

/// 定时任务调度器
pub struct Scheduler {
    /// Fetch interval in minutes
    interval_minutes: u64,
    /// arXiv categories to monitor
    categories: Vec<String>,
    /// Keywords to search
    keywords: Vec<String>,
}

impl Scheduler {
    pub fn new(interval_minutes: u64) -> Self {
        Self {
            interval_minutes: interval_minutes.max(30), // minimum 30 minutes
            categories: vec!["cs.RO".into(), "cs.AI".into(), "cs.CV".into()],
            keywords: vec!["robotics".into()],
        }
    }

    /// Load preferences and update config
    pub fn from_preferences(prefs: &UserPreferences) -> Self {
        Self {
            interval_minutes: prefs.fetch_interval_minutes.max(30),
            categories: if prefs.categories.is_empty() {
                vec!["cs.RO".into(), "cs.AI".into(), "cs.CV".into()]
            } else {
                prefs.categories.clone()
            },
            keywords: if prefs.keywords.is_empty() {
                vec!["robotics".into()]
            } else {
                prefs.keywords.clone()
            },
        }
    }

    /// Start the scheduler loop
    pub async fn start(self, db: Database) {
        let duration = Duration::from_secs(self.interval_minutes * 60);
        info!(
            "定时任务已启动: 每 {} 分钟检查一次 {} 个分类的论文",
            self.interval_minutes,
            self.categories.len()
        );

        // Fetch immediately on start
        self.fetch_and_save(&db).await;

        let mut ticker = interval(duration);
        loop {
            ticker.tick().await;
            self.fetch_and_save(&db).await;
        }
    }

    async fn fetch_and_save(&self, db: &Database) {
        info!("定时任务: 开始获取最新论文...");

        let arxiv = ArxivClient::new();
        let ss = SemanticScholarClient::new();

        let mut new_count = 0;

        // Fetch from arXiv categories
        for cat in &self.categories {
            match arxiv.search_by_category(cat, 3).await {
                Ok(papers) => {
                    for paper in papers {
                        if db.get_paper(&paper.id).await.ok().flatten().is_none() {
                            if let Err(e) = db.save_paper(&paper).await {
                                warn!("保存论文失败 {}: {}", paper.id, e);
                            } else {
                                new_count += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("arXiv {} 查询失败: {}", cat, e);
                }
            }
            // Rate limit delay
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // Fetch from Semantic Scholar
        for kw in &self.keywords {
            match ss.search(kw, 3, None).await {
                Ok(papers) => {
                    for paper in papers {
                        if db.get_paper(&paper.id).await.ok().flatten().is_none() {
                            if let Err(e) = db.save_paper(&paper).await {
                                warn!("保存论文失败 {}: {}", paper.id, e);
                            } else {
                                new_count += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Semantic Scholar 查询失败: {}", e);
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        if new_count > 0 {
            info!("定时任务: 发现 {} 篇新论文并已保存", new_count);
        } else {
            info!("定时任务: 没有发现新论文");
        }
    }
}
