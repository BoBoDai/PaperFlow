//! Group-chat and final-opinion usecases for the three-brain panel.

use paperflow_adapters::{LlmProvider, PaperSearch, ReviewProvider};
use paperflow_core::{
    Error, FinalizeRequest, OpinionBatch, OpinionOutcome, PersonaAnswer, PersonaChatRequest,
    PersonaId, Result,
};

use super::budget::{char_count, trim_transcript, truncate_context};
use super::review::aggregate_opinions;
use super::{App, MAGI_QUESTION_BUDGET, MAGI_TRANSCRIPT_BUDGET, QA_CONTEXT_BUDGET};

impl<A: PaperSearch, L: LlmProvider + ReviewProvider, O: PaperSearch> App<A, L, O> {
    /// One persona's streamed reply in the shared group chat.
    pub async fn persona_chat_stream<F>(
        &self,
        mut request: PersonaChatRequest,
        mut on_delta: F,
    ) -> Result<PersonaAnswer>
    where
        F: FnMut(String) + Send,
    {
        let question = validate_question(&request.question)?;
        let context = prepare_context(&request.context)?;
        request.question = question;
        request.context = context;
        request.transcript = trim_transcript(request.transcript, MAGI_TRANSCRIPT_BUDGET);

        self.llm.persona_chat_stream(&request, &mut on_delta).await
    }

    /// Collect three independent final opinions and aggregate them.
    ///
    /// Each persona receives the shared transcript and its own review, never
    /// another persona's final opinion.
    pub async fn finalize_opinions(&self, mut request: FinalizeRequest) -> Result<OpinionBatch> {
        let question = validate_question(&request.question)?;
        let context = prepare_context(&request.context)?;
        request.question = question;
        request.context = context;
        request.transcript = trim_transcript(request.transcript, MAGI_TRANSCRIPT_BUDGET);

        let rigor = request.opinion_request(PersonaId::Rigor);
        let repro = request.opinion_request(PersonaId::Repro);
        let insight = request.opinion_request(PersonaId::Insight);

        let (rigor, repro, insight) = tokio::join!(
            self.llm.persona_opinion(&rigor),
            self.llm.persona_opinion(&repro),
            self.llm.persona_opinion(&insight),
        );

        let outcomes: Vec<OpinionOutcome> = PersonaId::ALL
            .into_iter()
            .zip([rigor, repro, insight])
            .map(|(persona, result)| match result {
                Ok(opinion) => OpinionOutcome::Ok { opinion },
                Err(error) => OpinionOutcome::Failed {
                    persona,
                    message: error.to_string(),
                },
            })
            .collect();

        let verdict = aggregate_opinions(&outcomes);

        Ok(OpinionBatch { outcomes, verdict })
    }
}

fn validate_question(question: &str) -> Result<String> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err(Error::InvalidInput("问题不能为空".into()));
    }
    if char_count(&question) > MAGI_QUESTION_BUDGET {
        return Err(Error::InvalidInput(format!(
            "问题过长（最多 {MAGI_QUESTION_BUDGET} 字）"
        )));
    }
    Ok(question)
}

fn prepare_context(context: &str) -> Result<String> {
    let context = context.trim();
    if context.is_empty() {
        return Err(Error::InvalidInput(
            "没有可用的论文上下文（摘要为空，且尚未加载全文）".into(),
        ));
    }
    let (context, _) = truncate_context(context, QA_CONTEXT_BUDGET, true);
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_questions() {
        assert!(matches!(
            validate_question("   "),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_questions_over_budget() {
        let question = "问".repeat(MAGI_QUESTION_BUDGET + 1);
        assert!(matches!(
            validate_question(&question),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_empty_context() {
        assert!(matches!(prepare_context("  "), Err(Error::InvalidInput(_))));
    }
}
