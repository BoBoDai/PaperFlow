# 定时调度器

`src/modules/scheduler/job.rs` — 后台定期拉取最新论文。

## 设计

```rust
pub struct Scheduler {
    interval_minutes: u64,     // 抓取间隔（默认 60 分钟）
    categories: Vec<String>,   // 监控的 arXiv 分类
    keywords: Vec<String>,     // Semantic Scholar 搜索关键词
}
```

## 启动方式

```rust
// server.rs
let scheduler = Scheduler::new(60);
tokio::spawn(async move {
    scheduler.start(db).await;
});
```

使用 `tokio::spawn` 在后台运行，不阻塞 HTTP 服务。

## 主循环

```rust
pub async fn start(self, db: Database) {
    let duration = Duration::from_secs(self.interval_minutes * 60);

    // 立即执行一次
    self.fetch_and_save(&db).await;

    let mut ticker = interval(duration);
    loop {
        ticker.tick().await;
        self.fetch_and_save(&db).await;
    }
}
```

## fetch_and_save 核心逻辑

```rust
async fn fetch_and_save(&self, db: &Database) {
    let mut new_count = 0;

    // 1. 从 arXiv 各分类拉取
    for cat in &self.categories {
        match arxiv.search_by_category(cat, 3).await {
            Ok(papers) => {
                for paper in papers {
                    // 检查是否已存在
                    if db.get_paper(&paper.id).await?.is_none() {
                        db.save_paper(&paper).await?;
                        new_count += 1;
                    }
                }
            }
            Err(e) => warn!("arXiv {} 查询失败: {}", cat, e),
        }
        // 分类间延迟 2 秒防限速
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // 2. 从 Semantic Scholar 搜索
    for kw in &self.keywords {
        match ss.search(kw, 3, None).await {
            Ok(papers) => {
                for paper in papers {
                    if db.get_paper(&paper.id).await?.is_none() {
                        db.save_paper(&paper).await?;
                        new_count += 1;
                    }
                }
            }
            Err(e) => warn!("Semantic Scholar 查询失败: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // 3. 日志报告
    if new_count > 0 {
        info!("定时任务: 发现 {} 篇新论文并已保存", new_count);
    } else {
        info!("定时任务: 没有发现新论文");
    }
}
```

## 设计要点

- **幂等** — `get_paper()` 检查后再 `save_paper()`，不会重复保存
- **容错** — 单个分类失败不阻断其他分类
- **日志** — 每次运行都记录新论文数量，便于排查
- **可配置** — 通过 `UserPreferences.fetch_interval_minutes` 调整频率
