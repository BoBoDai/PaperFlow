//! Shared search-source abstraction.

use std::error::Error as StdError;

use paperflow_core::{Error, Paper, Result};

/// A keyword search source.
///
/// Both the arXiv and OpenAlex adapters implement this so the usecase layer can
/// try one and fall back to the other without depending on source-specific
/// details.
#[allow(async_fn_in_trait)]
pub trait PaperSearch {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>>;

    /// Fetch the newest papers matching any of `interests`, starting at `offset`.
    async fn feed(&self, interests: &[String], offset: usize, limit: usize)
    -> Result<Vec<Paper>>;
}

/// Flatten an error's source chain into one message. `Display` alone hides
/// transport details such as proxy, DNS, or TLS failures.
pub(crate) fn describe_error(
    context: &str,
    error: &(dyn StdError + 'static),
) -> String {
    let mut message = format!("{context}: {error}");
    let mut source = error.source();

    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }

    message
}

pub(crate) fn request_error(context: &str, error: reqwest::Error) -> Error {
    Error::Network(describe_error(context, &error))
}
