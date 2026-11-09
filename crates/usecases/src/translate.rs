//! Text translation usecase.
//!
//! The caller supplies a selection-sized excerpt; anything larger is truncated
//! head-only and the response records that fact instead of failing silently.

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Error, Result, TranslateRequest, Translation};

use super::budget::truncate_context;
use super::{App, TRANSLATE_TEXT_BUDGET};

impl<A: PaperSearch, L: LlmProvider, O: PaperSearch> App<A, L, O> {
    /// Translate text into Simplified Chinese.
    pub async fn translate_text(&self, request: TranslateRequest) -> Result<Translation> {
        let (text, truncated) = prepare_translation(&request.text)?;
        let mut translation = self.llm.translate_text(&text).await?;
        translation.truncated = truncated;
        Ok(translation)
    }

    /// Streaming translation: deltas flow through `on_delta`.
    pub async fn translate_text_stream<F>(
        &self,
        request: TranslateRequest,
        mut on_delta: F,
    ) -> Result<Translation>
    where
        F: FnMut(String) + Send,
    {
        let (text, truncated) = prepare_translation(&request.text)?;
        let mut translation = self.llm.translate_text_stream(&text, &mut on_delta).await?;
        translation.truncated = truncated;
        Ok(translation)
    }
}

fn prepare_translation(raw: &str) -> Result<(String, bool)> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(Error::InvalidInput("翻译内容不能为空".into()));
    }

    Ok(truncate_context(text, TRANSLATE_TEXT_BUDGET, false))
}
