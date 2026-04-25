//! 论文和摘要数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// arXiv 论文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    /// arXiv ID (如 "2401.12345")
    pub id: String,

    /// 论文标题
    pub title: String,

    /// 作者列表
    pub authors: Vec<String>,

    /// 摘要文本
    pub abstract_text: String,

    /// 分类标签
    pub categories: Vec<String>,

    /// 发布时间
    pub published: DateTime<Utc>,

    /// 更新时间
    pub updated: DateTime<Utc>,

    /// PDF 下载地址
    pub pdf_url: String,

    /// LLM 评分
    pub relevance_score: Option<f64>,

    /// 生成的摘要
    pub summary: Option<Summary>,

    /// 是否已读
    pub is_read: bool,
}

impl Paper {
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            authors: Vec::new(),
            abstract_text: String::new(),
            categories: Vec::new(),
            published: Utc::now(),
            updated: Utc::now(),
            pdf_url: String::new(),
            relevance_score: None,
            summary: None,
            is_read: false,
        }
    }
}

/// 论文摘要（由 LLM 生成）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// 唯一 ID
    pub id: String,

    /// 所属论文 ID
    pub paper_id: String,

    /// 短摘要（用于语音播报）
    pub short_summary: String,

    /// 详细摘要
    pub detailed_summary: String,

    /// 关键点
    pub key_points: Vec<String>,

    /// 生成时间
    pub generated_at: DateTime<Utc>,

    /// 使用的 Provider
    pub provider: String,
}

/// 用户偏好设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// 关注的关键词
    pub keywords: Vec<String>,

    /// 关注的分类
    pub categories: Vec<String>,

    /// 排除的分类
    pub exclude_categories: Vec<String>,

    /// 抓取间隔（分钟）
    pub fetch_interval_minutes: u64,

    /// 每次抓取最大论文数
    pub max_papers_per_fetch: usize,

    /// 语速 (0.5 - 2.0)
    pub voice_speed: f64,

    /// 音量 (0.0 - 1.0)
    pub voice_volume: f64,

    /// LLM Provider 名称
    pub llm_provider: String,

    /// 语音识别 Provider 名称
    pub speech_provider: String,

    /// 语音合成 Provider 名称
    pub synthesizer_provider: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            categories: Vec::new(),
            exclude_categories: Vec::new(),
            fetch_interval_minutes: 60,
            max_papers_per_fetch: 5,
            voice_speed: 1.0,
            voice_volume: 1.0,
            llm_provider: "minimax".to_string(),
            speech_provider: "groq_whisper".to_string(),
            synthesizer_provider: "system_say".to_string(),
        }
    }
}
