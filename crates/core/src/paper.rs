//! 论文领域模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 论文来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Arxiv,
    SemanticScholar,
    OpenAlex,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Arxiv => "arxiv",
            Source::SemanticScholar => "semantic_scholar",
            Source::OpenAlex => "openalex",
        }
    }
}

/// 一篇论文
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: String,
    pub categories: Vec<String>,
    pub published: DateTime<Utc>,
    pub pdf_url: String,
    pub source: Source,
    pub venue: Option<String>,
}

/// One page of the interest-based recent feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedPage {
    pub papers: Vec<Paper>,
    /// True when the source returned a full page before deduplication.
    pub has_more: bool,
}

impl Paper {
    /// 跨源去重用的键。
    ///
    /// arXiv 和 Semantic Scholar 对同一篇论文的 id 不同，所以用归一化标题。
    pub fn dedup_key(&self) -> String {
        self.title
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_key_ignores_case_and_punctuation() {
        let mk = |title: &str| Paper {
            id: "x".into(),
            title: title.into(),
            authors: vec![],
            abstract_text: String::new(),
            categories: vec![],
            published: Utc::now(),
            pdf_url: String::new(),
            source: Source::Arxiv,
            venue: None,
        };

        assert_eq!(
            mk("Attention Is All You Need!").dedup_key(),
            mk("attention   is all you need").dedup_key()
        );
    }
}
