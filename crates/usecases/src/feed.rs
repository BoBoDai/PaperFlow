//! Interest feed usecases.

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Error, FeedPage, Paper, Result};

use super::{App, FEED_MAX_LIMIT, MAX_FEED_INTERESTS, dedupe};

impl<A: PaperSearch, L: LlmProvider, O: PaperSearch> App<A, L, O> {
    /// 兴趣信息流：按最新发表时间返回一页论文。
    ///
    /// arXiv 是主源；首页为空或失败时回退 OpenAlex。`has_more` 在去重前
    /// 根据原始页大小计算，避免去重后误判「到底了」。
    pub async fn browse_feed(
        &self,
        interests: &[String],
        offset: usize,
        limit: usize,
    ) -> Result<FeedPage> {
        let interests = normalize_interests(interests);
        if interests.is_empty() {
            return Ok(FeedPage {
                papers: Vec::new(),
                has_more: false,
            });
        }

        let interests = self.translate_interests(&interests).await?;
        let limit = limit.clamp(1, FEED_MAX_LIMIT);
        let offset = (offset / limit) * limit;

        let page = match self.arxiv.feed(&interests, offset, limit).await {
            Ok(papers) if !papers.is_empty() => feed_page(papers, limit),
            // An empty page past the first one means the feed is exhausted;
            // do not restart another source at an arbitrary offset.
            Ok(_) if offset > 0 => FeedPage {
                papers: Vec::new(),
                has_more: false,
            },
            Ok(_) => match self.openalex.feed(&interests, offset, limit).await {
                Ok(papers) => feed_page(papers, limit),
                Err(openalex_error) => {
                    return Err(Error::Sources(format!(
                        "arXiv returned no results; OpenAlex: {openalex_error}"
                    )));
                }
            },
            Err(arxiv_error) => {
                tracing::warn!(
                    error = %arxiv_error,
                    offset,
                    "arXiv feed failed; falling back to OpenAlex"
                );

                match self.openalex.feed(&interests, offset, limit).await {
                    Ok(papers) => feed_page(papers, limit),
                    Err(openalex_error) => {
                        return Err(Error::Sources(format!(
                            "arXiv: {arxiv_error}; OpenAlex: {openalex_error}"
                        )));
                    }
                }
            }
        };

        tracing::debug!(
            papers = page.papers.len(),
            has_more = page.has_more,
            "feed page ready"
        );

        Ok(page)
    }

    async fn translate_interests(&self, interests: &[String]) -> Result<Vec<String>> {
        let mut translated = Vec::with_capacity(interests.len());

        for interest in interests {
            translated.push(self.effective_query(interest).await?);
        }

        Ok(translated)
    }
}

pub(crate) fn normalize_interests(interests: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();

    interests
        .iter()
        .map(|interest| interest.trim())
        .filter(|interest| !interest.is_empty())
        .filter(|interest| seen.insert(interest.to_lowercase()))
        .take(MAX_FEED_INTERESTS)
        .map(|interest| interest.to_string())
        .collect()
}

fn feed_page(raw: Vec<Paper>, limit: usize) -> FeedPage {
    // Compute before dedupe so duplicate titles do not make a full page look
    // like the end of the feed.
    let has_more = raw.len() >= limit;
    FeedPage {
        papers: dedupe(raw),
        has_more,
    }
}
