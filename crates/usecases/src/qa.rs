//! Paper Q&A usecases.

use paperflow_adapters::{LlmProvider, PaperSearch};
use paperflow_core::{Error, QaRequest, QaResponse, QaRole, QaScope, QaTurn, Result};

use super::{
    App, QA_CONTEXT_BUDGET, QA_HISTORY_BUDGET, QA_HISTORY_TURNS, QA_QUESTION_BUDGET,
    QA_SELECTION_BUDGET,
};

impl<A: PaperSearch, L: LlmProvider, O: PaperSearch> App<A, L, O> {
    /// 基于 PDF 上下文回答论文问题。
    pub async fn ask_paper(&self, request: QaRequest) -> Result<QaResponse> {
        let (request, context_truncated, context_chars) = prepare_qa(request)?;
        let answer = self.llm.ask_paper(&request).await?;

        Ok(QaResponse {
            answer: answer.answer,
            model: answer.model,
            prompt_version: answer.prompt_version,
            scope: request.scope,
            page: request.page,
            context_truncated,
            context_chars,
        })
    }

    /// 流式版本：每次收到增量时调用 `on_delta`，最后返回完整响应。
    pub async fn ask_paper_stream<F>(
        &self,
        request: QaRequest,
        mut on_delta: F,
    ) -> Result<QaResponse>
    where
        F: FnMut(String) + Send,
    {
        let (request, context_truncated, context_chars) = prepare_qa(request)?;
        let answer = self.llm.ask_paper_stream(&request, &mut on_delta).await?;

        Ok(QaResponse {
            answer: answer.answer,
            model: answer.model,
            prompt_version: answer.prompt_version,
            scope: request.scope,
            page: request.page,
            context_truncated,
            context_chars,
        })
    }
}

fn char_count(text: &str) -> usize {
    text.chars().count()
}

fn prepare_qa(mut request: QaRequest) -> Result<(QaRequest, bool, usize)> {
    let question = request.question.trim().to_string();
    if question.is_empty() {
        return Err(Error::InvalidInput("问题不能为空".into()));
    }
    if char_count(&question) > QA_QUESTION_BUDGET {
        return Err(Error::InvalidInput(format!(
            "问题过长（最多 {QA_QUESTION_BUDGET} 字）"
        )));
    }
    request.question = question;

    let context = request.context.trim().to_string();
    if context.is_empty() {
        return Err(Error::InvalidInput(
            "没有可用的 PDF 文本。该 PDF 可能是扫描版，或文本尚未提取完成。".into(),
        ));
    }

    let budget = match request.scope {
        QaScope::Selection => QA_SELECTION_BUDGET,
        QaScope::CurrentPage | QaScope::FullPaper => QA_CONTEXT_BUDGET,
    };

    let (context, truncated) =
        if request.scope == QaScope::FullPaper && char_count(&context) > budget {
            let head: String = context.chars().take(budget * 3 / 4).collect();
            let tail: String = context
                .chars()
                .rev()
                .take(budget / 4)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            (
                format!("{head}\n\n[…上下文过长，中间内容已省略…]\n\n{tail}"),
                true,
            )
        } else if char_count(&context) > budget {
            (context.chars().take(budget).collect(), true)
        } else {
            (context, false)
        };

    let context_chars = char_count(&context);
    request.context = context;
    request.history = sanitize_history(request.history);

    Ok((request, truncated, context_chars))
}

fn sanitize_history(history: Vec<QaTurn>) -> Vec<QaTurn> {
    let mut turns: Vec<QaTurn> = history
        .into_iter()
        .filter_map(|turn| {
            let content = turn.content.trim().to_string();
            if content.is_empty() {
                return None;
            }

            let budget = match turn.role {
                QaRole::User => 2_000,
                QaRole::Assistant => 4_000,
            };

            Some(QaTurn {
                role: turn.role,
                content: content.chars().take(budget).collect(),
            })
        })
        .collect();

    if turns.len() > QA_HISTORY_TURNS {
        turns = turns.split_off(turns.len() - QA_HISTORY_TURNS);
    }

    let mut total: usize = turns.iter().map(|turn| char_count(&turn.content)).sum();
    while total > QA_HISTORY_BUDGET && turns.len() > 1 {
        let removed = turns.remove(0);
        total -= char_count(&removed.content);
    }

    turns
}
