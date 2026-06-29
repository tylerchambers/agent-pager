use async_trait::async_trait;

use crate::{AgentPagerError, domain::OutboundPayload};

#[async_trait]
pub trait PagerGateway: Send + Sync {
    async fn send(&self, payload: OutboundPayload) -> Result<(), AgentPagerError>;
}
