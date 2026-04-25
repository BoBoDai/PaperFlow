//! 交互式对话模式

use crate::config::Settings;
use crate::modules::arxiv::ArxivClient;
use crate::modules::filter::Filter;
use crate::modules::llm::{LlmProvider, MiniMaxProvider, LlmConfig};
use crate::modules::speech::providers::SystemSay;
use crate::modules::speech::synthesizer::SpeechSynthesizer;
use crate::modules::storage::Database;
use crate::core::{UserPreferences, Result};
use std::io::{self, Write};

/// 交互式对话命令
pub struct ChatCommand;

impl ChatCommand {
    /// 运行交互式对话
    pub async fn run(settings: &Settings, db: &Database) -> Result<()> {
        let client = ArxivClient::new();
        let tts = SystemSay::new();

        println!("\n📚 PaperFlow - 语音学术助理\n");
        println!("{}", "=".repeat(50));
        println!("输入你想了解的论文主题，例如：");
        println!("  - multimodal learning");
        println!("  - CLIP, vision language");
        println!("  - 3D reconstruction");
        println!("  - diffusion model");
        println!("\n输入 quit 退出\n");
        println!("{}", "=".repeat(50));

        // 加载偏好设置
        let prefs = db.get_preferences().await?.unwrap_or_default();
        let api_key = std::env::var("MINIMAX_API_KEY").ok();

        loop {
            print!("\n🎯 请输入搜索主题: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input == "quit" || input == "q" || input == "退出" {
                println!("👋 再见！");
                break;
            }

            // 检测并翻译多语言查询
            let search_query = if Self::contains_chinese(input) {
                if let Some(key) = &api_key {
                    println!("\n🌐 检测到中文查询，正在翻译...");
                    match Self::translate_to_english(key, input).await {
                        Ok(english_query) => {
                            println!("🌐 翻译结果: {}", english_query);
                            english_query
                        }
                        Err(e) => {
                            println!("⚠️ 翻译失败: {}，将使用原文搜索", e);
                            input.to_string()
                        }
                    }
                } else {
                    println!("⚠️ 中文查询需要 API Key 才能翻译，将使用原文搜索");
                    input.to_string()
                }
            } else {
                input.to_string()
            };

            println!("\n🔍 正在搜索: {} ...", search_query);

            // 构建查询
            let query = format!("all:{}", search_query.replace(' ', " AND "));

            // 搜索论文
            let papers = match client.search(&query, 5).await {
                Ok(p) => p,
                Err(e) => {
                    println!("❌ 搜索失败: {}", e);
                    continue;
                }
            };

            if papers.is_empty() {
                println!("❌ 没有找到相关论文");
                continue;
            }

            // 筛选
            let search_prefs = UserPreferences {
                keywords: vec![input.to_string()],
                max_papers_per_fetch: 5,
                ..prefs.clone()
            };
            let scored_papers = Filter::filter_and_score(papers, &search_prefs);

            println!("\n📄 找到 {} 篇相关论文:\n", scored_papers.len());

            // 显示论文列表
            for (i, paper) in scored_papers.iter().enumerate() {
                let score = paper.relevance_score.unwrap_or(0.0);
                println!("{}. [评分: {:.1}] {}", i + 1, score, paper.title);
                println!("   👤 {}", paper.authors.join(", "));
                println!("   📅 {}\n", paper.published.format("%Y-%m-%d"));
            }

            // 如果有 API key，生成摘要
            if let Some(key) = &api_key {
                println!("\n📝 正在生成摘要...\n");

                let llm_config = LlmConfig {
                    provider_type: "minimax".to_string(),
                    api_key: key.clone(),
                    model: "MiniMax-01".to_string(),
                    base_url: Some("https://api.minimax.chat/v1".to_string()),
                };
                let llm = MiniMaxProvider::new(llm_config);

                for (i, paper) in scored_papers.iter().enumerate() {
                    print!("\n📖 论文 {}/{}: {} ...\n", i + 1, scored_papers.len(), paper.title);

                    match llm.summarize(paper).await {
                        Ok(summary) => {
                            println!("\n💡 摘要:");
                            println!("{}", summary.detailed_summary);

                            if !summary.key_points.is_empty() {
                                println!("\n🔑 关键点:");
                                for point in &summary.key_points {
                                    println!("  • {}", point);
                                }
                            }

                            // 保存摘要
                            let _ = db.save_summary(&summary).await;

                            // 询问是否语音播报
                            print!("\n🔊 是否语音播报这篇论文? (y/n): ");
                            io::stdout().flush().unwrap();

                            let mut answer = String::new();
                            io::stdin().read_line(&mut answer).unwrap();

                            if answer.trim().to_lowercase() == "y" || answer.trim().to_lowercase() == "yes" {
                                match llm.verbalize(&summary).await {
                                    Ok(text) => {
                                        println!("\n🔊 播报中...");
                                        if let Err(e) = tts.speak(&text).await {
                                            println!("⚠️ 语音播报失败: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ 生成语音文本失败: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️ 生成摘要失败: {}", e);
                        }
                    }
                }
            } else {
                println!("\n💡 设置 MINIMAX_API_KEY 环境变量可获得论文摘要");
            }

            println!("\n{}", "=".repeat(50));
        }

        Ok(())
    }

    /// 检测字符串是否包含中文
    fn contains_chinese(s: &str) -> bool {
        s.chars().any(|c| {
            let c = c as u32;
            // CJK Unified Ideographs Range: 4E00-9FFF
            // CJK Extension A: 3400-4DBF
            // CJK Compatibility: F900-FAFF
            // Full-width letters also count
            (0x4E00..=0x9FFF).contains(&c) ||
            (0x3400..=0x4DBF).contains(&c) ||
            (0xF900..=0xFAFF).contains(&c) ||
            // Full-width punctuation
            (0xFF00..=0xFFEF).contains(&c)
        })
    }

    /// 使用 MiniMax LLM 翻译中文到英文
    async fn translate_to_english(api_key: &str, chinese_text: &str) -> Result<String> {
        use crate::modules::llm::{LlmConfig, MiniMaxProvider};

        let llm_config = LlmConfig {
            provider_type: "minimax".to_string(),
            api_key: api_key.to_string(),
            model: "MiniMax-01".to_string(),
            base_url: Some("https://api.minimax.chat/v1".to_string()),
        };
        let llm = MiniMaxProvider::new(llm_config);

        let prompt = format!(
            "Translate the following Chinese text to English. Only return the English translation, no explanations.\n\nChinese: {}\nEnglish:",
            chinese_text
        );

        let response = llm.complete(&prompt).await?;
        Ok(response.trim().to_string())
    }
}
