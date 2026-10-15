//! arXiv 客户端
//!
//! ⚠️ **当前是占位实现**，返回固定数据，用来打通链路。
//! 真正的 arXiv 客户端（HTTP + Atom XML 解析）是下一步。

use chrono::{TimeZone, Utc};
use paperflow_core::{Paper, Result, Source};

/// 按关键词检索论文
#[allow(async_fn_in_trait)]
pub trait ArxivClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>>;
}

/// 占位实现：返回固定的样例数据。
///
/// 存在的意义是让「UI → IPC → usecase → 领域类型」这条链先跑通，
/// 等真实客户端接上，这一层直接换掉即可，上层一行不用动。
pub struct StubArxivClient;

impl ArxivClient for StubArxivClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Paper>> {
        tracing::info!(query, limit, "StubArxivClient: 返回占位数据");

        let papers = vec![
            Paper {
                id: "2401.00001".into(),
                title: "Attention Is All You Need".into(),
                authors: vec!["Ashish Vaswani".into(), "Noam Shazeer".into()],
                abstract_text: "The dominant sequence transduction models are based on complex \
                    recurrent or convolutional neural networks..."
                    .into(),
                categories: vec!["cs.CL".into(), "cs.LG".into()],
                published: Utc.with_ymd_and_hms(2017, 6, 12, 0, 0, 0).unwrap(),
                pdf_url: "https://arxiv.org/pdf/1706.03762".into(),
                source: Source::Arxiv,
                venue: Some("NeurIPS".into()),
            },
            Paper {
                id: "2401.00002".into(),
                title: "Deep Residual Learning for Image Recognition".into(),
                authors: vec!["Kaiming He".into(), "Xiangyu Zhang".into()],
                abstract_text: "Deeper neural networks are more difficult to train. We present a \
                    residual learning framework to ease the training of networks..."
                    .into(),
                categories: vec!["cs.CV".into()],
                published: Utc.with_ymd_and_hms(2015, 12, 10, 0, 0, 0).unwrap(),
                pdf_url: "https://arxiv.org/pdf/1512.03385".into(),
                source: Source::Arxiv,
                venue: Some("CVPR".into()),
            },
            Paper {
                id: "2401.00003".into(),
                title: "Language Models are Few-Shot Learners".into(),
                authors: vec!["Tom B. Brown".into()],
                abstract_text: "Recent work has demonstrated substantial gains on many NLP tasks \
                    and benchmarks by pre-training on a large corpus of text..."
                    .into(),
                categories: vec!["cs.CL".into()],
                published: Utc.with_ymd_and_hms(2020, 5, 28, 0, 0, 0).unwrap(),
                pdf_url: "https://arxiv.org/pdf/2005.14165".into(),
                source: Source::Arxiv,
                venue: Some("NeurIPS".into()),
            },
        ];

        Ok(papers.into_iter().take(limit).collect())
    }
}
