use super::*;
use paperflow_core::{Error, QaRole, QaScope, QaTurn};

fn config(provider: ProviderProfileId) -> LlmConfig {
    let defaults = provider.defaults();
    LlmConfig {
        api_key: "sk-test".into(),
        base_url: defaults.base_url.into(),
        model: defaults.default_model.into(),
        provider,
        compat: LlmCompat::default(),
    }
}

#[test]
fn provider_capabilities_match_profiles() {
    let deepseek = ProviderProfileId::DeepSeek.capabilities();
    assert!(deepseek.json_mode);
    assert_eq!(deepseek.max_tokens_field, MaxTokensField::MaxTokens);

    let openai = ProviderProfileId::OpenAi.capabilities();
    assert_eq!(openai.max_tokens_field, MaxTokensField::MaxCompletionTokens);

    let minimax = ProviderProfileId::MiniMax.capabilities();
    assert!(!minimax.json_mode);
    assert_eq!(minimax.thinking_mode, ThinkingMode::Default);
}

#[test]
fn provider_profile_id_parses_leniently() {
    assert_eq!(
        ProviderProfileId::from_id("deepseek"),
        ProviderProfileId::DeepSeek
    );
    assert_eq!(
        ProviderProfileId::from_id(" OPENAI "),
        ProviderProfileId::OpenAi
    );
    assert_eq!(ProviderProfileId::from_id(""), ProviderProfileId::OpenAi);
    assert_eq!(
        ProviderProfileId::from_id("unknown"),
        ProviderProfileId::Custom
    );
}

#[test]
fn build_body_uses_profile_capabilities() {
    let deepseek = build_chat_body(
        &config(ProviderProfileId::DeepSeek),
        vec![],
        10,
        true,
        false,
    );
    assert_eq!(deepseek["response_format"]["type"], "json_object");
    assert_eq!(deepseek["max_tokens"].as_u64(), Some(10));
    assert!(deepseek.get("max_completion_tokens").is_none());

    let openai = build_chat_body(&config(ProviderProfileId::OpenAi), vec![], 10, true, false);
    assert_eq!(openai["max_completion_tokens"].as_u64(), Some(10));
    assert!(openai.get("max_tokens").is_none());

    let minimax = build_chat_body(&config(ProviderProfileId::MiniMax), vec![], 10, true, false);
    assert!(minimax.get("response_format").is_none());
    assert!(minimax.get("thinking").is_none());
    assert_eq!(minimax["max_completion_tokens"].as_u64(), Some(10));
    assert!(minimax.get("max_tokens").is_none());

    let mut minimax_disabled = config(ProviderProfileId::MiniMax);
    minimax_disabled.compat.thinking_mode = Some(ThinkingMode::Disabled);
    let body = build_chat_body(&minimax_disabled, vec![], 10, true, false);
    assert_eq!(body["thinking"]["type"], "disabled");
}

#[test]
fn compat_overrides_take_precedence() {
    let mut config = config(ProviderProfileId::Custom);
    config.compat = LlmCompat {
        json_mode: Some(false),
        max_tokens_field: Some(MaxTokensField::MaxCompletionTokens),
        thinking_mode: Some(ThinkingMode::Disabled),
        thinking_format: Some(ThinkingFormat::EnableThinking),
    };

    let body = build_chat_body(&config, vec![], 10, true, false);
    assert!(body.get("response_format").is_none());
    assert_eq!(body["max_completion_tokens"].as_u64(), Some(10));
    assert_eq!(body["enable_thinking"].as_bool(), Some(false));
}

#[test]
fn adaptive_thinking_is_sent_when_requested() {
    let mut config = config(ProviderProfileId::MiniMax);
    config.compat = LlmCompat {
        thinking_mode: Some(ThinkingMode::Adaptive),
        ..LlmCompat::default()
    };

    let body = build_chat_body(&config, vec![], 10, true, false);
    assert_eq!(body["thinking"]["type"], "adaptive");
}

