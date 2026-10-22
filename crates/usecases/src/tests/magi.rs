use paperflow_core::{
    Error, FinalizeRequest, MagiTurn, OpinionOutcome, PersonaChatRequest, PersonaId, PersonaReview,
    ReviewVerdict, Source, Stance,
};

use super::support::{FakeLlm, FakeSearch, app, app_with_llm, paper};

fn source_paper() -> paperflow_core::Paper {
    paper("p1", "Paper", Source::Arxiv)
}

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
        model: "fake".into(),
    }
}

#[tokio::test]
async fn streams_a_persona_answer() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let mut deltas = Vec::new();
    let answer = app
        .persona_chat_stream(
            PersonaChatRequest {
                paper: source_paper(),
                persona: PersonaId::Rigor,
                question: "这篇论文的结论可靠吗？".into(),
                context: "abstract text".into(),
                transcript: vec![],
                own_review: None,
                panel_reviews: vec![],
            },
            |delta| deltas.push(delta),
        )
        .await
        .expect("chat should succeed");

    assert_eq!(answer.answer, "严谨 的回答");
    assert_eq!(deltas, vec!["严谨 的回答"]);
    assert_eq!(app.llm.chat_calls(), 1);
}

#[tokio::test]
async fn trims_the_transcript_to_the_budget() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let turn = |content: &str| MagiTurn {
        persona: None,
        content: content.into(),
    };
    let transcript = vec![turn(&"a".repeat(6_000)), turn(&"b".repeat(6_000)), turn(&"c".repeat(6_000))];

    app.persona_chat_stream(
        PersonaChatRequest {
            paper: source_paper(),
            persona: PersonaId::Repro,
            question: "继续".into(),
            context: "abstract text".into(),
            transcript,
            own_review: None,
            panel_reviews: vec![],
        },
        |_| {},
    )
    .await
    .expect("chat should succeed");

    let recorded = app
        .llm
        .last_chat_request()
        .expect("the fake should record the request");
    assert_eq!(recorded.transcript.len(), 2);
    assert_eq!(recorded.transcript[0].content, "b".repeat(6_000));
}

#[tokio::test]
async fn rejects_empty_context() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let result = app
        .persona_chat_stream(
            PersonaChatRequest {
                paper: source_paper(),
                persona: PersonaId::Rigor,
                question: "问题".into(),
                context: "   ".into(),
                transcript: vec![],
                own_review: None,
                panel_reviews: vec![],
            },
            |_| {},
        )
        .await;

    assert!(matches!(result, Err(Error::InvalidInput(_))));
}

#[tokio::test]
async fn finalizes_three_opinions() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let batch = app
        .finalize_opinions(FinalizeRequest {
            paper: source_paper(),
            question: "这篇论文可信吗？".into(),
            transcript: vec![MagiTurn {
                persona: None,
                content: "这篇论文可信吗？".into(),
            }],
            context: "abstract text".into(),
            reviews: vec![],
        })
        .await
        .expect("finalize should succeed");

    assert_eq!(batch.outcomes.len(), 3);
    assert_eq!(
        batch.verdict,
        ReviewVerdict::Consensus {
            stance: Stance::Support
        }
    );
    assert_eq!(app.llm.opinion_calls(), 3);
}

#[tokio::test]
async fn each_persona_only_sees_its_own_review() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::default(),
        FakeSearch::ok(vec![]),
    );

    let reviews = vec![
        review(PersonaId::Rigor, Stance::Oppose),
        review(PersonaId::Repro, Stance::Support),
        review(PersonaId::Insight, Stance::Borderline),
    ];

    app.finalize_opinions(FinalizeRequest {
        paper: source_paper(),
        question: "最终判断？".into(),
        transcript: vec![],
        context: "abstract text".into(),
        reviews: reviews.clone(),
    })
    .await
    .expect("finalize should succeed");

    let requests = app.llm.recorded_opinion_requests();
    assert_eq!(requests.len(), 3);

    for request in requests {
        let own = request
            .own_review
            .expect("each request should carry its own review");
        assert_eq!(own.persona, request.persona);
        assert_eq!(
            own,
            reviews
                .iter()
                .find(|review| review.persona == request.persona)
                .cloned()
                .expect("matching review")
        );
    }
}

#[tokio::test]
async fn a_failed_opinion_is_explicit_and_incomplete() {
    let app = app_with_llm(
        FakeSearch::ok(vec![]),
        FakeLlm::failing_opinion(PersonaId::Insight),
        FakeSearch::ok(vec![]),
    );

    let batch = app
        .finalize_opinions(FinalizeRequest {
            paper: source_paper(),
            question: "最终判断？".into(),
            transcript: vec![],
            context: "abstract text".into(),
            reviews: vec![],
        })
        .await
        .expect("finalize should return partial outcomes");

    assert_eq!(batch.verdict, ReviewVerdict::Incomplete);
    assert!(matches!(
        batch.outcomes[2],
        OpinionOutcome::Failed { .. }
    ));
    assert!(matches!(batch.outcomes[0], OpinionOutcome::Ok { .. }));
}

#[tokio::test]
async fn app_helper_still_works_with_the_default_fake() {
    // Keep the pre-existing helper exercised so the new trait bound does not
    // silently break the other tests' construction path.
    let _app = app(FakeSearch::ok(vec![]), FakeSearch::ok(vec![]));
}
