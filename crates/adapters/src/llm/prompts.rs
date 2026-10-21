//! Prompt construction for summaries and paper Q&A.

use paperflow_core::{Paper, QaRequest, QaScope};

const SUMMARY_SYSTEM_PROMPT: &str = r#"You are an academic paper analyst.
Analyze the paper and reply in Simplified Chinese using exactly this plain-text format:
一句话：<one-sentence summary>
详细分析：
<3-5 sentences of analysis>
要点：
- <key point>
- <key point>
Do not use JSON, markdown fences, or add any text outside these sections."#;

pub(crate) fn summary_messages(paper: &Paper) -> Vec<serde_json::Value> {
    let authors = paper.authors.join(", ");
    let categories = paper.categories.join(", ");
    let user = format!(
        "Title: {}\nAuthors: {}\nCategories: {}\nAbstract:\n{}",
        paper.title,
        if authors.is_empty() {
            "unknown"
        } else {
            &authors
        },
        if categories.is_empty() {
            "unknown"
        } else {
            &categories
        },
        paper.abstract_text,
    );

    vec![
        serde_json::json!({ "role": "system", "content": SUMMARY_SYSTEM_PROMPT }),
        serde_json::json!({ "role": "user", "content": user }),
    ]
}

pub(crate) const KEYWORDS_SYSTEM_PROMPT: &str = r#"Translate the user's Chinese academic search query into concise English keywords for arXiv and OpenAlex.
Preserve well-known acronyms such as LLM, RAG, GAN, and Transformer.
Reply with JSON only, exactly in this shape: {"keywords": "3-8 English keywords separated by spaces"}
Do not include Chinese characters, quotes, explanations, or markdown fences."#;

const QA_SYSTEM_PROMPT: &str = r#"You are a reading assistant embedded in an academic paper reader.
Answer strictly in Simplified Chinese.
Use only the paper metadata and context supplied in the final user message. If the context does not contain the answer, say "根据当前上下文无法回答" and suggest changing the scope (当前页 / 全文) or asking a narrower question. Never invent facts, citations, or numbers.
If the context contains page markers such as [第 3 页], cite the relevant page in your answer.
If the context contains a marker saying the middle was omitted, explicitly say that the omitted section was not available.
The scope can be 选中文本, 当前页, or 全文. For 选中文本, the excerpt may be incomplete; explain it in the paper's context.
Reply with plain text in Simplified Chinese. Do not wrap the answer in JSON or markdown fences."#;

pub(crate) fn build_qa_messages(request: &QaRequest) -> Vec<serde_json::Value> {
    let mut messages = vec![serde_json::json!({ "role": "system", "content": QA_SYSTEM_PROMPT })];

    for turn in &request.history {
        messages.push(serde_json::json!({
            "role": turn.role.as_str(),
            "content": turn.content,
        }));
    }

    messages.push(serde_json::json!({
        "role": "user",
        "content": qa_user_prompt(request),
    }));

    messages
}

fn qa_user_prompt(request: &QaRequest) -> String {
    let paper = &request.paper;
    let authors = paper
        .authors
        .iter()
        .take(12)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let authors = if authors.is_empty() {
        "unknown".to_string()
    } else {
        authors
    };
    let categories = if paper.categories.is_empty() {
        "unknown".to_string()
    } else {
        paper.categories.join(", ")
    };
    let abstract_text: String = paper.abstract_text.chars().take(3000).collect();
    let abstract_text = if abstract_text.trim().is_empty() {
        "（无摘要）".to_string()
    } else {
        abstract_text
    };

    let scope = match request.scope {
        QaScope::Selection => "选中文本",
        QaScope::CurrentPage => "当前页",
        QaScope::FullPaper => "全文",
    };
    let page = request
        .page
        .map(|page| format!("（第 {page} 页）"))
        .unwrap_or_default();

    format!(
        "论文信息\n标题: {}\n作者: {}\n分类: {}\n摘要:\n{}\n\n阅读范围: {}{}\n上下文:\n{}\n问题:\n{}",
        paper.title,
        authors,
        categories,
        abstract_text,
        scope,
        page,
        request.context,
        request.question,
    )
}
