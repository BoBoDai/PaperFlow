//! 语音 Provider 实现

pub mod groq_whisper;
pub mod system_say;

pub use groq_whisper::GroqWhisper;
pub use system_say::SystemSay;
