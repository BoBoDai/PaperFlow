//! Groq Whisper 语音识别实现

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::core::{Error, Result};
use crate::modules::speech::recognizer::SpeechRecognizer;

/// Groq Whisper API 响应
#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
}

#[derive(Debug, Serialize)]
struct WhisperRequest {
    model: String,
}

/// Groq Whisper 语音识别
pub struct GroqWhisper {
    client: Client,
    api_key: String,
}

impl GroqWhisper {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl SpeechRecognizer for GroqWhisper {
    async fn transcribe(&self, audio_path: &Path) -> Result<String> {
        let file_bytes = tokio::fs::read(audio_path).await
            .map_err(|e| Error::Speech(format!("Failed to read audio file: {}", e)))?;

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(audio_path.file_name().unwrap().to_string_lossy().to_string());

        let form = reqwest::multipart::Form::new()
            .text("model", "whisper-large-v3")
            .part("file", part);

        let response = self.client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Speech(format!("Failed to send request: {}", e)))?;

        let result: WhisperResponse = response.json().await
            .map_err(|e| Error::Speech(format!("Failed to parse response: {}", e)))?;

        Ok(result.text)
    }

    fn name(&self) -> &str {
        "groq_whisper"
    }
}
