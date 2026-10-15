//! LLM 生成的论文摘要

use serde::{Deserialize, Serialize};

/// 一次 LLM 分析的产物。
///
/// 注意：这个类型**只能由校验过的 LLM 输出构造**。见
/// [`crate::Error::InvalidLlmOutput`]。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// 一句话概括
    pub short: String,
    /// 详细分析
    pub detailed: String,
    /// 关键要点
    pub key_points: Vec<String>,
    /// 生成它的模型 —— 落库时纳入缓存键，换模型自动失效
    pub model: String,
    /// prompt 版本 —— 同理，改 prompt 不该复用旧结果
    pub prompt_version: String,
}

impl Summary {
    /// 当前 prompt 版本。改 prompt 时手动 bump，旧缓存自动失效。
    pub const PROMPT_VERSION: &'static str = "v1";
}
