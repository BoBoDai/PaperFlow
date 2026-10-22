//! Three-brain paper review usecase.
//!
//! The three persona calls run concurrently and independently. A failed
//! persona is returned explicitly instead of being dropped; the aggregate
//! verdict is then computed by a pure function.

use paperflow_adapters::{LlmProvider, PaperSearch, ReviewProvider};
use paperflow_core::{
    OpinionOutcome, PersonaId, PersonaOutcome, Result, ReviewContext, ReviewRequest, ReviewResult,
    ReviewVerdict, Stance,
};

use super::budget::truncate_context;
use super::{App, MAGI_ABSTRACT_CONFIDENCE_CAP, QA_CONTEXT_BUDGET};

impl<A: PaperSearch, L: LlmProvider + ReviewProvider, O: PaperSearch> App<A, L, O> {
    /// Review a paper through the three independent personas.
    pub async fn review_paper(&self, request: ReviewRequest) -> Result<ReviewResult> {
        let context = request
            .context
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());

        let (context, review_context) = match context {
            Some(text) => {
                let (text, _) = truncate_context(text, QA_CONTEXT_BUDGET, true);
                (Some(text), ReviewContext::FullText)
            }
            None => (None, ReviewContext::Abstract),
        };

        let (rigor, repro, insight) = tokio::join!(
            self.llm
                .review_persona(&request.paper, PersonaId::Rigor, context.as_deref()),
            self.llm
                .review_persona(&request.paper, PersonaId::Repro, context.as_deref()),
            self.llm
                .review_persona(&request.paper, PersonaId::Insight, context.as_deref()),
        );

        let mut model = String::new();
        let mut outcomes = Vec::with_capacity(3);

        for (persona, result) in PersonaId::ALL.into_iter().zip([rigor, repro, insight]) {
            match result {
                Ok(mut review) => {
                    if review_context == ReviewContext::Abstract {
                        review.confidence = review.confidence.min(MAGI_ABSTRACT_CONFIDENCE_CAP);
                    }
                    if model.is_empty() {
                        model = review.model.clone();
                    }
                    outcomes.push(PersonaOutcome::Ok { review });
                }
                Err(error) => outcomes.push(PersonaOutcome::Failed {
                    persona,
                    message: error.to_string(),
                }),
            }
        }

        let verdict = aggregate_review(&outcomes);

        Ok(ReviewResult {
            paper_id: request.paper.id,
            outcomes,
            verdict,
            context: review_context,
            model,
            prompt_version: ReviewResult::PROMPT_VERSION.into(),
        })
    }
}

/// Aggregate three review slots into a verdict.
///
/// Only decisive stances can form a consensus or a majority. Three
/// `Borderline` stances are reported as a split, not as agreement.
pub(crate) fn aggregate_review(outcomes: &[PersonaOutcome]) -> ReviewVerdict {
    if outcomes.len() != PersonaId::ALL.len() {
        return ReviewVerdict::Incomplete;
    }

    let mut stances = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        match outcome {
            PersonaOutcome::Ok { review } => stances.push(review.stance),
            PersonaOutcome::Failed { .. } => return ReviewVerdict::Incomplete,
        }
    }

    aggregate_stances(&stances)
}

/// Aggregate three final-opinion slots using the same policy as reviews.
pub(crate) fn aggregate_opinions(outcomes: &[OpinionOutcome]) -> ReviewVerdict {
    if outcomes.len() != PersonaId::ALL.len() {
        return ReviewVerdict::Incomplete;
    }

    let mut stances = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        match outcome {
            OpinionOutcome::Ok { opinion } => stances.push(opinion.stance),
            OpinionOutcome::Failed { .. } => return ReviewVerdict::Incomplete,
        }
    }

    aggregate_stances(&stances)
}

