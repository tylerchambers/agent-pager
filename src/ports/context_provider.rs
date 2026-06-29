use crate::{
    AgentPagerError,
    command::ContextOptions,
    domain::{HostName, PageContext},
};

pub trait ContextProvider {
    fn gather(
        &self,
        default_host: HostName,
        options: ContextOptions,
    ) -> Result<PageContext, AgentPagerError>;
}
