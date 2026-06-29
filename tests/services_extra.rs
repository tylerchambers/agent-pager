use std::sync::{Arc, Mutex};

use agent_pager::{
    AgentPagerError,
    app::{PagerConfigLoader, TestOutcome, TestPageService},
    domain::{BotToken, ChatId, HostName, MessageFormat, OutboundPayload, TelegramConfig},
    ports::{ConfigSource, PagerGateway},
    render::PageRenderer,
};
use async_trait::async_trait;

#[derive(Debug, Clone)]
struct SingleValueConfig;

impl ConfigSource for SingleValueConfig {
    fn get(&self, key: &str) -> Option<String> {
        match key {
            "AGENT_PAGER_TELEGRAM_BOT_TOKEN" => Some(" token ".to_owned()),
            "AGENT_PAGER_TELEGRAM_CHAT_ID" => Some(" chat ".to_owned()),
            _ => None,
        }
    }
}

#[test]
fn config_loader_exposes_source_and_fallback_for_composition() {
    let loader = PagerConfigLoader::new(
        SingleValueConfig,
        HostName::new(" fallback ").expect("fallback"),
    );

    assert_eq!(loader.host_fallback().as_str(), "fallback");
    assert_eq!(
        loader
            .source()
            .get("AGENT_PAGER_TELEGRAM_BOT_TOKEN")
            .as_deref(),
        Some(" token ")
    );
    let config = loader.load().expect("config");
    assert_eq!(config.default_host().as_str(), "fallback");
}

#[derive(Debug, Clone, Default)]
struct RecordingGateway {
    payloads: Arc<Mutex<Vec<OutboundPayload>>>,
}

#[async_trait]
impl PagerGateway for RecordingGateway {
    async fn send(&self, payload: OutboundPayload) -> Result<(), AgentPagerError> {
        self.payloads.lock().expect("payloads").push(payload);
        Ok(())
    }
}

#[tokio::test]
async fn test_page_service_sends_plain_smoke_message() {
    let gateway = RecordingGateway::default();
    let seen = gateway.payloads.clone();
    let service = TestPageService::new(gateway, PageRenderer::default());
    let config = agent_pager::domain::PagerConfig::new(
        TelegramConfig::new(
            BotToken::new("token").expect("token"),
            ChatId::new("chat").expect("chat"),
        ),
        HostName::new("desktop").expect("host"),
    );

    let outcome = service.send(&config).await.expect("test page sent");

    assert_eq!(outcome, TestOutcome::Sent);
    let payloads = seen.lock().expect("payloads");
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        payloads[0],
        OutboundPayload::Text {
            text: "agent-pager test from desktop".to_owned(),
            format: MessageFormat::Plain,
        }
    );
}
