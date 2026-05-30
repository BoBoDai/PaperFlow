# 配置管理

`src/core/config.rs` 管理应用配置的加载、合并和持久化。

## 配置来源优先级

```
环境变量 > api-key 文件 > config.toml
```

## AppConfig

可持久化的配置结构体，使用 TOML 序列化：

```rust
pub struct AppConfig {
    pub api_key: Option<String>,
    pub llm_provider: Option<String>,  // "openai" | "minimax"
    pub llm_model: Option<String>,     // "MiniMax-M2.7" | "gpt-4o-mini"
    pub llm_base_url: Option<String>,  // "https://api.minimaxi.com/v1"
    pub max_papers: Option<usize>,
    pub voice_speed: Option<f64>,
}
```

## 加载逻辑

```rust
pub fn load(config_path: &PathBuf) -> Self {
    // 1. 从 config.toml 加载基础配置
    let mut config = match std::fs::read_to_string(config_path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Self::default(),
    };

    // 2. 从项目目录 api-key 文件覆盖
    if let Ok(cwd) = std::env::current_dir() {
        let api_key_file = cwd.join("api-key");
        if let Ok(content) = std::fs::read_to_string(&api_key_file) {
            let api_config = Self::parse_api_key_file(&content);
            // 逐字段覆盖非 None 的值
            if api_config.api_key.is_some() { config.api_key = api_config.api_key; }
            if api_config.llm_base_url.is_some() { config.llm_base_url = api_config.llm_base_url; }
            // ...
        }
    }

    // 3. 环境变量最高优先级（在 Default 中处理）
    config
}
```

## api-key 文件解析

使用 `.env` 风格的 `KEY=VALUE` 格式，支持：

- `OPENAI_API_KEY` / `MINIMAX_API_KEY` / `API_KEY` → api_key
- `OPENAI_BASE_URL` / `BASE_URL` → llm_base_url
- `LLM_MODEL` / `MODEL` → llm_model
- `LLM_PROVIDER` / `PROVIDER` → llm_provider
- 注释行（`#` 开头）自动跳过
- `export` 前缀自动剥离
- `KEY=` 空值跳过
- `$` 未展开变量跳过
- 纯文本行（无 `=`）作为 api_key（向后兼容）

## 保存逻辑

```rust
pub fn save(&self, config_path: &PathBuf) -> anyhow::Result<()> {
    // 确保父目录存在
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // TOML 序列化并写入
    let content = toml::to_string_pretty(self)?;
    std::fs::write(config_path, content)?;
    Ok(())
}
```

`POST /api/config` 更新配置后自动调用 `save()` 持久化到磁盘。

## 系统路径

| 系统 | 配置路径 |
|------|---------|
| macOS | `~/Library/Application Support/com.paperflow.PaperFlow/config.toml` |
| Linux | `~/.config/paperflow/config.toml` |
