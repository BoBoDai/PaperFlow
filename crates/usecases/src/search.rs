//! Keyword search usecases.

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Error, Result, SearchResponse, contains_cjk};

use super::{App, dedupe};

impl<A: PaperSearch, L: LlmProvider, O: PaperSearch> App<A, L, O> {
    /// 检索论文，并做跨源去重。
    ///
    /// 中文查询在 LLM 已配置时转成英文关键词；未配置时直接按原文检索。
    /// LLM 已配置但调用失败仍然报错，不静默吞掉失败。
    /// arXiv 是主源；它失败或没有结果时回退到 OpenAlex。两个源都失败才
    /// 返回 [`Error::Sources`]，绝不假装成功返回空列表。
    ///
    /// 返回值包含实际发送给数据源的英文查询，前端可以展示给用户。
    pub async fn search_papers(&self, query: &str, limit: usize) -> Result<SearchResponse> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(SearchResponse {
                papers: Vec::new(),
                query: String::new(),
            });
        }

        let search_query = self.effective_query(query).await?;

        let papers = match self.arxiv.search(&search_query, limit).await {
            Ok(papers) if !papers.is_empty() => dedupe(papers),
            Ok(_) => {
                tracing::info!(
                    query = %search_query,
                    "arXiv returned no results; falling back to OpenAlex"
                );

                match self.openalex.search(&search_query, limit).await {
                    Ok(papers) => dedupe(papers),
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            query = %search_query,
                            "OpenAlex fallback failed after an empty arXiv result"
                        );
                        Vec::new()
                    }
                }
            }
            Err(arxiv_error) => {
                tracing::warn!(
                    error = %arxiv_error,
                    query = %search_query,
                    "arXiv failed; falling back to OpenAlex"
                );

                match self.openalex.search(&search_query, limit).await {
                    Ok(papers) => dedupe(papers),
                    Err(openalex_error) => {
                        return Err(Error::Sources(format!(
                            "arXiv: {arxiv_error}; OpenAlex: {openalex_error}"
                        )));
                    }
                }
            }
        };

        Ok(SearchResponse {
            papers,
            query: search_query,
        })
    }

    /// Translate a Chinese query with the configured LLM. Without an LLM,
    /// fall back to the original text so search still works.
    pub(crate) async fn effective_query(&self, query: &str) -> Result<String> {
        if !contains_cjk(query) {
            return Ok(query.to_string());
        }

        if !self.llm.is_configured() {
            tracing::warn!(
                query,
                "LLM is not configured; searching the original Chinese query"
            );
            return Ok(query.to_string());
        }

        let translated = self.llm.to_english_keywords(query).await?;
        tracing::info!(original = query, translated = %translated, "translated query with LLM");
        Ok(translated)
    }
}
