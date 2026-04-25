//! Prompt 管理

use crate::core::Paper;

/// Prompt 管理器
pub struct PromptManager;

impl PromptManager {
    /// 生成摘要的 Prompt
    pub fn summarize_prompt(paper: &Paper) -> String {
        format!(
            r#"你是一个学术论文助手。请为以下论文生成简洁的摘要。

论文标题: {}
作者: {}
摘要: {}

请用 JSON 格式返回，包含以下字段：
- short_summary: 一句话概括论文的主要贡献（50字以内）
- detailed_summary: 详细摘要（100-200字），用新闻播报风格
- key_points: 3-5个关键点，每个点用一句话概括

要求：
1. 语言生动，适合语音播报
2. 避免使用专业术语缩写（如 CVPR, NLP 等直接说明）
3. 重点突出创新点和实验结论

JSON 格式："#,
            paper.title,
            paper.authors.join(", "),
            paper.abstract_text
        )
    }

    /// 口语化 Prompt
    pub fn verbalize_prompt(summary: &str) -> String {
        format!(
            r#"将以下论文摘要转换为适合语音播报的口语化表达。

摘要:
{}

要求：
1. 自然流畅，像新闻播报
2. 将专业术语转换为通俗表达
3. 适合在通勤时收听
4. 长度控制在200字以内
"#,
            summary
        )
    }

    /// 相关性评分 Prompt
    pub fn relevance_prompt(paper: &Paper, interests: &[String]) -> String {
        format!(
            r#"评估以下论文与用户研究兴趣的相关性。

用户兴趣: {}
论文标题: {}
论文摘要: {}

请只返回一个 0.0-10.0 的分数，其中：
- 0.0-3.0: 不相关
- 3.0-6.0: 部分相关
- 6.0-10.0: 高度相关

请只返回数字，不要其他内容。"#,
            interests.join(", "),
            paper.title,
            paper.abstract_text
        )
    }
}