fn aggregate_stances(stances: &[Stance]) -> ReviewVerdict {
    let support = stances
        .iter()
        .filter(|stance| **stance == Stance::Support)
        .count();
    let oppose = stances
        .iter()
        .filter(|stance| **stance == Stance::Oppose)
        .count();

    if support == 3 {
        ReviewVerdict::Consensus {
            stance: Stance::Support,
        }
    } else if oppose == 3 {
        ReviewVerdict::Consensus {
            stance: Stance::Oppose,
        }
    } else if support == 2 {
        ReviewVerdict::Majority {
            stance: Stance::Support,
        }
    } else if oppose == 2 {
        ReviewVerdict::Majority {
            stance: Stance::Oppose,
        }
    } else {
        ReviewVerdict::Split
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paperflow_core::{PersonaOpinion, PersonaReview};

    fn review(persona: PersonaId, stance: Stance) -> PersonaReview {
        PersonaReview {
            persona,
            stance,
            confidence: 80,
            summary: "summary".into(),
            strengths: vec![],
            concerns: vec!["concern".into()],
            riskiest_claim: "claim".into(),
            would_change_mind: "evidence".into(),
            model: "test".into(),
        }
    }

    fn ok(persona: PersonaId, stance: Stance) -> PersonaOutcome {
        PersonaOutcome::Ok {
            review: review(persona, stance),
        }
    }

    #[test]
    fn three_support_is_consensus() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Support),
            ok(PersonaId::Repro, Stance::Support),
            ok(PersonaId::Insight, Stance::Support),
        ];

        assert_eq!(
            aggregate_review(&outcomes),
            ReviewVerdict::Consensus {
                stance: Stance::Support
            }
        );
    }

    #[test]
    fn two_oppose_is_majority() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Oppose),
            ok(PersonaId::Repro, Stance::Oppose),
            ok(PersonaId::Insight, Stance::Borderline),
        ];

        assert_eq!(
            aggregate_review(&outcomes),
            ReviewVerdict::Majority {
                stance: Stance::Oppose
            }
        );
    }

    #[test]
    fn all_different_is_split() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Support),
            ok(PersonaId::Repro, Stance::Oppose),
            ok(PersonaId::Insight, Stance::Borderline),
        ];

        assert_eq!(aggregate_review(&outcomes), ReviewVerdict::Split);
    }

    #[test]
    fn all_borderline_is_split() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Borderline),
            ok(PersonaId::Repro, Stance::Borderline),
            ok(PersonaId::Insight, Stance::Borderline),
        ];

        assert_eq!(aggregate_review(&outcomes), ReviewVerdict::Split);
    }

    #[test]
    fn one_borderline_does_not_form_a_majority() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Support),
            ok(PersonaId::Repro, Stance::Borderline),
            ok(PersonaId::Insight, Stance::Borderline),
        ];

        assert_eq!(aggregate_review(&outcomes), ReviewVerdict::Split);
    }

    #[test]
    fn failed_slot_is_incomplete() {
        let outcomes = [
            ok(PersonaId::Rigor, Stance::Support),
            PersonaOutcome::Failed {
                persona: PersonaId::Repro,
                message: "timeout".into(),
            },
            ok(PersonaId::Insight, Stance::Support),
        ];

        assert_eq!(aggregate_review(&outcomes), ReviewVerdict::Incomplete);
    }

    #[test]
    fn opinions_use_the_same_policy() {
        let outcomes = [
            OpinionOutcome::Ok {
                opinion: PersonaOpinion {
                    persona: PersonaId::Rigor,
                    stance: Stance::Oppose,
                    confidence: 70,
                    reason: "weak".into(),
                },
            },
            OpinionOutcome::Ok {
                opinion: PersonaOpinion {
                    persona: PersonaId::Repro,
                    stance: Stance::Support,
                    confidence: 60,
                    reason: "ok".into(),
                },
            },
            OpinionOutcome::Failed {
                persona: PersonaId::Repro,
                message: "timeout".into(),
            },
        ];

        assert_eq!(aggregate_opinions(&outcomes), ReviewVerdict::Incomplete);
    }
}
