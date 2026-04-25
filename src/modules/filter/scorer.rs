//! 论文筛选和评分

use crate::core::{Paper, UserPreferences};

/// 论文过滤器
pub struct Filter;

impl Filter {
    /// 根据用户偏好筛选论文
    pub fn filter_papers(papers: Vec<Paper>, prefs: &UserPreferences) -> Vec<Paper> {
        papers.into_iter()
            .filter(|paper| Self::matches_preferences(paper, prefs))
            .collect()
    }

    /// 检查论文是否匹配用户偏好
    fn matches_preferences(paper: &Paper, prefs: &UserPreferences) -> bool {
        // 检查排除分类
        for cat in &paper.categories {
            if prefs.exclude_categories.contains(cat) {
                return false;
            }
        }

        // 检查包含分类（如果指定了分类）
        if !prefs.categories.is_empty() {
            let has_matching_category = prefs.categories.iter().any(|c| {
                paper.categories.iter().any(|pc| pc.to_lowercase().contains(&c.to_lowercase()))
            });
            if !has_matching_category {
                return false;
            }
        }

        // 检查关键词（标题和摘要）
        if !prefs.keywords.is_empty() {
            let text = format!("{} {}", paper.title, paper.abstract_text).to_lowercase();
            let has_matching_keyword = prefs.keywords.iter().any(|k| {
                text.contains(&k.to_lowercase())
            });
            if !has_matching_keyword {
                return false;
            }
        }

        true
    }

    /// 计算论文相关性评分
    pub fn score_paper(paper: &Paper, prefs: &UserPreferences) -> f64 {
        let mut score = 0.0;

        // 分类匹配加分
        for cat in &paper.categories {
            if prefs.categories.contains(cat) {
                score += 2.0;
            }
        }

        // 关键词匹配加分
        let text = format!("{} {}", paper.title, paper.abstract_text).to_lowercase();
        for keyword in &prefs.keywords {
            if text.contains(&keyword.to_lowercase()) {
                score += 1.0;
            }
        }

        // 顶级机构加分
        let top_institutions = ["openai", "deepmind", "google", "meta", "microsoft", "stanford", "mit"];
        let author_text = paper.authors.join(" ").to_lowercase();
        for inst in top_institutions {
            if author_text.contains(inst) {
                score += 2.0;
                break;
            }
        }

        // Survey/Review 加分
        if paper.title.to_lowercase().contains("survey")
            || paper.title.to_lowercase().contains("review") {
            score += 1.0;
        }

        score
    }

    /// 筛选并评分
    pub fn filter_and_score(papers: Vec<Paper>, prefs: &UserPreferences) -> Vec<Paper> {
        let filtered = Self::filter_papers(papers, prefs);

        filtered.into_iter()
            .map(|mut paper| {
                paper.relevance_score = Some(Self::score_paper(&paper, prefs));
                paper
            })
            .collect()
    }
}
