//! Shared search-source abstraction.

use paperflow_core::{Error, Paper, Result};

/// A keyword search source.
///
/// Both the arXiv and OpenAlex adapters implement this so the usecase layer can
/// try one and fall back to the other without depending on source-specific
/// details.
#[allow(async_fn_in_trait)]
pub trait PaperSearch {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>>;
}

/// Include reqwest's underlying cause; its `Display` alone hides transport
/// details such as proxy, DNS, or TLS failures.
pub(crate) fn request_error(context: &str, error: reqwest::Error) -> Error {
    let mut message = format!("{context}: {error}");
    let mut source = std::error::Error::source(&error);

    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }

    Error::Network(message)
}
