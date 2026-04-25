# PaperFlow 开发讨论记录

## 日期：2026-04-25

---

## 1. 产品定位

**PaperFlow** 是一个"全天候语音学术助理"，核心功能：
- 从 arXiv 定时抓取论文
- 用 LLM 生成口语化摘要
- 语音播报，让用户在通勤时获取学术资讯

**目标用户**：博士生、研究员、算法工程师

---

## 2. 技术栈决策

### 2.1 核心语言
- **Rust** - 高性能、低内存占用、强类型安全

### 2.2 载体形态
用户选择了 **两者都实现**：
1. **Phase 1**: TUI 终端界面 - 快速验证核心逻辑
2. **Phase 2**: macOS 菜单栏应用 - 定时推送 + 语音播报

### 2.3 LLM Provider
用户选择 **MiniMax**（可扩展设计）

### 2.4 语音识别
用户选择 **Groq Whisper API**（速度快，实时转录）

### 2.5 存储方案
用户选择了 **SQLite**（轻量级嵌入数据库）

---

## 3. 架构设计

### 四层架构

| 层级 | 职责 | 技术栈 |
|------|------|--------|
| 调度层 | 定时任务、后台守护 | tokio, tokio-cron-scheduler |
| 认知层 | LLM 调用、摘要生成 | MiniMax, reqwest |
| 感知层 | 语音识别/合成 | Groq Whisper API, macOS say 命令 |
| 数据层 | SQLite 存储 | sqlx |

### 项目结构

```
src/
├── main.rs                 # 入口点
├── lib.rs                  # 库入口
├── config/                 # 配置管理 (CLI > env > config.toml)
├── core/                   # 核心类型定义 (error, paper, audio)
├── modules/
│   ├── arxiv/              # arXiv API 客户端
│   ├── filter/             # 智能筛选
│   ├── llm/                # LLM 抽象层 + MiniMax Provider
│   ├── speech/             # 语音识别/合成
│   ├── storage/            # SQLite 数据库
│   └── scheduler/          # 定时任务
├── ui/                     # UI 层 (TUI)
└── commands/               # CLI 命令
```

### 核心 Trait 设计

```rust
// LLM Provider trait（可插拔）
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn summarize(&self, paper: &Paper) -> Result<Summary>;
    async fn verbalize(&self, summary: &Summary) -> Result<String>;
    fn name(&self) -> &str;
}

// 语音识别/合成 trait
#[async_trait]
pub trait SpeechRecognizer: Send + Sync {
    async fn transcribe(&self, audio_path: &Path) -> Result<String>;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait SpeechSynthesizer: Send + Sync {
    async fn speak(&self, text: &str) -> Result<()>;
    fn name(&self) -> &str;
}
```

---

## 4. CLI 命令

```bash
paperflow fetch          # 抓取最新论文
paperflow list           # 列出已抓取的论文
paperflow speak [id]     # 语音播报指定论文
paperflow speak          # 播报所有未读论文
paperflow tui            # 启动 TUI 界面
```

### 示例

```bash
paperflow fetch --keywords "multimodal,CLIP"
paperflow list
paperflow speak 2401.12345
```

---

## 5. 依赖清单

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "multipart"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
clap = { version = "4", features = ["derive"] }
ratatui = "0.28"
crossterm = "0.28"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
directories = "6"
tokio-cron-scheduler = "0.14"
hound = "3.5"
async-trait = "0.1"
quick-xml = { version = "0.37", features = ["serialize"] }
toml = "0.8"
```

---

## 6. 数据流

```
scheduler (定时) ──► arxiv fetch ──► filter (score) ──► llm (summarize)
                                                              │
                                                              ▼
                                               ┌──────────────────────────┐
                                               │   speech.speak()          │
                                               │   (macOS say 命令)        │
                                               └──────────────────────────┘
```

---

## 7. 环境变量

```bash
MINIMAX_API_KEY=your_api_key
GROQ_API_KEY=your_api_key
```

---

## 8. 下一步计划

### Phase 1 (TUI MVP) ✅ 已完成
- [x] 项目结构搭建
- [x] core 模块 (error, paper)
- [x] storage 模块 (SQLite)
- [x] arxiv 模块
- [x] filter 模块
- [x] llm 模块 (MiniMax Provider)
- [x] speech 模块 (say 命令)
- [x] CLI 命令
- [x] TUI 界面（简化版）

### Phase 2 (完整 TUI)
- [ ] ratatui 完整 TUI 实现
- [ ] 键盘事件处理
- [ ] 实时语音识别交互

### Phase 3 (菜单栏应用)
- [ ] macOS 菜单栏应用
- [ ] 定时推送通知
- [ ] 语音唤醒功能

---

## 9. 备注

- Rust Edition 2024 已发布，本项目使用 2024 edition
- 可扩展架构：添加新 LLM Provider 只需实现 `LlmProvider` trait
