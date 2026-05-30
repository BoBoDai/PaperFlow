# Paper 模型

`src/core/paper.rs` 定义了论文和用户偏好的核心数据结构。

## Paper

```rust
pub struct Paper {
    pub id: String,               // arXiv ID 或 Semantic Scholar ID
    pub title: String,            // 论文标题
    pub authors: Vec<String>,     // 作者列表
    pub abstract_text: String,    // 原始摘要
    pub categories: Vec<String>,  // 分类标签 (如 ["cs.RO", "cs.AI"])
    pub published: DateTime<Utc>, // 发布日期
    pub updated: DateTime<Utc>,   // 更新日期
    pub pdf_url: String,          // PDF 下载链接
    pub relevance_score: Option<f64>, // LLM 相关性评分 (0-10)
    pub summary: Option<Summary>,     // LLM 生成的摘要
    pub source: String,           // "arxiv" | "semantic_scholar"
    pub venue: Option<String>,    // 期刊/会议名 (如 "ICRA 2025")
    pub is_read: bool,            // 是否已读
}
```

### 字段说明

- **`id`** — 唯一标识。arXiv 论文用 ID（如 `2401.12345`），Semantic Scholar 论文优先用 ArXiv ID，没有则用 paperId
- **`source`** — 区分数据来源，前端据此显示不同标签（⭐ arXiv / 📰 ICRA 2025）
- **`venue`** — Semantic Scholar 特有，标注发表期刊或会议名称。arXiv 论文此字段为 `None`
- **`relevance_score`** — `Filter::score_paper()` 基于关键词匹配和机构加分计算的分数，也复用 Semantic Scholar 的引用数

## Summary

LLM 生成的论文摘要结构：

```rust
pub struct Summary {
    pub id: String,                // UUID
    pub paper_id: String,          // 关联的论文 ID
    pub short_summary: String,     // 短摘要（用于语音播报）
    pub detailed_summary: String,  // 详细分析
    pub key_points: Vec<String>,   // 关键发现列表
    pub generated_at: DateTime<Utc>,
    pub provider: String,          // "openai" | "minimax"
}
```

## UserPreferences

```rust
pub struct UserPreferences {
    pub keywords: Vec<String>,            // 关注的关键词
    pub categories: Vec<String>,          // 关注的分类
    pub exclude_categories: Vec<String>,  // 排除的分类
    pub fetch_interval_minutes: u64,      // 抓取间隔 (默认 60)
    pub max_papers_per_fetch: usize,      // 每次最大论文数 (默认 5)
    pub voice_speed: f64,                 // 语速 (0.5-2.0)
    pub voice_volume: f64,                // 音量 (0.0-1.0)
    pub llm_provider: String,             // "minimax" | "openai"
    pub speech_provider: String,
    pub synthesizer_provider: String,
}
```

当前 `UserPreferences` 主要用于 `Filter::filter_and_score()` 的评分依据。定时调度器也从中读取关注分类和关键词。
