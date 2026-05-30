# 翻译接口

## POST /api/translate

中文查询 → 英文关键词。

```json
// Request
{"text": "机器人抓取的最新论文"}

// Response (成功)
{"translated": "robot grasping recent papers", "original": "...", "success": true}

// Response (无 API Key)
{"translated": "...", "original": "...", "success": false}
```

## POST /api/translate-paper

论文标题和摘要 → 中文。

```json
// Request
{"title": "Learning to Grasp...", "abstract_text": "We propose..."}

// Response
{"title_cn": "基于扩散模型学习抓取新物体", "abstract_cn": "我们提出了...", "success": true}
```

## 容错 JSON 解析

LLM 返回的 JSON 不一定完美（可能带 markdown 代码块、多余文字）。使用 `extract_json_field()` 做容错提取：

```rust
fn extract_json_field(text: &str, field: &str) -> String {
    // 1. 找 "{field}": 的位置
    // 2. 取冒号后的值
    // 3. 尝试提取引号内字符串
    // 4. 回退：取逗号/大括号前的文本
    // 5. 最终回退：text 第一行（用于 title）
}
```

## 思考标签清理

MiniMax M2 返回内容可能包含 `...`。清理逻辑：

```rust
let cleaned = response
    .split("</think>")
    .last()
    .unwrap_or(&response)
    .trim()
    .to_string();
```
