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
