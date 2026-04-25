//! 系统 TTS 实现 (macOS say 命令)

use async_trait::async_trait;
use std::process::Command;

use crate::core::{Error, Result};
use crate::modules::speech::synthesizer::SpeechSynthesizer;

/// 系统 TTS（使用 macOS say 命令）
pub struct SystemSay {
    voice: Option<String>,
    speed: u32,
}

impl SystemSay {
    pub fn new() -> Self {
        Self {
            voice: None,
            speed: 200, // 默认语速
        }
    }

    /// 设置语音
    pub fn with_voice(mut self, voice: &str) -> Self {
        self.voice = Some(voice.to_string());
        self
    }

    /// 设置语速 (100-300)
    pub fn with_speed(mut self, speed: u32) -> Self {
        self.speed = speed;
        self
    }
}

impl Default for SystemSay {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SpeechSynthesizer for SystemSay {
    async fn speak(&self, text: &str) -> Result<()> {
        let mut cmd = Command::new("say");
        cmd.arg(text);

        if let Some(ref voice) = self.voice {
            cmd.arg("-v").arg(voice);
        }

        cmd.arg("-r").arg(self.speed.to_string());

        let output = cmd.output()
            .map_err(|e| Error::Speech(format!("Failed to execute say: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Speech(format!("say command failed: {:?}", output.stderr)))
        }
    }

    fn name(&self) -> &str {
        "system_say"
    }
}
