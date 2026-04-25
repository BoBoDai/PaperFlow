//! TUI 应用
//!
//! 简化版 TUI，后续可以扩展

use crate::core::{Paper, Result};

/// TUI 应用状态
pub struct TuiApp {
    papers: Vec<Paper>,
    selected_index: usize,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            papers: Vec::new(),
            selected_index: 0,
        }
    }

    /// 设置论文列表
    pub fn set_papers(&mut self, papers: Vec<Paper>) {
        self.papers = papers;
        self.selected_index = 0;
    }

    /// 向上选择
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// 向下选择
    pub fn select_next(&mut self) {
        if self.selected_index < self.papers.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    /// 获取选中的论文
    pub fn selected_paper(&self) -> Option<&Paper> {
        self.papers.get(self.selected_index)
    }

    /// 运行 TUI
    pub async fn run(&mut self) -> Result<()> {
        println!("PaperFlow TUI 模式");
        println!("==================\n");

        self.display_list();

        // 简化实现：直接打印列表，不使用复杂的 TUI
        // 后续可以扩展完整的事件处理

        Ok(())
    }

    /// 显示论文列表
    pub fn display_list(&self) {
        println!("论文列表:\n");

        for (i, paper) in self.papers.iter().enumerate() {
            let prefix = if i == self.selected_index { "> " } else { "  " };
            let score = paper.relevance_score
                .map(|s| format!("[{:.1}]", s))
                .unwrap_or_else(|| "[N/A]".to_string());
            println!("{}{} {} - {}", prefix, score, paper.title, paper.id);
        }

        println!("\n使用 ↑↓ 选择论文，Enter 播报，q 退出");
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}
