# Prompt 工程

`src/modules/llm/prompt.rs` — 管理所有 LLM 提示词模板。

## 翻译提示词

### 中文查询 → 英文关键词

```
Translate this Chinese academic query into concise English keywords
for searching academic papers. Return ONLY the English keywords,
no explanation, no punctuation.

Chinese: {query}
```

设计要点：
- `concise English keywords` — 要求返回关键词而非完整句子
- `ONLY the English keywords` — 抑制额外解释
- `no punctuation` — 避免逗号句号干扰搜索

### 论文标题和摘要 → 中文

```
Translate the following academic paper title and abstract into Chinese.
Keep technical terms accurate. Return in JSON format:
{"title": "中文标题", "abstract": "中文摘要"}

Title: {title}
Abstract: {abstract}
```

设计要点：
- `Keep technical terms accurate` — 专业术语保持准确
- JSON 格式输出 — 便于程序解析
- `extract_json_field()` 函数做容错处理：先找 `{field}":` 模式，再提取引号内值，最后回退到全文

## 摘要生成提示词

```rust
pub fn summarize_prompt(paper: &Paper) -> String {
    format!(
        "You are an academic paper analyst. Analyze the following paper and provide:\n\
         1. A one-sentence summary of the core contribution\n\
         2. A detailed analysis (2-3 sentences)\n\
         3. Key findings as a list\n\n\
         Title: {}\nAuthors: {}\nAbstract: {}\n\n\
         Respond in JSON format:\n\
         {{\"short_summary\": \"...\", \"detailed_summary\": \"...\", \"key_points\": [\"...\"]}}",
        paper.title,
        paper.authors.join(", "),
        paper.abstract_text,
    )
}
```

设计要点：
- `academic paper analyst` — 角色设定，引导专业分析
- 三层信息：一句话概述 + 详细分析 + 要点列表
- JSON 输出 — 结构化解析

## 相关性评分提示词

```rust
pub fn relevance_prompt(paper: &Paper, interests: &[String]) -> String {
    format!(
        "Rate the relevance of this paper to the user's interests on a scale of 0-10.\n\
         User interests: {}\n\
         Paper title: {}\n\
         Paper abstract: {}\n\
         Return ONLY a number between 0 and 10.",
        interests.join(", "),
        paper.title,
        paper.abstract_text,
    )
}
```

## 口语化提示词

```rust
pub fn verbalize_prompt(detailed_summary: &str) -> String {
    format!(
        "Convert the following academic summary into natural spoken Chinese.\n\
         Keep it concise, suitable for voice reading in 30 seconds.\n\n\
         Summary: {}",
        detailed_summary
    )
}
```

用于语音播报功能（预留）。
