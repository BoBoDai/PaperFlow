//! Translation domain types.

use serde::{Deserialize, Serialize};

/// Request to translate a piece of text into Simplified Chinese.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
}

/// Structurally validated translation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Translation {
    pub text: String,
    pub model: String,
    pub prompt_version: String,
    #[serde(default)]
    pub truncated: bool,
}

impl Translation {
    pub const PROMPT_VERSION: &'static str = "v1";
}
