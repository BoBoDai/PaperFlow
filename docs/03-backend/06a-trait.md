# LLM Provider — Trait 设计

`src/modules/llm/trait_.rs` 定义了可插拔的 LLM Provider 接口。

## 设计目标

支持多种 LLM 后端（OpenAI、MiniMax 等），通过配置切换，不修改业务代码。

## LlmConfig

```rust
pub struct LlmConfig {
    pub provider_type: String,  // "openai" | "minimax"
    pub api_key: String,
    pub model: String,          // "MiniMax-M2.7" | "gpt-4o-mini"
    pub base_url: Option<String>,
}
```

由 `handlers::create_llm()` 从运行时 `ApiConfig` 构造。

## LlmProvider Trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 生成论文摘要（结构化 JSON）
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;

    /// 将摘要转换为口语化表达（用于语音播报）
    async fn verbalize(&self, summary: &Summary) -> Result<String>;

    /// 评估论文与用户兴趣的相关性 (0.0 - 10.0)
    async fn score_relevance(&self, paper: &Paper, interests: &[String]) -> Result<f64>;

    /// Provider 名称
    fn name(&self) -> &str;

    /// 通用文本补全（用于翻译等自由文本任务）
    async fn complete(&self, prompt: &str) -> Result<String>;
}
```

## 为什么用 async-trait

Rust 原生不支持 async trait 方法。`#[async_trait]` 宏将 async fn 转换为返回 `Pin<Box<dyn Future>>` 的普通方法，允许 trait object 的动态分发。

## Provider 选择

```rust
fn create_llm(config: &ApiConfig) -> Box<dyn LlmProvider> {
    let llm_config = LlmConfig {
        provider_type: config.llm_provider.clone(),
        api_key: config.api_key.clone().unwrap_or_default(),
        model: config.llm_model.clone(),
        base_url: Some(config.llm_base_url.clone()),
    };

    if config.llm_provider == "minimax" {
        Box::new(MiniMaxProvider::new(llm_config))
    } else {
        Box::new(OpenAiProvider::new(llm_config))
    }
}
```

返回 `Box<dyn LlmProvider>` 实现运行时多态。调用方不关心具体是哪个 Provider。

## 使用场景

| 方法 | 使用场景 |
|------|---------|
| `summarize` | 论文详情页「智能分析」|
| `complete` | 中文查询翻译、论文标题/摘要中译 |
| `score_relevance` | 论文列表排序（预留）|
| `verbalize` | 文本转口语（预留语音播报）|
