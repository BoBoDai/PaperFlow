# PaperFlow 📄

全天候语音学术助理 — 终端里的论文检索和分析工具。

- 用自然语言搜索 arXiv 和 Semantic Scholar 最新论文
- 中文输入自动翻译为英文关键词
- 论文标题和摘要自动翻译成中文
- LLM 智能分析论文核心贡献和关键点
- 后台定时监控新论文并保存
- 收藏和稍后读管理

详细技术文档见 [docs/](docs/SUMMARY.md)。

## 安装

```bash
# 编译后端
cargo build --release

# 安装前端依赖
cd ui && npm install
```

## 配置

在项目目录创建 `api-key` 文件：

```
OPENAI_API_KEY=你的APIKey
OPENAI_BASE_URL=https://api.minimaxi.com/v1
LLM_MODEL=MiniMax-M2.7
LLM_PROVIDER=openai
```

支持任何 OpenAI 兼容接口（MiniMax、DeepSeek、OpenAI 等）。

## 启动

```bash
# 终端 1：后端
cargo run --release --bin server

# 终端 2：前端 TUI
cd ui && npm start
```

## 使用

### 快捷查询

启动后直接按一个键：

| 键 | 领域 |
|----|------|
| `r` | 机器人（cs.RO + cs.AI + cs.CV + cs.LG）|
| `a` | AI / ML |
| `c` | 计算机视觉 |

### 自定义搜索

按 `Enter` 进入搜索模式，输入关键词后回车。

支持的命令：

| 输入 | 效果 |
|------|------|
| `robot grasping` | 英文直接搜索 |
| `机器人抓取` | 中文自动翻译后搜索 |
| `/robotics` | 快捷命令，同 `r` |
| `/ai` / `/cv` | 快捷命令 |

### 论文列表

搜索结果展示标题、作者、日期、摘要预览。按数字键 `1`-`9` 进入详情。

### 论文详情

| 键 | 功能 |
|----|------|
| `f` | 收藏论文 |
| `s` | 语音播报 |
| `o` | 浏览器打开 PDF |
| `q` | 返回列表 |

中英双语展示标题和摘要，LLM 智能分析核心贡献和关键发现。

### 导航

| 键 | 作用 |
|----|------|
| `q` | 返回上一步 / 退出 |
| `Ctrl+C` | 双击退出 |
| `/` | 配置菜单 |
| `Enter` | 确认 / 搜索 |

## 架构

```
PaperFlow
├── src/                     # Rust 后端
│   ├── api/                 # Axum HTTP API
│   │   ├── handlers.rs      # 接口处理
│   │   └── state.rs         # 共享状态
│   ├── core/                # 核心类型
│   │   ├── paper.rs         # Paper 模型
│   │   ├── config.rs        # 配置管理
│   │   └── error.rs         # 错误类型
│   ├── modules/
│   │   ├── arxiv/           # arXiv API 客户端
│   │   ├── semantic_scholar/# Semantic Scholar 客户端
│   │   ├── llm/             # LLM Provider（OpenAI/MiniMax）
│   │   ├── scheduler/       # 定时论文监控
│   │   ├── storage/         # SQLite 数据库
│   │   ├── filter/          # 论文筛选评分
│   │   └── speech/          # 语音识别/合成
│   └── bin/server.rs        # 入口
├── ui/                      # TypeScript 前端 (Ink/TUI)
│   └── src/
│       ├── App.tsx           # 主组件
│       ├── components/       # UI 组件
│       └── services/         # API 调用
└── api-key                  # API 配置（不提交）
```

## API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/search` | GET | 搜索论文 |
| `/api/quick-search` | GET | 预设快捷查询 |
| `/api/translate` | POST | 中文→英文翻译 |
| `/api/translate-paper` | POST | 论文中译 |
| `/api/summarize` | POST | LLM 智能摘要 |
| `/api/papers/save` | POST | 收藏论文 |
| `/api/saved` | GET | 查看已收藏 |
| `/api/config` | GET/POST | 配置管理 |
