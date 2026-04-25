//! 语音合成 trait

use async_trait::async_trait;
use crate::core::{Error, Result};

/// 语音合成 trait
#[async_trait]
pub trait SpeechSynthesizer: Send + Sync {
    /// 文本转语音并播报
    async fn speak(&self, text: &str) -> Result<()>;

    /// 获取 Provider 名称
    fn name(&self) -> &str;
}
