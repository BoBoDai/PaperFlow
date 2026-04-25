//! list 命令

use crate::modules::storage::Database;
use crate::core::Result;

/// 列出论文命令
pub struct ListCommand;

impl ListCommand {
    /// 执行列出
    pub async fn run(db: &Database, show_all: bool) -> Result<()> {
        let papers = if show_all {
            db.list_papers(50).await?
        } else {
            db.list_unread_papers(50).await?
        };

        if papers.is_empty() {
            println!("没有找到论文，请先运行 fetch 命令");
            return Ok(());
        }

        println!("共 {} 篇论文:\n", papers.len());

        for (i, paper) in papers.iter().enumerate() {
            let read_status = if paper.is_read { "[已读]" } else { "[未读]" };
            let score = paper.relevance_score
                .map(|s| format!("{:.1}", s))
                .unwrap_or_else(|| "N/A".to_string());
            println!("{}. {} {} [评分: {}]", i + 1, read_status, paper.title, score);
            println!("   ID: {}", paper.id);
            println!("   作者: {}", paper.authors.join(", "));
            println!();
        }

        Ok(())
    }
}
