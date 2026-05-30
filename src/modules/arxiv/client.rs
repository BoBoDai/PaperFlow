//! arXiv API 客户端
//!
//! 使用 arXiv 的 Atom feed API 获取论文

use chrono::{DateTime, Utc};
use reqwest::Client;
use regex::Regex;

use crate::core::{Error, Paper, Result};

/// arXiv 客户端
#[derive(Clone)]
pub struct ArxivClient {
    http_client: Client,
    base_url: String,
}

impl ArxivClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .no_proxy()
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url: "https://export.arxiv.org/api/query".to_string(),
        }
    }

    /// 搜索论文
    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<Paper>> {
        let url = format!(
            "{}?search_query={}&start=0&max_results={}&sortBy=submittedDate&sortOrder=descending",
            self.base_url, query, max_results
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;

        let body = response.text().await
            .map_err(|e| Error::Network(e.to_string()))?;

        self.parse_feed(&body)
    }

    /// 按分类搜索近期论文
    /// category: arXiv 分类如 "cs.RO", "cs.AI", "cs.CV"
    pub async fn search_by_category(
        &self,
        category: &str,
        max_results: usize,
    ) -> Result<Vec<Paper>> {
        let query = format!("cat:{}", category);
        self.search(&query, max_results).await
    }

    /// 并行搜索多个分类的最新论文（带延迟以避免被限速）
    pub async fn search_multi_categories(
        &self,
        categories: &[&str],
        max_per_category: usize,
    ) -> Result<Vec<Paper>> {
        let mut all_papers: Vec<Paper> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for (i, category) in categories.iter().enumerate() {
            // arXiv API rate limit: add 3s delay between requests (except first)
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }

            match self.search_by_category(category, max_per_category).await {
                Ok(papers) => {
                    tracing::info!("arXiv {} → {} 篇论文", category, papers.len());
                    for paper in papers {
                        if seen_ids.insert(paper.id.clone()) {
                            all_papers.push(paper);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("arXiv {} 查询失败: {}", category, e);
                }
            }
        }

        // Sort by published date descending
        all_papers.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(all_papers)
    }

    /// 解析 Atom feed
    fn parse_feed(&self, xml: &str) -> Result<Vec<Paper>> {
        // 提取所有 entry 块 - 使用 (?s) 让 . 匹配换行符
        let entry_regex = Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap();
        let id_regex = Regex::new(r"<id>http://arxiv\.org/abs/([^<]+)</id>").unwrap();
        let title_regex = Regex::new(r"<title>(.*?)</title>").unwrap();
        let summary_regex = Regex::new(r"<summary>(.*?)</summary>").unwrap();
        let published_regex = Regex::new(r"<published>(.*?)</published>").unwrap();
        let updated_regex = Regex::new(r"<updated>(.*?)</updated>").unwrap();
        let name_regex = Regex::new(r"<name>(.*?)</name>").unwrap();
        let category_regex = Regex::new(r#"category term="([^"]+)""#).unwrap();
        let pdf_regex = Regex::new(r#"link[^>]*title="pdf"[^>]*href="([^"]+)""#).unwrap();
        let pdf_alt_regex = Regex::new(r#"href="([^"]*\.pdf[^"]*)""#).unwrap();

        let mut papers = Vec::new();

        for cap in entry_regex.captures_iter(xml) {
            let entry = &cap[1];

            // 提取 ID
            let id = id_regex.captures(entry)
                .map(|c| c[1].to_string())
                .unwrap_or_default();

            // 提取标题
            let title = title_regex.captures(entry)
                .map(|c| c[1].replace('\n', " ").trim().to_string())
                .unwrap_or_default();

            // 提取摘要
            let summary = summary_regex.captures(entry)
                .map(|c| c[1].replace('\n', " ").trim().to_string())
                .unwrap_or_default();

            // 提取日期
            let published = published_regex.captures(entry)
                .and_then(|c| DateTime::parse_from_rfc3339(&c[1]).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            let updated = updated_regex.captures(entry)
                .and_then(|c| DateTime::parse_from_rfc3339(&c[1]).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(published);

            // 提取作者
            let authors: Vec<String> = name_regex.captures_iter(entry)
                .map(|c| c[1].to_string())
                .collect();

            // 提取分类
            let categories: Vec<String> = category_regex.captures_iter(entry)
                .map(|c| c[1].to_string())
                .collect();

            // 提取 PDF URL
            let pdf_url = pdf_regex.captures(entry)
                .map(|c| c[1].to_string())
                .or_else(|| {
                    pdf_alt_regex.captures(entry)
                        .map(|c| c[1].to_string())
                })
                .unwrap_or_default();

            if id.is_empty() || title.is_empty() {
                continue;
            }

            papers.push(Paper {
                id: id.clone(),
                title,
                authors,
                abstract_text: summary,
                categories: categories.clone(),
                published,
                updated,
                pdf_url,
                relevance_score: None,
                summary: None,
                source: "arxiv".to_string(),
                venue: None,
                is_read: false,
            });
        }

        Ok(papers)
    }
}

impl Default for ArxivClient {
    fn default() -> Self {
        Self::new()
    }
}
