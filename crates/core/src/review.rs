//! Three-brain paper review and group-chat types.
//!
//! The domain layer owns the persona identities, the structured review shape,
//! and the IPC-facing request/response types. Aggregation lives in
//! `paperflow-usecases` because it needs to be unit-tested against the same
//! policy the UI displays.

use serde::{Deserialize, Serialize};

use crate::Paper;

/// The three review personalities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaId {
    /// Methodology, statistics, and claim/evidence alignment.
    Rigor,
    /// Reproducibility, data/code availability, and engineering cost.
    Repro,
    /// Novelty, differentiation from prior work, and practical value.
    Insight,
}

impl PersonaId {
    pub const ALL: [Self; 3] = [Self::Rigor, Self::Repro, Self::Insight];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rigor => "rigor",
            Self::Repro => "repro",
            Self::Insight => "insight",
        }
    }

    /// Chinese label used by prompts and the UI.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rigor => "严谨",
            Self::Repro => "复现",
            Self::Insight => "洞见",
        }
    }

    /// Per-persona sampling temperature. Keeping these different is what
    /// stops the three calls from collapsing into one opinion.
    pub const fn temperature(self) -> f64 {
        match self {
            Self::Rigor => 0.2,
            Self::Repro => 0.4,
            Self::Insight => 0.7,
        }
    }
}

/// A reviewer's overall disposition toward the paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Support,
    Borderline,
    Oppose,
}

impl Stance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Support => "support",
            Self::Borderline => "borderline",
            Self::Oppose => "oppose",
        }
    }
}

/// What the review was based on. Abstract-only reviews have their confidence
/// capped and are labelled as such in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewContext {
    Abstract,
    FullText,
}

/// One persona's structured review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonaReview {
    pub persona: PersonaId,
    pub stance: Stance,
    /// 0-100, calibrated by the model.
    pub confidence: u8,
    pub summary: String,
    pub strengths: Vec<String>,
    pub concerns: Vec<String>,
    pub riskiest_claim: String,
    pub would_change_mind: String,
    pub model: String,
}

/// A review slot that either succeeded or failed explicitly.
///
/// A failed persona is never dropped: the UI renders the error and the
/// aggregate verdict becomes incomplete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PersonaOutcome {
    Ok { review: PersonaReview },
    Failed { persona: PersonaId, message: String },
}

/// Aggregate review verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReviewVerdict {
    Consensus { stance: Stance },
    Majority { stance: Stance },
    Split,
    Incomplete,
}

/// Full review returned to the transport layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewResult {
    pub paper_id: String,
    /// Always three entries, ordered like [`PersonaId::ALL`].
    pub outcomes: Vec<PersonaOutcome>,
    pub verdict: ReviewVerdict,
    pub context: ReviewContext,
    pub model: String,
    pub prompt_version: String,
}

impl ReviewResult {
    pub const PROMPT_VERSION: &'static str = "v1";
}

/// Input for an initial review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub paper: Paper,
    /// Extracted full text, when the caller has it.
    #[serde(default)]
    pub context: Option<String>,
}

/// One message in the shared MAGI transcript. `persona = None` is the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagiTurn {
    #[serde(default)]
    pub persona: Option<PersonaId>,
    pub content: String,
}

/// Input for one persona's reply in the group chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonaChatRequest {
    pub paper: Paper,
    pub persona: PersonaId,
    pub question: String,
    pub context: String,
    /// Shared transcript, including earlier rounds.
    #[serde(default)]
    pub transcript: Vec<MagiTurn>,
    /// The persona's own review, used as a stance anchor.
    #[serde(default)]
    pub own_review: Option<PersonaReview>,
    /// Initial reviews from the whole panel. The others are public context;
    /// they are not final opinions.
    #[serde(default)]
    pub panel_reviews: Vec<PersonaReview>,
}

/// A persona's streamed group-chat answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonaAnswer {
    pub answer: String,
    pub model: String,
    pub prompt_version: String,
}

impl PersonaAnswer {
    pub const PROMPT_VERSION: &'static str = "v1";
}

/// A persona's independent final opinion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonaOpinion {
    pub persona: PersonaId,
    pub stance: Stance,
    pub confidence: u8,
    pub reason: String,
}

