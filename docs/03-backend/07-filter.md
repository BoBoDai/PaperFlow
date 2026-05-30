# 论文筛选与评分

`src/modules/filter/scorer.rs` — `Filter` 结构体提供论文的相关性判断和打分。

## 筛选逻辑

```rust
fn filter_papers(papers: Vec<Paper>, prefs: &UserPreferences) -> Vec<Paper>
```

三道过滤：

1. **排除分类** — `exclude_categories` 中的分类直接丢弃
2. **包含分类** — 如果设了 `categories`，只保留匹配分类的论文
3. **关键词匹配** — 标题或摘要中必须包含 `keywords` 中的至少一个词

```rust
fn matches_preferences(paper: &Paper, prefs: &UserPreferences) -> bool {
    // 排除分类检查
    for cat in &paper.categories {
        if prefs.exclude_categories.contains(cat) {
            return false;
        }
    }

    // 包含分类检查（大小写不敏感）
    if !prefs.categories.is_empty() {
        let has_match = prefs.categories.iter().any(|c| {
            paper.categories.iter().any(|pc| pc.to_lowercase().contains(&c.to_lowercase()))
        });
        if !has_match { return false; }
    }

    // 关键词检查
    if !prefs.keywords.is_empty() {
        let text = format!("{} {}", paper.title, paper.abstract_text).to_lowercase();
        let has_keyword = prefs.keywords.iter().any(|k| {
            text.contains(&k.to_lowercase())
        });
        if !has_keyword { return false; }
    }

    true
}
```

## 评分逻辑

```rust
fn score_paper(paper: &Paper, prefs: &UserPreferences) -> f64
```

打分项（每项累加）：

| 条件 | 权重 |
|------|------|
| 分类匹配 | 每个匹配分类 +2.0 |
| 关键词匹配 | 每个匹配关键词 +1.0 |
| 顶级机构作者 | +2.0（OpenAI、DeepMind、Google、Meta、Microsoft、Stanford、MIT）|
| Survey/Review 论文 | +1.0（标题含 "survey" 或 "review"）|

```rust
// 顶级机构加分
let top_institutions = ["openai", "deepmind", "google", "meta",
                         "microsoft", "stanford", "mit"];
let author_text = paper.authors.join(" ").to_lowercase();
for inst in top_institutions {
    if author_text.contains(inst) {
        score += 2.0;
        break;  // 只加一次
    }
}
```

## 使用

```rust
let scored = Filter::filter_and_score(papers, &prefs);
```

先筛选后评分，返回带 `relevance_score` 的论文列表。搜索和快捷查询 handler 在返回结果前都会调用此方法。