#[test]
fn normalizes_base_urls() {
    assert_eq!(
        normalize_base_url(" https://api.deepseek.com/ ").unwrap(),
        "https://api.deepseek.com"
    );
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1").unwrap(),
        "http://localhost:11434/v1"
    );
    assert!(normalize_base_url("api.deepseek.com").is_err());
    assert!(normalize_base_url("https://api.openai.com/v1/chat/completions").is_err());
    assert!(normalize_base_url("https://user:pass@example.com/v1").is_err());
}

#[test]
fn parses_and_sorts_model_list() {
    let body = r#"{"data":[{"id":"b-model"},{"id":"A-model"},{"id":"a-model"},{"id":""}]}"#;
    let models = parse_models_response(body).expect("models should parse");

    assert_eq!(models, vec!["A-model".to_string(), "b-model".to_string()]);
}

#[test]
fn maps_unknown_model_errors() {
    let error = map_http_error(
        reqwest::StatusCode::BAD_REQUEST,
        &reqwest::header::HeaderMap::new(),
        r#"{"error":{"message":"unknown model 'foo'","type":"bad_request_error"},"request_id":"req-1"}"#,
    );

    assert_eq!(error.kind, LlmErrorKind::UnknownModel);
    assert_eq!(error.request_id.as_deref(), Some("req-1"));
    assert!(error.message.contains("模型不可用"));
}

#[test]
fn maps_auth_and_rate_limit_errors() {
    let headers = reqwest::header::HeaderMap::new();

    let unauthorized = map_http_error(reqwest::StatusCode::UNAUTHORIZED, &headers, "{}");
    assert_eq!(unauthorized.kind, LlmErrorKind::Unauthorized);

    let limited = map_http_error(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, "{}");
    assert_eq!(limited.kind, LlmErrorKind::RateLimited);
}

#[test]
fn debug_redacts_api_key() {
    let debug = format!("{:?}", config(ProviderProfileId::DeepSeek));
    assert!(!debug.contains("sk-test"));
    assert!(debug.contains("***"));
}

#[test]
fn parses_plain_text_summary() {
    let content =
        "一句话：这是概括。\n详细分析：\n分析与方法。\n更多分析。\n要点：\n- 要点一\n- 要点二";
    let summary = parse_summary_text(content, "deepseek-chat").expect("summary should parse");

    assert_eq!(summary.short, "这是概括。");
    assert_eq!(summary.detailed, "分析与方法。\n更多分析。");
    assert_eq!(summary.key_points, ["要点一", "要点二"]);
    assert_eq!(summary.model, "deepseek-chat");
    assert_eq!(summary.prompt_version, Summary::PROMPT_VERSION);
}

#[test]
fn parses_summary_without_headings() {
    let summary =
        parse_summary_text("这是第一行。\n这是第二行。", "m").expect("summary should parse");

    assert_eq!(summary.short, "这是第一行。");
    assert_eq!(summary.detailed, "这是第一行。\n这是第二行。");
    assert!(summary.key_points.is_empty());
}

#[test]
fn rejects_empty_summary() {
    assert!(matches!(
        parse_summary_text("   ", "m"),
        Err(Error::InvalidLlmOutput(_))
    ));
}