/// Input for one persona's final opinion. `finalize_opinions` builds these
/// internally so the three opinions never see each other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpinionRequest {
    pub paper: Paper,
    pub persona: PersonaId,
    pub question: String,
    #[serde(default)]
    pub transcript: Vec<MagiTurn>,
    pub context: String,
    #[serde(default)]
    pub own_review: Option<PersonaReview>,
    /// Initial reviews from the whole panel, excluding this persona's own.
    #[serde(default)]
    pub panel_reviews: Vec<PersonaReview>,
}

/// A final-opinion slot that either succeeded or failed explicitly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OpinionOutcome {
    Ok { opinion: PersonaOpinion },
    Failed { persona: PersonaId, message: String },
}

/// Aggregate final verdict. Same policy as [`ReviewVerdict`].
pub type FinalVerdict = ReviewVerdict;

/// Result of one `finalize_opinions` call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpinionBatch {
    /// Always three entries, ordered like [`PersonaId::ALL`].
    pub outcomes: Vec<OpinionOutcome>,
    pub verdict: FinalVerdict,
}

/// Input for `finalize_opinions`: the shared discussion plus the initial
/// reviews, from which the backend picks each persona's own review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinalizeRequest {
    pub paper: Paper,
    pub question: String,
    #[serde(default)]
    pub transcript: Vec<MagiTurn>,
    pub context: String,
    #[serde(default)]
    pub reviews: Vec<PersonaReview>,
}

impl FinalizeRequest {
    /// Build the isolated per-persona request. The returned request contains
    /// only this persona's own review, never another persona's opinion.
    pub fn opinion_request(&self, persona: PersonaId) -> OpinionRequest {
        OpinionRequest {
            paper: self.paper.clone(),
            persona,
            question: self.question.clone(),
            transcript: self.transcript.clone(),
            context: self.context.clone(),
            own_review: self
                .reviews
                .iter()
                .find(|review| review.persona == persona)
                .cloned(),
            panel_reviews: self
                .reviews
                .iter()
                .filter(|review| review.persona != persona)
                .cloned()
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persona_serializes_as_snake_case() {
        let json = serde_json::to_string(&PersonaId::Repro).unwrap();
        assert_eq!(json, "\"repro\"");
    }

    #[test]
    fn failed_outcome_is_tagged_explicitly() {
        let outcome = PersonaOutcome::Failed {
            persona: PersonaId::Repro,
            message: "timeout".into(),
        };
        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(json["status"], "failed");
        assert_eq!(json["persona"], "repro");
        assert_eq!(json["message"], "timeout");
    }

    #[test]
    fn verdict_is_tagged_for_the_ui() {
        let verdict = ReviewVerdict::Majority {
            stance: Stance::Support,
        };
        let json = serde_json::to_value(verdict).unwrap();
        assert_eq!(json["kind"], "majority");
        assert_eq!(json["stance"], "support");

        let split = serde_json::to_value(ReviewVerdict::Split).unwrap();
        assert_eq!(split["kind"], "split");
    }

    #[test]
    fn finalize_request_isolates_persona_reviews() {
        let persona = PersonaId::Rigor;
        let review = PersonaReview {
            persona,
            stance: Stance::Oppose,
            confidence: 80,
            summary: "weak evaluation".into(),
            strengths: vec![],
            concerns: vec!["small sample".into()],
            riskiest_claim: "generalization".into(),
            would_change_mind: "a larger benchmark".into(),
            model: "test".into(),
        };
        let request = FinalizeRequest {
            paper: Paper {
                id: "p1".into(),
                title: "Title".into(),
                authors: vec![],
                abstract_text: String::new(),
                categories: vec![],
                published: chrono::Utc::now(),
                pdf_url: String::new(),
                source: crate::Source::Arxiv,
                venue: None,
            },
            question: "Is it sound?".into(),
            transcript: vec![],
            context: "abstract".into(),
            reviews: vec![review.clone()],
        };

        let rigor = request.opinion_request(PersonaId::Rigor);
        assert_eq!(rigor.own_review, Some(review));
        assert!(request.opinion_request(PersonaId::Insight).own_review.is_none());
    }
}
