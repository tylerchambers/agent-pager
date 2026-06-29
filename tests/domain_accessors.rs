use std::path::Path;

use agent_pager::{
    AgentPagerError,
    domain::{
        BotToken, ChatId, DisplayPath, Document, DocumentFileName, HostName, MessageBody,
        MessageFormat, OutboundPayload, Page, PageContext, PagerConfig, Priority, TelegramConfig,
        TmuxSession, display_path,
    },
};

#[test]
fn message_format_display_and_wire_strings_are_stable() {
    assert_eq!(MessageFormat::Plain.as_str(), "plain");
    assert_eq!(MessageFormat::MarkdownV2.as_str(), "markdown-v2");
    assert_eq!(MessageFormat::Html.as_str(), "html");
    assert_eq!(MessageFormat::Html.to_string(), "html");
}

#[test]
fn context_value_objects_validate_and_expose_values() {
    let cwd = DisplayPath::new(" ~/repo ").expect("cwd");
    let tmux = TmuxSession::new(" main ").expect("tmux");
    let context = PageContext::new(
        HostName::new(" desktop ").expect("host"),
        Some(cwd.clone()),
        Some(tmux.clone()),
    );

    assert_eq!(cwd.as_str(), "~/repo");
    assert_eq!(tmux.as_str(), "main");
    assert_eq!(TmuxSession::unavailable().as_str(), "unavailable");
    assert_eq!(context.host().as_str(), "desktop");
    assert_eq!(context.cwd().expect("cwd").as_str(), "~/repo");
    assert_eq!(context.tmux().expect("tmux").as_str(), "main");
    assert!(matches!(
        DisplayPath::new(" ").expect_err("empty path"),
        AgentPagerError::InvalidCommand(_)
    ));
    assert!(matches!(
        TmuxSession::new(" ").expect_err("empty tmux"),
        AgentPagerError::InvalidCommand(_)
    ));
    assert!(matches!(
        HostName::new(" ").expect_err("empty host"),
        AgentPagerError::InvalidCommand(_)
    ));
}

#[test]
fn display_path_uses_home_relative_tilde_when_possible() {
    let home = Path::new("/home/agent");

    assert_eq!(display_path(Path::new("/home/agent"), Some(home)), "~");
    assert_eq!(
        display_path(Path::new("/home/agent/src/repo"), Some(home)),
        "~/src/repo"
    );
    assert_eq!(
        display_path(Path::new("/var/tmp/repo"), Some(home)),
        "/var/tmp/repo"
    );
    assert_eq!(
        display_path(Path::new("/var/tmp/repo"), None),
        "/var/tmp/repo"
    );
}

#[test]
fn document_and_message_debug_output_omits_payload_contents() {
    let message = MessageBody::new("very secret body").expect("message");
    assert_eq!(message.clone().into_bytes(), b"very secret body");
    assert!(!format!("{message:?}").contains("very secret body"));

    let file_name = DocumentFileName::new(" secret.txt ").expect("file name");
    assert_eq!(file_name.clone().into_string(), "secret.txt");
    let document = Document::new(file_name, b"secret document bytes".to_vec()).expect("document");
    assert_eq!(document.file_name().as_str(), "secret.txt");
    assert_eq!(document.bytes(), b"secret document bytes");
    assert!(!format!("{document:?}").contains("secret document bytes"));

    let (file_name, bytes) = document.into_parts();
    assert_eq!(file_name.as_str(), "secret.txt");
    assert_eq!(bytes, b"secret document bytes");
}

#[test]
fn page_and_payload_debug_output_omits_payload_text() {
    let page = Page::new(
        MessageBody::new("payload text").expect("message"),
        Priority::High,
        PageContext::new(HostName::new("desktop").expect("host"), None, None),
    );
    assert_eq!(page.body().as_str(), "payload text");
    assert_eq!(page.priority(), Priority::High);
    assert_eq!(page.context().host().as_str(), "desktop");
    assert!(!format!("{page:?}").contains("payload text"));

    let text_payload = OutboundPayload::Text {
        text: "secret text".to_owned(),
        format: MessageFormat::Plain,
    };
    assert!(!format!("{text_payload:?}").contains("secret text"));

    let doc_payload = OutboundPayload::Document {
        document: Document::new(
            DocumentFileName::new("report.md").expect("name"),
            b"secret bytes".to_vec(),
        )
        .expect("document"),
        caption: Some("secret caption".to_owned()),
        format: MessageFormat::Html,
    };
    assert!(!format!("{doc_payload:?}").contains("secret caption"));
    assert!(!format!("{doc_payload:?}").contains("secret bytes"));
}

#[test]
fn config_accessors_and_lengths_do_not_require_public_secret_fields() {
    let token = BotToken::new(" 123456:abcdefghijklmnopqrstuvwxyz ").expect("token");
    let chat = ChatId::new(" 987654321 ").expect("chat");
    assert_eq!(token.len_chars(), 33);
    assert_eq!(chat.as_str(), "987654321");
    assert_eq!(chat.len_chars(), 9);

    let telegram = TelegramConfig::new(token, chat);
    let config = PagerConfig::new(telegram, HostName::new("desktop").expect("host"));

    assert_eq!(config.telegram().chat_id().as_str(), "987654321");
    assert_eq!(config.default_host().as_str(), "desktop");
    assert!(!format!("{config:?}").contains("abcdefghijklmnopqrstuvwxyz"));
}
