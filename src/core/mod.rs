//! 核心数据类型和错误定义

mod error;
mod paper;
mod audio;

pub use error::{Error, Result};
pub use paper::{Paper, Summary, UserPreferences};
pub use audio::AudioData;
