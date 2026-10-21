//! Provider profiles and compatibility capabilities.

use serde::{Deserialize, Serialize};

/// Provider family. The profile supplies defaults and request capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProviderProfileId {
    DeepSeek,
    #[default]
    OpenAi,
    MiniMax,
    Custom,
}

impl ProviderProfileId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAi => "openai",
            Self::MiniMax => "minimax",
            Self::Custom => "custom",
        }
    }

    pub fn from_id(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" => Self::OpenAi,
            "deepseek" => Self::DeepSeek,
            "openai" => Self::OpenAi,
            "minimax" => Self::MiniMax,
            _ => Self::Custom,
        }
    }

    pub const fn defaults(self) -> ProviderDefaults {
        match self {
            Self::DeepSeek => ProviderDefaults {
                label: "DeepSeek",
                base_url: "https://api.deepseek.com",
                default_model: "deepseek-chat",
            },
            Self::OpenAi => ProviderDefaults {
                label: "OpenAI",
                base_url: "https://api.openai.com/v1",
                default_model: "gpt-4o-mini",
            },
            Self::MiniMax => ProviderDefaults {
                label: "MiniMax",
                base_url: "https://api.minimaxi.com/v1",
                default_model: "MiniMax-M2",
            },
            Self::Custom => ProviderDefaults {
                label: "自定义服务商",
                base_url: "",
                default_model: "",
            },
        }
    }

    pub const fn capabilities(self) -> EffectiveCapabilities {
        match self {
            Self::DeepSeek => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::OpenAi => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxCompletionTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::MiniMax => EffectiveCapabilities {
                json_mode: false,
                // MiniMax documents max_completion_tokens as the preferred
                // field; max_tokens is deprecated.
                max_tokens_field: MaxTokensField::MaxCompletionTokens,
                // Omit thinking by default: M3.1 requires adaptive thinking
                // and rejects `disabled`, while M2.x works with the default.
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
            Self::Custom => EffectiveCapabilities {
                json_mode: true,
                max_tokens_field: MaxTokensField::MaxTokens,
                thinking_mode: ThinkingMode::Default,
                thinking_format: ThinkingFormat::Thinking,
            },
        }
    }
}

impl Serialize for ProviderProfileId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderProfileId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::from_id(&raw))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderDefaults {
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    MaxTokens,
    MaxCompletionTokens,
}

impl MaxTokensField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingFormat {
    Thinking,
    EnableThinking,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingMode {
    /// Omit the thinking parameter and use the provider's default.
    #[default]
    Default,
    /// Send `thinking: {"type": "disabled"}` (or `enable_thinking: false`).
    Disabled,
    /// Send `thinking: {"type": "adaptive"}` (required by some models).
    Adaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EffectiveCapabilities {
    pub json_mode: bool,
    pub max_tokens_field: MaxTokensField,
    pub thinking_mode: ThinkingMode,
    pub thinking_format: ThinkingFormat,
}

/// Per-provider compatibility overrides. `None` means inherit the profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LlmCompat {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<MaxTokensField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_mode: Option<ThinkingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<ThinkingFormat>,
}

impl EffectiveCapabilities {
    pub fn with_overrides(self, compat: &LlmCompat) -> Self {
        Self {
            json_mode: compat.json_mode.unwrap_or(self.json_mode),
            max_tokens_field: compat.max_tokens_field.unwrap_or(self.max_tokens_field),
            thinking_mode: compat.thinking_mode.unwrap_or(self.thinking_mode),
            thinking_format: compat.thinking_format.unwrap_or(self.thinking_format),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderProfileInfo {
    pub id: ProviderProfileId,
    pub label: &'static str,
    pub default_base_url: &'static str,
    pub default_model: &'static str,
    pub capabilities: EffectiveCapabilities,
    pub protocol: &'static str,
}

pub fn provider_catalog() -> Vec<ProviderProfileInfo> {
    [
        ProviderProfileId::DeepSeek,
        ProviderProfileId::OpenAi,
        ProviderProfileId::MiniMax,
        ProviderProfileId::Custom,
    ]
    .into_iter()
    .map(|id| {
        let defaults = id.defaults();
        ProviderProfileInfo {
            id,
            label: defaults.label,
            default_base_url: defaults.base_url,
            default_model: defaults.default_model,
            capabilities: id.capabilities(),
            protocol: "openai_chat_completions",
        }
    })
    .collect()
}
