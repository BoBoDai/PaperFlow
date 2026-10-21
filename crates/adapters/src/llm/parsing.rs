//! Response parsing and structural validation.

use paperflow_core::{Answer, Error, Result, Summary, contains_cjk};
use serde::Deserialize;

use super::error::{LlmEndpointError, truncate};

pub(crate) fn parse_answer(content: &str, model: &str) -> Result<Answer> {
    let answer = strip_think_block(content).trim().to_string();
    if answer.is_empty() {
        return Err(Error::InvalidLlmOutput("answer is empty".into()));
    }

    Ok(Answer {
        answer,
        model: model.to_string(),
        prompt_version: Answer::PROMPT_VERSION.into(),
    })
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
}

pub(crate) fn parse_chat_content(body: &str) -> Result<String> {
    let response: ChatResponse = serde_json::from_str(body)
        .map_err(|e| Error::InvalidLlmOutput(format!("invalid chat completion response: {e}")))?;

    let choice = response.choices.into_iter().next();
    let finish_reason = choice
        .as_ref()
        .and_then(|choice| choice.finish_reason.clone());

    let content = choice
        .and_then(|choice| choice.message.content)
        .map(|content| strip_think_block(&content).to_string())
        .filter(|content| !content.is_empty());

    match content {
        Some(content) => Ok(content),
        None => {
            tracing::warn!(
                finish_reason = ?finish_reason,
                body = %truncate(body, 500),
                "LLM returned empty content"
            );

            let detail = if finish_reason.as_deref() == Some("length") {
                "（输出被 token 上限截断，请调大 max tokens 或降低 thinking）"
            } else {
                ""
            };

            Err(Error::InvalidLlmOutput(format!(
                "LLM returned an empty response{detail}"
            )))
        }
    }
}

pub(crate) fn strip_think_block(content: &str) -> &str {
    if let Some(position) = content.rfind("</think")
        && let Some(end) = content[position..].find('>')
    {
        return content[position + end + 1..].trim();
    }

    content.trim()
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: Option<String>,
}

pub(crate) fn parse_models_response(
    body: &str,
) -> std::result::Result<Vec<String>, LlmEndpointError> {
    let response: ModelsResponse = serde_json::from_str(body).map_err(|e| {
        LlmEndpointError::invalid_response(format!("invalid model list response: {e}"))
    })?;

    let mut models: Vec<String> = response
        .data
        .into_iter()
        .filter_map(|entry| entry.id)
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    models.sort_by_key(|id| id.to_ascii_lowercase());
    models.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(models)
}

pub(crate) fn parse_summary_text(content: &str, model: &str) -> Result<Summary> {
    let text = strip_think_block(content).trim();
    if text.is_empty() {
        return Err(Error::InvalidLlmOutput("summary text is empty".into()));
    }

    let mut short = String::new();
    let mut detailed = String::new();
    let mut key_points = Vec::new();
    let mut section = 0_u8;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(rest) = strip_label(trimmed, &["一句话概括", "一句话"]) {
            section = 1;
            if !rest.is_empty() {
                short = rest.to_string();
            }
            continue;
        }
        if let Some(rest) = strip_label(trimmed, &["详细分析", "分析"]) {
            section = 2;
            if !rest.is_empty() {
                detailed.push_str(rest);
                detailed.push('\n');
            }
            continue;
        }
        if let Some(rest) = strip_label(trimmed, &["关键要点", "要点"]) {
            section = 3;
            let point = rest.trim_start_matches(['-', '•', '*']).trim();
            if !point.is_empty() {
                key_points.push(point.to_string());
            }
            continue;
        }

        match section {
            1 if !trimmed.is_empty() => {
                if !short.is_empty() {
                    short.push(' ');
                }
                short.push_str(trimmed);
            }
            2 => {
                detailed.push_str(line);
                detailed.push('\n');
            }
            3 => {
                let point = trimmed.trim_start_matches(['-', '•', '*']).trim();
                if !point.is_empty() {
                    key_points.push(point.to_string());
                }
            }
            _ => {}
        }
    }

    let detailed = detailed.trim().to_string();

    if short.trim().is_empty() || detailed.is_empty() {
        tracing::warn!("summary response did not follow the expected sections");
    }

    let short = if short.trim().is_empty() {
        first_line(text)
    } else {
        short.trim().to_string()
    };

    let detailed = if detailed.is_empty() {
        text.to_string()
    } else {
        detailed
    };

    if short.is_empty() {
        return Err(Error::InvalidLlmOutput("summary is empty".into()));
    }

    Ok(Summary {
        short,
        detailed,
        key_points,
        model: model.to_string(),
        prompt_version: Summary::PROMPT_VERSION.into(),
    })
}

fn strip_label<'a>(line: &'a str, labels: &[&str]) -> Option<&'a str> {
    labels.iter().find_map(|label| {
        let rest = line.strip_prefix(label)?.trim_start();

        // Only treat the label as a section header when it stands alone or is
        // followed by a colon. Content lines may also start with these words.
        if !rest.is_empty() && !rest.starts_with([':', '：']) {
            return None;
        }

        Some(rest.trim_start_matches([':', '：']).trim())
    })
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect()
}

#[derive(Debug, Deserialize)]
struct KeywordsPayload {
    keywords: String,
}

pub(crate) fn parse_keywords(content: &str) -> Result<String> {
    let payload: KeywordsPayload = serde_json::from_str(content).map_err(|e| {
        Error::InvalidLlmOutput(format!("keywords JSON does not match the schema: {e}"))
    })?;

    let keywords = payload.keywords.trim().to_string();
    if keywords.is_empty() {
        return Err(Error::InvalidLlmOutput("keywords is empty".into()));
    }
    if contains_cjk(&keywords) {
        return Err(Error::InvalidLlmOutput(
            "keywords still contain Chinese characters".into(),
        ));
    }

    Ok(keywords)
}
