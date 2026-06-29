use std::collections::HashMap;

use agent_pager::{
    app::DoctorService,
    domain::{
        BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER, CHAT_ID_ENV, CHAT_ID_PLACEHOLDER, DEFAULT_HOST_ENV,
        HostName,
    },
    ports::ConfigSource,
};

#[test]
fn reports_token_and_chat_presence_by_length_without_values() {
    let token = "123456789:super-secret-token";
    let chat = "987654321";
    let source = FakeConfigSource::new([
        (BOT_TOKEN_ENV, token),
        (CHAT_ID_ENV, chat),
        (DEFAULT_HOST_ENV, "env-host"),
    ]);
    let report = DoctorService::new(source, host("fallback-host")).run();

    assert!(report.ok);
    assert!(report.lines.contains(&format!(
        "bot token: present, length {}",
        token.chars().count()
    )));
    assert!(report.lines.contains(&format!(
        "chat id: present, length {}",
        chat.chars().count()
    )));
    let joined = report.lines.join("\n");
    assert!(!joined.contains(token));
    assert!(!joined.contains(chat));
}

#[test]
fn reports_missing_token_and_invalid_config() {
    let source = FakeConfigSource::new([(CHAT_ID_ENV, "123456")]);
    let report = DoctorService::new(source, host("fallback-host")).run();

    assert!(!report.ok);
    assert!(report.lines.contains(&"bot token: missing".to_owned()));
    assert!(report.lines.iter().any(|line| line
        == "config: missing required environment variable AGENT_PAGER_TELEGRAM_BOT_TOKEN"));
}

#[test]
fn reports_placeholder_token_as_invalid_without_printing_value() {
    let source = FakeConfigSource::new([
        (BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER),
        (CHAT_ID_ENV, "123456"),
    ]);
    let report = DoctorService::new(source, host("fallback-host")).run();

    assert!(!report.ok);
    assert!(report.lines.contains(&format!(
        "bot token: present, length {}",
        BOT_TOKEN_PLACEHOLDER.chars().count()
    )));
    assert!(report.lines.iter().any(|line| line
        == "config: AGENT_PAGER_TELEGRAM_BOT_TOKEN still contains the example placeholder value"));
    assert!(!report.lines.join("\n").contains(BOT_TOKEN_PLACEHOLDER));
}

#[test]
fn uses_fallback_host_when_default_host_env_absent() {
    let source = FakeConfigSource::new([(BOT_TOKEN_ENV, "token"), (CHAT_ID_ENV, "chat")]);
    let report = DoctorService::new(source, host("fallback-host")).run();

    assert!(report.ok);
    assert!(
        report
            .lines
            .contains(&"default host: fallback-host".to_owned())
    );
    assert!(report.lines.contains(&"config: ok".to_owned()));
}

#[test]
fn reports_ok_false_for_invalid_config() {
    let source = FakeConfigSource::new([
        (BOT_TOKEN_ENV, "valid-token"),
        (CHAT_ID_ENV, CHAT_ID_PLACEHOLDER),
    ]);
    let report = DoctorService::new(source, host("fallback-host")).run();

    assert!(!report.ok);
    assert!(report.lines.contains(&format!(
        "chat id: present, length {}",
        CHAT_ID_PLACEHOLDER.chars().count()
    )));
    assert!(report.lines.iter().any(|line| line
        == "config: AGENT_PAGER_TELEGRAM_CHAT_ID still contains the example placeholder value"));
}

#[derive(Clone, Default)]
struct FakeConfigSource {
    values: HashMap<&'static str, String>,
}

impl FakeConfigSource {
    fn new<const N: usize>(values: [(&'static str, &str); N]) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|(key, value)| (key, value.to_owned()))
                .collect(),
        }
    }
}

impl ConfigSource for FakeConfigSource {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

fn host(value: &str) -> HostName {
    HostName::new(value).expect("host")
}