#[test]
fn parses_valid_keywords() {
    let keywords = parse_keywords(r#"{"keywords":"diffusion models image generation"}"#)
        .expect("keywords should parse");
    assert_eq!(keywords, "diffusion models image generation");
}

#[test]
fn rejects_chinese_keywords() {
    assert!(matches!(
        parse_keywords(r#"{"keywords":"扩散模型"}"#),
        Err(Error::InvalidLlmOutput(_))
    ));
}

#[test]
fn rejects_empty_chat_content() {
    let body = r#"{"choices":[{"message":{"content":""}}]}"#;
    assert!(matches!(
        parse_chat_content(body),
        Err(Error::InvalidLlmOutput(_))
    ));
}

#[test]
fn reports_truncated_empty_content() {
    let body = r#"{"choices":[{"message":{"content":""},"finish_reason":"length"}]}"#;
    let error = parse_chat_content(body).expect_err("empty content must fail");

    assert!(matches!(
        error,
        Error::InvalidLlmOutput(message) if message.contains("截断")
    ));
}

#[test]
fn strips_minimax_thinking_blocks() {
    let content = "<think>reasoning\nhere</think>\n{\"short\":\"x\",\"detailed\":\"y\",\"key_points\":[\"z\"]}";

    assert_eq!(
        strip_think_block(content),
        r#"{"short":"x","detailed":"y","key_points":["z"]}"#
    );
}

fn qa_request() -> QaRequest {
    QaRequest {
        paper: Paper {
            id: "2401.1".into(),
            title: "Test Paper".into(),
            authors: vec!["Alice".into(), "Bob".into()],
            abstract_text: "An abstract.".into(),
            categories: vec!["cs.CL".into()],
            published: chrono::Utc::now(),
            pdf_url: "https://example.com/secret.pdf?token=abc".into(),
            source: paperflow_core::Source::Arxiv,
            venue: None,
        },
        question: "核心贡献是什么？".into(),
        scope: QaScope::CurrentPage,
        context: "[第 3 页]\nThe method ...".into(),
        page: Some(3),
        history: vec![
            QaTurn {
                role: QaRole::User,
                content: "上一轮问题".into(),
            },
            QaTurn {
                role: QaRole::Assistant,
                content: "上一轮回答".into(),
            },
        ],
    }
}

#[test]
fn builds_qa_messages_with_metadata_context_and_history() {
    let messages = build_qa_messages(&qa_request());
    let serialized = serde_json::to_string(&messages).expect("messages should serialize");

    assert!(serialized.contains("Test Paper"));
    assert!(serialized.contains("核心贡献"));
    assert!(serialized.contains("[第 3 页]"));
    assert!(serialized.contains("上一轮问题"));
    assert!(serialized.contains("上一轮回答"));
    assert_eq!(messages.len(), 4);
}

#[test]
fn qa_prompt_omits_pdf_url() {
    let serialized = serde_json::to_string(&build_qa_messages(&qa_request()))
        .expect("messages should serialize");

    assert!(!serialized.contains("secret.pdf"));
    assert!(!serialized.contains("token=abc"));
}

#[test]
fn parses_valid_answer() {
    let answer = parse_answer("这是解释", "model").expect("answer should parse");

    assert_eq!(answer.answer, "这是解释");
    assert_eq!(answer.model, "model");
    assert_eq!(answer.prompt_version, Answer::PROMPT_VERSION);
}

#[test]
fn rejects_empty_answer() {
    assert!(matches!(
        parse_answer("   ", "model"),
        Err(Error::InvalidLlmOutput(_))
    ));
}

#[test]
fn finds_sse_events_with_lf_and_crlf() {
    assert_eq!(next_sse_event("data: a\n\n"), Some((7, 2)));
    assert_eq!(next_sse_event("data: a\r\n\r\n"), Some((7, 4)));
    assert_eq!(next_sse_event("data: a"), None);
}

#[test]
fn streams_plain_text_immediately() {
    let mut stream = AnswerStream::default();

    assert_eq!(stream.push("Hello "), "Hello ");
    assert_eq!(stream.push("world"), "world");
    assert_eq!(stream.finish(), "Hello world");
}

#[test]
fn holds_back_partial_think_tags() {
    let mut stream = AnswerStream::default();

    assert_eq!(stream.push("<thi"), "");
    assert_eq!(stream.push("nk>reasoning"), "");
    assert_eq!(stream.push(" more</think>answer"), "answer");
    assert_eq!(stream.finish(), "answer");
}

#[test]
fn rejects_unclosed_think_block() {
    let mut stream = AnswerStream::default();

    assert_eq!(stream.push("<think>reasoning"), "");
    assert_eq!(stream.finish(), "");
}
