//! 领域错误类型

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("网络请求失败: {0}")]
    Network(String),

    #[error("响应解析失败: {0}")]
    Parse(String),

    #[error("LLM 调用失败: {0}")]
    Llm(String),

    /// LLM 返回了合法 JSON，但形状不对。
    ///
    /// **这个变体存在的意义就是不许静默降级** —— 绝不允许「解析失败就
    /// 把原文当结果返回」这种兜底，那会让用户看到垃圾而日志一片安静。
    #[error("LLM 输出结构不合法: {0}")]
    InvalidLlmOutput(String),

    #[error("存储错误: {0}")]
    Storage(String),

    #[error("配置缺失或无效: {0}")]
    Config(String),
}
