# 配置与部署

## api-key 文件

项目根目录下的 `api-key` 文件（.gitignore 已排除）：

```bash
OPENAI_API_KEY=你的密钥
OPENAI_BASE_URL=https://api.minimaxi.com/v1
LLM_MODEL=MiniMax-M2.7
LLM_PROVIDER=openai
```

### 支持的变量

| 变量 | 说明 | 示例 |
|------|------|------|
| `OPENAI_API_KEY` / `API_KEY` | LLM API 密钥 | `sk-cp-xxx` |
| `OPENAI_BASE_URL` / `BASE_URL` | LLM API 地址 | `https://api.minimaxi.com/v1` |
| `LLM_MODEL` / `MODEL` | 模型名 | `MiniMax-M2.7` |
| `LLM_PROVIDER` / `PROVIDER` | Provider 类型 | `openai` 或 `minimax` |

### 支持纯文本密钥（向后兼容）

第一行直接写密钥也是有效的（用于简单场景）。

## 系统配置文件

`~/Library/Application Support/com.paperflow.PaperFlow/config.toml`（macOS）

```toml
llm_provider = "openai"
llm_model = "MiniMax-M2.7"
llm_base_url = "https://api.minimaxi.com/v1"
max_papers = 5
voice_speed = 5.0
```

## 配置优先级

```
环境变量 OPENAI_API_KEY/BASE_URL
    ↓ 覆盖
项目目录 api-key 文件
    ↓ 覆盖
系统 config.toml
    ↓ 默认
代码内置默认值
```

## 部署

### 开发

```bash
# 终端 1：后端
cargo run --bin server

# 终端 2：前端
cd ui && npm start
```

### 生产

```bash
cargo build --release
cd ui && npm install && npm run build

# 后端二进制 + 前端 bundle
./target/release/server &
cd ui && node dist/index.js
```

## 环境要求

- Rust 1.80+
- Node.js 18+
- SQLite（由 sqlx 自动管理）

## 数据存储

| 存储 | 路径 |
|------|------|
| 数据库 | `~/Library/Application Support/com.paperflow.PaperFlow/paperflow.db` |
| 配置 | `~/Library/Application Support/com.paperflow.PaperFlow/config.toml` |
| API Key | `./api-key`（项目目录）|
