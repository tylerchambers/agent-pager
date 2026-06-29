use std::collections::BTreeMap;

use agent_pager::{
    AgentPagerError,
    adapters::telegram::telegram_parse_mode,
    app::PagerConfigLoader,
    domain::{
        BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER, BotToken, CHAT_ID_ENV, CHAT_ID_PLACEHOLDER, ChatId,
        DEFAULT_HOST_ENV, DisplayPath, Document, DocumentFileName, HostName, MessageBody,
        MessageFormat, OutboundPayload, Page, PageContext, PagerConfig, Priority, TelegramConfig,
        TmuxSession,
    },
    ports::{ConfigSource, SensitiveContentScanner},
    render::{PageRenderer, PayloadPlanKind, PayloadPlanner, TelegramLimits},
    security::{HeuristicSensitiveContentScanner, SensitiveReason},
};

#[derive(Debug, Default, Clone)]
struct MapConfigSource(BTreeMap<String, String>);

impl ConfigSource for MapConfigSource {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

fn source(entries: &[(&str, &str)]) -> MapConfigSource {
    MapConfigSource(
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    )
}

fn host(name: &str) -> HostName {
    HostName::new(name).expect("test host name is valid")
}

fn body(message: impl Into<String>) -> MessageBody {
    MessageBody::new(message).expect("test message body is valid")
}

fn file_name(name: &str) -> DocumentFileName {
    DocumentFileName::new(name).expect("test document file name is valid")
}

fn context_with_host(name: &str) -> PageContext {
    PageContext::new(host(name), None, None)
}

fn page(message: impl Into<String>, priority: Priority, context: PageContext) -> Page {
    Page::new(body(message), priority, context)
}

fn expect_missing_env(error: AgentPagerError, expected_key: &'static str) {
    match error {
        AgentPagerError::MissingEnv(key) => assert_eq!(key, expected_key),
        other => panic!("expected MissingEnv({expected_key}), got {other:?}"),
    }
}

fn expect_placeholder_env(error: AgentPagerError, expected_key: &'static str) {
    match error {
        AgentPagerError::PlaceholderEnv(key) => assert_eq!(key, expected_key),
        other => panic!("expected PlaceholderEnv({expected_key}), got {other:?}"),
    }
}

fn expect_char_limit(
    error: AgentPagerError,
    expected_label: &'static str,
    expected_actual: usize,
    expected_limit: usize,
) {
    match error {
        AgentPagerError::CharacterLimitExceeded {
            label,
            actual,
            limit,
        } => {
            assert_eq!(label, expected_label);
            assert_eq!(actual, expected_actual);
            assert_eq!(limit, expected_limit);
        }
        other => panic!("expected CharacterLimitExceeded, got {other:?}"),
    }
}

fn expect_sensitive(
    error: AgentPagerError,
    expected_label: &str,
    expected_reason: SensitiveReason,
) {
    match error {
        AgentPagerError::SensitivePayload { label, reason } => {
            assert_eq!(label, expected_label);
            assert_eq!(reason, expected_reason);
        }
        other => panic!("expected SensitivePayload, got {other:?}"),
    }
}

#[test]
fn message_body_trims_input_and_rejects_empty_messages() {
    let message = MessageBody::new("  deploy finished  ").expect("message should be valid");
    assert_eq!(message.as_str(), "deploy finished");
    assert_eq!(message.as_bytes(), b"deploy finished");
    assert_eq!(message.clone().into_bytes(), b"deploy finished".to_vec());

    assert!(matches!(
        MessageBody::new(" \n\t "),
        Err(AgentPagerError::EmptyMessage)
    ));
}

#[test]
fn document_file_name_trims_input_and_rejects_empty_names() {
    let file_name = DocumentFileName::new("  pager-output.md  ").expect("name should be valid");
    assert_eq!(file_name.as_str(), "pager-output.md");
    assert_eq!(file_name.clone().into_string(), "pager-output.md");

    assert!(matches!(
        DocumentFileName::new(" \t\n "),
        Err(AgentPagerError::EmptyDocumentFileName)
    ));
}

#[test]
fn document_rejects_empty_bytes() {
    let file_name = file_name("message.md");
    let document =
        Document::new(file_name.clone(), b"payload".to_vec()).expect("document is valid");
    assert_eq!(document.file_name().as_str(), "message.md");
    assert_eq!(document.bytes(), b"payload");

    assert!(matches!(
        Document::new(file_name, Vec::new()),
        Err(AgentPagerError::EmptyDocument)
    ));
}

#[test]
fn priority_display_matches_wire_strings() {
    assert_eq!(Priority::Normal.as_str(), "normal");
    assert_eq!(Priority::Normal.to_string(), "normal");
    assert_eq!(Priority::High.as_str(), "high");
    assert_eq!(Priority::High.to_string(), "high");
}

#[test]
fn telegram_parse_mode_maps_formats_for_adapter_payloads() {
    assert_eq!(telegram_parse_mode(MessageFormat::Plain), None);
    assert_eq!(
        telegram_parse_mode(MessageFormat::MarkdownV2),
        Some("MarkdownV2")
    );
    assert_eq!(telegram_parse_mode(MessageFormat::Html), Some("HTML"));
}

#[test]
fn config_loader_requires_trims_rejects_placeholders_and_uses_fallback_host() {
    let fallback = host("fallback-host");

    let trimmed_config = PagerConfigLoader::new(
        source(&[
            (BOT_TOKEN_ENV, "  123456:real-token-secret  "),
            (CHAT_ID_ENV, "  -1001234567890  "),
            (DEFAULT_HOST_ENV, "  configured-host  "),
        ]),
        fallback.clone(),
    )
    .load()
    .expect("trimmed config should load");
    assert_eq!(
        trimmed_config.telegram().bot_token(),
        &BotToken::new("123456:real-token-secret").expect("token is valid")
    );
    assert_eq!(
        trimmed_config.telegram().chat_id().as_str(),
        "-1001234567890"
    );
    assert_eq!(trimmed_config.default_host().as_str(), "configured-host");

    let fallback_config = PagerConfigLoader::new(
        source(&[
            (BOT_TOKEN_ENV, "123456:real-token-secret"),
            (CHAT_ID_ENV, "-1001234567890"),
            (DEFAULT_HOST_ENV, "  "),
        ]),
        fallback,
    )
    .load()
    .expect("blank default host should fall back");
    assert_eq!(fallback_config.default_host().as_str(), "fallback-host");

    let absent_default_host_config = PagerConfigLoader::new(
        source(&[
            (BOT_TOKEN_ENV, "123456:real-token-secret"),
            (CHAT_ID_ENV, "-1001234567890"),
        ]),
        host("absent-default-fallback"),
    )
    .load()
    .expect("absent default host should fall back");
    assert_eq!(
        absent_default_host_config.default_host().as_str(),
        "absent-default-fallback"
    );

    expect_missing_env(
        PagerConfigLoader::new(source(&[(CHAT_ID_ENV, "-1001234567890")]), host("fallback"))
            .load()
            .expect_err("missing bot token should fail"),
        BOT_TOKEN_ENV,
    );
    expect_missing_env(
        PagerConfigLoader::new(
            source(&[(BOT_TOKEN_ENV, "123456:real-token-secret")]),
            host("fallback"),
        )
        .load()
        .expect_err("missing chat id should fail"),
        CHAT_ID_ENV,
    );
    expect_placeholder_env(
        PagerConfigLoader::new(
            source(&[
                (BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER),
                (CHAT_ID_ENV, "-1001234567890"),
            ]),
            host("fallback"),
        )
        .load()
        .expect_err("placeholder bot token should fail"),
        BOT_TOKEN_ENV,
    );
    expect_placeholder_env(
        PagerConfigLoader::new(
            source(&[
                (BOT_TOKEN_ENV, "123456:real-token-secret"),
                (CHAT_ID_ENV, CHAT_ID_PLACEHOLDER),
            ]),
            host("fallback"),
        )
        .load()
        .expect_err("placeholder chat id should fail"),
        CHAT_ID_ENV,
    );
}

#[test]
fn secret_values_are_redacted_from_debug_output() {
    let secret = "123456:super-secret-bot-token";
    let config = TelegramConfig::new(
        BotToken::new(secret).expect("token is valid"),
        ChatId::new("-1001234567890").expect("chat id is valid"),
    );
    let telegram_debug = format!("{config:?}");
    assert!(telegram_debug.contains("BotToken(<redacted>)"));
    assert!(!telegram_debug.contains(secret));

    let pager_config = PagerConfig::new(config, host("debug-host"));
    let pager_debug = format!("{pager_config:?}");
    assert!(pager_debug.contains("BotToken(<redacted>)"));
    assert!(!pager_debug.contains(secret));
}

#[test]
fn normal_full_context_rendering_is_stable() {
    let renderer = PageRenderer::default();
    let context = PageContext::new(
        host("agent-host"),
        Some(DisplayPath::new("  ~/work/agent-pager  ").expect("cwd is valid")),
        Some(TmuxSession::new("  pager:1  ").expect("tmux session is valid")),
    );
    let page = page("build finished", Priority::Normal, context);

    assert_eq!(
        renderer.render_page(&page).expect("page should render"),
        "🟡 Agent needs attention\nhost: agent-host\ncwd: ~/work/agent-pager\ntmux: pager:1\npriority: normal\nbuild finished"
    );
}

#[test]
fn high_priority_minimal_rendering_is_stable() {
    let renderer = PageRenderer::default();
    let page = page("please inspect", Priority::High, context_with_host("pi"));

    assert_eq!(
        renderer.render_page(&page).expect("page should render"),
        "🔴 Agent needs attention\nhost: pi\npriority: high\nplease inspect"
    );
}

#[test]
fn document_captions_include_trimmed_messages_and_omit_absent_messages() {
    let renderer = PageRenderer::default();
    let context = context_with_host("agent-host");

    assert_eq!(
        renderer
            .render_document_caption(Some("  attached log  "), Priority::Normal, &context)
            .expect("caption should render"),
        "🟡 Agent needs attention\nhost: agent-host\npriority: normal\nattached log"
    );
    assert_eq!(
        renderer
            .render_document_caption(None, Priority::Normal, &context)
            .expect("caption without message should render"),
        "🟡 Agent needs attention\nhost: agent-host\npriority: normal"
    );
    assert_eq!(
        renderer
            .render_document_caption(Some(" \n\t "), Priority::Normal, &context)
            .expect("blank caption message should be omitted"),
        "🟡 Agent needs attention\nhost: agent-host\npriority: normal"
    );
}

#[test]
fn text_and_caption_limits_count_unicode_characters_not_bytes() {
    let context = context_with_host("unicode-host");
    let unicode_page = page("🙂🙂", Priority::Normal, context.clone());
    let unlimited_text = PageRenderer::default().render_page_unlimited(&unicode_page);
    let text_chars = unlimited_text.chars().count();

    let exact_text_renderer = PageRenderer::new(TelegramLimits {
        text_message_chars: text_chars,
        document_caption_chars: 1024,
    });
    assert_eq!(
        exact_text_renderer
            .render_page(&unicode_page)
            .expect("exact text char limit should pass"),
        unlimited_text
    );

    let too_small_text_renderer = PageRenderer::new(TelegramLimits {
        text_message_chars: text_chars - 1,
        document_caption_chars: 1024,
    });
    expect_char_limit(
        too_small_text_renderer
            .render_page(&unicode_page)
            .expect_err("one Unicode character over the text limit should fail"),
        "Telegram text message",
        text_chars,
        text_chars - 1,
    );

    let caption_message = "résumé 🙂";
    let unlimited_caption = PageRenderer::default()
        .render_document_caption(Some(caption_message), Priority::High, &context)
        .expect("caption should render");
    let caption_chars = unlimited_caption.chars().count();

    let exact_caption_renderer = PageRenderer::new(TelegramLimits {
        text_message_chars: 4096,
        document_caption_chars: caption_chars,
    });
    assert_eq!(
        exact_caption_renderer
            .render_document_caption(Some(caption_message), Priority::High, &context)
            .expect("exact caption char limit should pass"),
        unlimited_caption
    );

    let too_small_caption_renderer = PageRenderer::new(TelegramLimits {
        text_message_chars: 4096,
        document_caption_chars: caption_chars - 1,
    });
    expect_char_limit(
        too_small_caption_renderer
            .render_document_caption(Some(caption_message), Priority::High, &context)
            .expect_err("one Unicode character over the caption limit should fail"),
        "Telegram document caption",
        caption_chars,
        caption_chars - 1,
    );
}

#[test]
fn short_page_plans_as_text_payload() {
    let renderer = PageRenderer::default();
    let planner = PayloadPlanner::default();
    let page = page(
        "short message",
        Priority::Normal,
        context_with_host("agent-host"),
    );
    let rendered = renderer.render_page(&page).expect("page should render");

    let plan = planner
        .plan_page(
            &page,
            rendered.clone(),
            MessageFormat::Html,
            None,
            &renderer,
        )
        .expect("short page should plan as text");

    assert_eq!(plan.kind, PayloadPlanKind::Text);
    match plan.payload {
        OutboundPayload::Text { text, format } => {
            assert_eq!(text, rendered);
            assert_eq!(format, MessageFormat::Html);
        }
        other => panic!("expected text payload, got {other:?}"),
    }
}

#[test]
fn long_pages_plan_as_automatic_documents_with_default_and_override_file_names() {
    let renderer = PageRenderer::default();
    let planner = PayloadPlanner::default();
    let long_body = "x".repeat(4100);
    let page = page(
        long_body.as_str(),
        Priority::High,
        context_with_host("agent-host"),
    );
    let rendered = renderer.render_page_unlimited(&page);

    let default_plan = planner
        .plan_page(
            &page,
            rendered.clone(),
            MessageFormat::MarkdownV2,
            None,
            &renderer,
        )
        .expect("long page should plan as automatic document");
    assert_eq!(default_plan.kind, PayloadPlanKind::AutomaticDocument);
    match default_plan.payload {
        OutboundPayload::Document {
            document,
            caption,
            format,
        } => {
            assert_eq!(document.file_name().as_str(), "agent-pager-message.md");
            assert_eq!(document.bytes(), long_body.as_bytes());
            assert_eq!(
                caption.as_deref(),
                Some(
                    "🔴 Agent needs attention\nhost: agent-host\npriority: high\nMessage attached as document"
                )
            );
            assert_eq!(format, MessageFormat::MarkdownV2);
        }
        other => panic!("expected automatic document payload, got {other:?}"),
    }

    let override_plan = planner
        .plan_page(
            &page,
            rendered,
            MessageFormat::Plain,
            Some(file_name("override.md")),
            &renderer,
        )
        .expect("long page with override should plan as automatic document");
    assert_eq!(override_plan.kind, PayloadPlanKind::AutomaticDocument);
    match override_plan.payload {
        OutboundPayload::Document {
            document,
            caption,
            format,
        } => {
            assert_eq!(document.file_name().as_str(), "override.md");
            assert_eq!(document.bytes(), long_body.as_bytes());
            assert_eq!(
                caption.as_deref(),
                Some(
                    "🔴 Agent needs attention\nhost: agent-host\npriority: high\nMessage attached as document"
                )
            );
            assert_eq!(format, MessageFormat::Plain);
        }
        other => panic!("expected automatic document payload with override, got {other:?}"),
    }
}

#[test]
fn explicit_documents_plan_as_document_payloads() {
    let planner = PayloadPlanner::default();
    let document = Document::new(file_name("trace.log"), b"trace bytes".to_vec())
        .expect("document should be valid");

    let plan = planner.plan_explicit_document(
        document,
        "operator supplied caption".to_owned(),
        MessageFormat::Plain,
    );

    assert_eq!(plan.kind, PayloadPlanKind::ExplicitDocument);
    match plan.payload {
        OutboundPayload::Document {
            document,
            caption,
            format,
        } => {
            assert_eq!(document.file_name().as_str(), "trace.log");
            assert_eq!(document.bytes(), b"trace bytes");
            assert_eq!(caption.as_deref(), Some("operator supplied caption"));
            assert_eq!(format, MessageFormat::Plain);
        }
        other => panic!("expected explicit document payload, got {other:?}"),
    }
}

#[test]
fn long_document_caption_returns_structured_limit_error() {
    let renderer = PageRenderer::new(TelegramLimits {
        text_message_chars: 4096,
        document_caption_chars: 12,
    });
    let context = context_with_host("agent-host");

    let error = renderer
        .render_document_caption(Some("caption"), Priority::Normal, &context)
        .expect_err("caption should exceed the configured limit");

    let actual = "🟡 Agent needs attention\nhost: agent-host\npriority: normal\ncaption"
        .chars()
        .count();
    expect_char_limit(error, "Telegram document caption", actual, 12);
}

#[test]
fn sensitive_content_scanner_reports_expected_findings_and_ignores_benign_inputs() {
    let scanner = HeuristicSensitiveContentScanner;

    expect_sensitive(
        scanner
            .scan_text("private-key", "-----BEGIN OPENSSH PRIVATE KEY-----\nabc")
            .expect_err("private key should be rejected"),
        "private-key",
        SensitiveReason::PrivateKeyMaterial,
    );
    expect_sensitive(
        scanner
            .scan_text("assignment", "export DATABASE_PASSWORD=swordfish")
            .expect_err("secret assignment should be rejected"),
        "assignment",
        SensitiveReason::SecretLookingAssignment,
    );
    expect_sensitive(
        scanner
            .scan_text(
                "github",
                "created ghp_abcdefghijklmnopqrstuvwxyz0123456789 token",
            )
            .expect_err("GitHub token should be rejected"),
        "github",
        SensitiveReason::TokenLookingValue,
    );
    expect_sensitive(
        scanner
            .scan_text("slack", "bot token xoxb-1234567890-abcdefghij")
            .expect_err("Slack token should be rejected"),
        "slack",
        SensitiveReason::TokenLookingValue,
    );
    expect_sensitive(
        scanner
            .scan_text("aws", "using AKIA1234567890ABCDEF in this example")
            .expect_err("AWS access key should be rejected"),
        "aws",
        SensitiveReason::TokenLookingValue,
    );
    expect_sensitive(
        scanner
            .scan_text("telegram", "123456:abcdefghijklmnopqrstuvwxyz_ABCDEF")
            .expect_err("Telegram bot token should be rejected"),
        "telegram",
        SensitiveReason::TelegramBotToken,
    );

    scanner
        .scan_text("benign", "deploy finished; no credentials here")
        .expect("benign text should pass");
    scanner
        .scan_bytes("non-utf8", &[0xff, 0xfe, b'A', b'K', b'I', b'A'])
        .expect("non-UTF8 bytes are ignored by text heuristics");
}
