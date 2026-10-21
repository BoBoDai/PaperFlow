//! Persistent LLM settings and provider configuration.
//!
//! Provider settings live in `settings.json`; API keys live in a separate
//! write-only `credentials.json` (0600 on Unix). The UI receives a redacted
//! view and never gets a saved key back.

mod manager;
mod migration;
mod normalize;
mod types;

#[cfg(test)]
mod tests;

pub use manager::SettingsManager;
pub use types::{LlmProbeInput, LlmSettingsInput, LlmSettingsView};

#[cfg(test)]
pub use types::{LlmProviderInput, LlmSettings};

pub const SETTINGS_SCHEMA_VERSION: u32 = 2;
