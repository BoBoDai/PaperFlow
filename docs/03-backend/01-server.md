# 入口与启动

`src/bin/server.rs` 是后端入口，负责初始化所有子系统并启动 HTTP 服务。

## 启动流程

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive("paperflow=info".parse().unwrap()))
        .init();

    // 2. 确定数据库路径
    let db_path = ProjectDirs::from("com", "paperflow", "PaperFlow")
        .map(|dirs| dirs.data_dir().join("paperflow.db"))
        .unwrap_or_else(|| "paperflow.db".into());

    // 3. 连接数据库
    let db = Database::new(&db_path).await?;

    // 4. 加载配置文件
    let config_path = ProjectDirs::from("com", "paperflow", "PaperFlow")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let app_config = AppConfig::load(&config_path);
    let api_config = ApiConfig::from_app_config(&app_config);

    // 5. 创建共享状态
    let state = ApiState { db: db.clone(), config: Arc::new(RwLock::new(api_config)), config_path };

    // 6. 启动后台调度器
    let scheduler = Scheduler::new(60); // 每 60 分钟
    tokio::spawn(async move { scheduler.start(db.clone()).await; });

    // 7. 启动 HTTP 服务器
    run_server(state).await
}
```

## 关键设计决策

### 为什么用 `Arc<RwLock<ApiConfig>>`？

配置需要在运行时通过 API 修改（`POST /api/config`），同时多个 handler 需要读取。`tokio::sync::RwLock` 提供了异步读写锁，允许多个读操作并发，写操作独占。

### 为什么 clone Database？

SQLx 的 `SqlitePool` 内部是 `Arc`，clone 只是增加引用计数。Scheduler 和 API handler 可以安全地共享同一个连接池。

### 配置文件优先级

1. 环境变量 `OPENAI_API_KEY` / `MINIMAX_API_KEY`
2. 项目目录 `api-key` 文件
3. 系统配置目录 `config.toml`
