//! 定时任务调度器
//!
//! 简化实现，使用 tokio 的 interval

use std::time::Duration;
use tokio::time::interval;
use tracing::info;

/// 定时任务调度器
pub struct Scheduler;

impl Scheduler {
    /// 创建调度器
    pub fn new() -> Self {
        Self
    }

    /// 运行每日任务
    pub async fn run_daily<F>(hour: u32, minute: u32, name: &str, mut f: F)
    where
        F: FnMut() + Send + 'static,
    {
        loop {
            // 简化实现：每小时检查一次
            let mut ticker = interval(Duration::from_secs(3600));

            loop {
                ticker.tick().await;
                info!("执行定时任务: {}", name);
                f();
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
