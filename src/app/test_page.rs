use crate::{
    AgentPagerError,
    domain::{MessageFormat, OutboundPayload, PagerConfig},
    ports::PagerGateway,
    render::PageRenderer,
};

use super::TestOutcome;

#[derive(Debug, Clone)]
pub struct TestPageService<G> {
    gateway: G,
    renderer: PageRenderer,
}

impl<G> TestPageService<G> {
    pub fn new(gateway: G, renderer: PageRenderer) -> Self {
        Self { gateway, renderer }
    }
}

impl<G> TestPageService<G>
where
    G: PagerGateway,
{
    pub async fn send(&self, config: &PagerConfig) -> Result<TestOutcome, AgentPagerError> {
        let text = self.renderer.render_test_text(config.default_host());
        self.gateway
            .send(OutboundPayload::Text {
                text,
                format: MessageFormat::Plain,
            })
            .await?;
        Ok(TestOutcome::Sent)
    }
}
