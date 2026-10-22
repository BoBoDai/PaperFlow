use paperflow_core::{
    PersonaId, PersonaOutcome, ReviewContext, ReviewRequest, ReviewVerdict, Source, Stance,
};

use super::support::{FakeLlm, FakeSearch, app_with_llm, paper};

fn source_paper() -> paperflow_core::Paper {
    paper("p1", "Paper", Source::Arxiv)
}

#[tokio::test]
async fn reviews_all_three_personas() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let result = app
        .review_paper(ReviewRequest {
            paper: source_paper(),
            context: None,
        })
        .await
        .expect("review should succeed");

    assert_eq!(result.paper_id, "p1");
    assert_eq!(result.outcomes.len(), 3);
    assert_eq!(result.context, ReviewContext::Abstract);
    assert_eq!(
        result.verdict,
        ReviewVerdict::Consensus {
            stance: Stance::Support
        }
    );
    assert_eq!(app.llm.review_calls(), 3);
}

#[tokio::test]
async fn abstract_reviews_cap_confidence() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let result = app
        .review_paper(ReviewRequest {
            paper: source_paper(),
            context: None,
        })
        .await
        .expect("review should succeed");

    for outcome in result.outcomes {
        let PersonaOutcome::Ok { review } = outcome else {
            panic!("expected a successful review");
        };
        assert_eq!(review.confidence, 70);
    }
}

#[tokio::test]
async fn full_text_reviews_keep_confidence() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let result = app
        .review_paper(ReviewRequest {
            paper: source_paper(),
            context: Some("a complete paper text".into()),
        })
        .await
        .expect("review should succeed");

    assert_eq!(result.context, ReviewContext::FullText);
    for outcome in result.outcomes {
        let PersonaOutcome::Ok { review } = outcome else {
            panic!("expected a successful review");
        };
        assert_eq!(review.confidence, 80);
    }
}

#[tokio::test]
async fn a_failed_persona_is_explicit_and_incomplete() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::failing_review(PersonaId::Repro),
        FakeSearch::ok(vec![]),
    );

    let result = app
        .review_paper(ReviewRequest {
            paper: source_paper(),
            context: None,
        })
        .await
        .expect("review should return partial outcomes");

    assert_eq!(result.verdict, ReviewVerdict::Incomplete);
    assert!(matches!(
        result.outcomes[1],
        PersonaOutcome::Failed { .. }
    ));
    assert!(matches!(result.outcomes[0], PersonaOutcome::Ok { .. }));
    assert!(matches!(result.outcomes[2], PersonaOutcome::Ok { .. }));
}
