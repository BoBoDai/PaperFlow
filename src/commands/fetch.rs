//! fetch 命令

use crate::config::Settings;
use crate::modules::arxiv::ArxivClient;
use crate::modules::filter::Filter;
use crate::modules::storage::Database;
use crate::core::{UserPreferences, Result};

/// 抓取论文命令
pub struct FetchCommand;

impl FetchCommand {
    /// 执行抓取
    pub async fn run(settings: &Settings, db: &Database) -> Result<()> {
        let client = ArxivClient::new();

        // 优先使用 CLI 参数中的 keywords，否则使用数据库中的偏好设置
        let keywords = if let Some(ref kws) = settings.cli.keywords {
            kws.split(',').map(|s| s.trim().to_string()).collect()
        } else if let Some(prefs) = db.get_preferences().await? {
            prefs.keywords
        } else {
            Vec::new()
        };

        let max_papers = if settings.cli.max_papers != 5 {
            settings.cli.max_papers
        } else if let Some(prefs) = db.get_preferences().await? {
            prefs.max_papers_per_fetch
        } else {
            settings.cli.max_papers
        };

        // 构建搜索查询
        let query = if keywords.is_empty() {
            "all".to_string()
        } else {
            keywords.iter().map(|k| {
                if k.contains(":") {
                    k.clone()
                } else {
                    format!("all:{}", k)
                }
            }).collect::<Vec<_>>().join(" OR ")
        };

        println!("正在搜索 arXiv: {}...", query);

        // 搜索论文
        let papers = client.search(&query, max_papers).await?;
        println!("找到 {} 篇论文", papers.len());

        if papers.is_empty() {
            return Ok(());
        }

        // 构建偏好设置用于筛选
        let prefs = UserPreferences {
            keywords: keywords.clone(),
            max_papers_per_fetch: max_papers,
            ..Default::default()
        };

        // 筛选并评分
        let scored_papers = Filter::filter_and_score(papers, &prefs);

        // 按评分排序
        let mut sorted_papers = scored_papers;
        sorted_papers.sort_by(|a, b| {
            b.relevance_score.partial_cmp(&a.relevance_score).unwrap()
        });

        // 保存到数据库
        db.save_papers(&sorted_papers).await?;

        // 保存偏好设置
        db.save_preferences(&prefs).await?;

        println!("已保存 {} 篇论文到本地数据库", sorted_papers.len());

        // 显示前 5 篇
        for (i, paper) in sorted_papers.iter().take(5).enumerate() {
            let score = paper.relevance_score.unwrap_or(0.0);
            println!("{}. [{}] {}", i + 1, score, paper.title);
        }

        Ok(())
    }
}
