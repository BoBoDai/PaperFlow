//! speak 命令

use crate::modules::storage::Database;
use crate::modules::llm::{LlmProvider, MiniMaxProvider, LlmConfig};
use crate::modules::speech::synthesizer::SpeechSynthesizer;
use crate::modules::speech::providers::SystemSay;
use crate::core::Result;

/// 语音播报命令
pub struct SpeakCommand;

impl SpeakCommand {
    /// 执行播报
    pub async fn run(db: &Database, paper_id: &str, api_key: &str) -> Result<()> {
        // 获取论文
        let paper = match db.get_paper(paper_id).await? {
            Some(p) => p,
            None => {
                println!("未找到论文: {}", paper_id);
                return Ok(());
            }
        };

        println!("正在生成摘要: {}...", paper.title);

        // 创建 LLM Provider
        let llm_config = LlmConfig {
            provider_type: "minimax".to_string(),
            api_key: api_key.to_string(),
            model: "MiniMax-01".to_string(),
            base_url: Some("https://api.minimax.chat/v1".to_string()),
        };
        let llm = MiniMaxProvider::new(llm_config);

        // 生成摘要
        let summary = llm.summarize(&paper).await?;
        println!("摘要生成完成");

        // 口语化
        let spoken_text = llm.verbalize(&summary).await?;

        // 保存摘要
        db.save_summary(&summary).await?;

        // 语音播报
        println!("\n开始播报...\n");
        println!("{}", spoken_text);

        let tts = SystemSay::new();
        tts.speak(&spoken_text).await?;

        // 标记已读
        db.mark_paper_read(paper_id).await?;

        Ok(())
    }

    /// 播报所有未读论文
    pub async fn run_all(db: &Database, api_key: &str) -> Result<()> {
        let papers = db.list_unread_papers(10).await?;

        if papers.is_empty() {
            println!("没有未读论文");
            return Ok(());
        }

        println!("共有 {} 篇未读论文\n", papers.len());

        let llm_config = LlmConfig {
            provider_type: "minimax".to_string(),
            api_key: api_key.to_string(),
            model: "MiniMax-01".to_string(),
            base_url: Some("https://api.minimax.chat/v1".to_string()),
        };
        let llm = MiniMaxProvider::new(llm_config);
        let tts = SystemSay::new();

        for (i, paper) in papers.iter().enumerate() {
            println!("\n--- 论文 {}/{} ---", i + 1, papers.len());
            println!("{}", paper.title);

            match llm.summarize(&paper).await {
                Ok(summary) => {
                    let spoken_text = llm.verbalize(&summary).await?;
                    db.save_summary(&summary).await?;

                    println!("\n{}", spoken_text);
                    tts.speak(&spoken_text).await?;
                }
                Err(e) => {
                    println!("生成摘要失败: {}", e);
                }
            }
        }

        println!("\n\n播报完成！");

        Ok(())
    }
}
