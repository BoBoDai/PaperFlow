//! 语音识别 trait

use async_trait::async_trait;
use std::path::Path;

use crate::core::{Error, Result};

/// 语音识别 trait
#[async_trait]
pub trait SpeechRecognizer: Send + Sync {
    /// 转录音频文件
    async fn transcribe(&self, audio_path: &Path) -> Result<String>;

    /// 获取 Provider 名称
    fn name(&self) -> &str;
}
