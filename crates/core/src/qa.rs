//! Paper reading assistant types.

use serde::{Deserialize, Serialize};

use crate::Paper;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QaScope {
    Selection,
    CurrentPage,
    FullPaper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QaRole {
    User,
    Assistant,
}

impl QaRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaTurn {
    pub role: QaRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaRequest {
    pub paper: Paper,
    pub question: String,
    pub scope: QaScope,
    pub context: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub history: Vec<QaTurn>,
}

/// Structurally validated LLM answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub answer: String,
    pub model: String,
    pub prompt_version: String,
}

impl Answer {
    pub const PROMPT_VERSION: &'static str = "v1";
}

/// Answer plus context metadata computed by the usecase layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QaResponse {
    pub answer: String,
    pub model: String,
    pub prompt_version: String,
    pub scope: QaScope,
    pub page: Option<usize>,
    pub context_truncated: bool,
    pub context_chars: usize,
}
