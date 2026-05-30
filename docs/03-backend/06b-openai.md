# OpenAI Provider

`src/modules/llm/providers/openai.rs` — 兼容 OpenAI Chat Completions API 格式。

## API 格式

```
POST {base_url}/v1/chat/completions
Authorization: Bearer {api_key}
Content-Type: application/json

{
  "model": "MiniMax-M2.7",
  "messages": [
    {"role": "user", "content": "Translate..."}
  ],
  "temperature": 0.3
}
```

响应：
```json
{
  "choices": [{
    "message": {"content": "robot grasping"}
  }]
}
```

## URL 拼接逻辑

```rust
let base = base_url.trim_end_matches('/');
let url = if base.ends_with("/v1") {
    format!("{}/chat/completions", base)   // 已有 /v1 不重复
} else {
    format!("{}/v1/chat/completions", base) // 补充 /v1
};
```

兼容两种配置：
- `https://api.minimaxi.com/v1` → 直接拼接
- `https://api.openai.com` → 补充 `/v1`

## 错误处理

```rust
let status = response.status();
let resp_text = response.text().await?;

if !status.is_success() {
    return Err(Error::Llm(format!(
        "OpenAI API error ({}): {}", status, &resp_text[..300]
    )));
}

let body: ChatResponse = serde_json::from_str(&resp_text)
    .map_err(|e| Error::Llm(format!("Parse error: {}", e)))?;
```

先读完整响应文本，再做 JSON 解析。这样解析失败时可以打印原始响应便于调试。

## 摘要 JSON 提取

LLM 返回的内容可能包含 markdown 代码块或额外文字。用 `find('{')` / `rfind('}')` 提取 JSON：

```rust
fn parse_summary_response(&self, content: &str, paper_id: &str) -> Result<Summary> {
    let json_str = if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            &content[start..=end]
        } else { content }
    } else { content };

    let json: JsonSummary = serde_json::from_str(json_str)?;
    // 映射到 Summary 结构体
}
```

## 思考标签清理

MiniMax M2 系列模型会在响应中包含 `&#x3c;think>` 推理过程。使用简单的字符串分割去除：

```rust
let cleaned = response
    .split("</think>")
    .last()
    .unwrap_or(&response)
    .trim()
    .to_string();
```

`</think>` 之后的内容才是最终答案。
