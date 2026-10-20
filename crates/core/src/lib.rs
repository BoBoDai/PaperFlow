//! # paperflow-core
//!
//! 领域层。**纯逻辑，零 IO。**
//!
//! ## 这一层不许出现的东西
//!
//! - 任何 HTTP 客户端（reqwest、hyper）
//! - 任何数据库（sqlx、rusqlite）
//! - 任何 GUI / IPC 类型（tauri、axum）
//!
//! 违反了这条，`core` 就没法纯单测，架构的地基就塌了。
//! 需要外部世界时，把 trait 定义在这里或 `adapters`，由调用方注入。

pub mod error;
pub mod paper;
pub mod summary;

pub use error::{Error, Result};
pub use paper::{Paper, Source};
pub use summary::Summary;

/// True when the text contains Han characters.
///
/// Used to decide whether a search query needs LLM translation before it is
/// sent to the English-language paper sources.
pub fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_han_characters() {
        assert!(contains_cjk("扩散模型"));
        assert!(contains_cjk("diffusion 模型"));
        assert!(!contains_cjk("diffusion models"));
        assert!(!contains_cjk(""));
    }
}
