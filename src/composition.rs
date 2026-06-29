use reqwest::Url;

use crate::{
    AgentPagerError,
    adapters::{
        EnvConfigSource, FsDocumentStore, FsSkillStore, StdioMessageReader, SystemContextProvider,
        SystemProcessRunner, TelegramPagerGateway,
    },
    app::{
        AppOutcome, DoctorService, InstallSkillService, PagerConfigLoader, SendPageService,
        TestPageService,
    },
    command::AppCommand,
    domain::{DEFAULT_HOST_ENV, HostName},
    ports::ConfigSource,
    render::{PageRenderer, PayloadPlanner},
    security::HeuristicSensitiveContentScanner,
};

const AGENT_PAGER_SKILL: &str = include_str!("../skills/agent-pager/SKILL.md");
const HOSTNAME_ENV: &str = "HOSTNAME";
const DEFAULT_TELEGRAM_BASE_URL: &str = "https://api.telegram.org";

#[derive(Debug, Clone)]
pub struct App<C = EnvConfigSource> {
    source: C,
    http: reqwest::Client,
    host_fallback: HostName,
    telegram_base_url: Url,
}

impl App<EnvConfigSource> {
    pub fn production() -> Result<Self, AgentPagerError> {
        let source = EnvConfigSource;
        let host_fallback = host_fallback(&source)?;
        let telegram_base_url = Url::parse(DEFAULT_TELEGRAM_BASE_URL)
            .map_err(|error| AgentPagerError::Other(error.to_string()))?;
        Ok(Self::new(
            source,
            reqwest::Client::new(),
            host_fallback,
            telegram_base_url,
        ))
    }
}

impl<C> App<C> {
    pub fn new(
        source: C,
        http: reqwest::Client,
        host_fallback: HostName,
        telegram_base_url: Url,
    ) -> Self {
        Self {
            source,
            http,
            host_fallback,
            telegram_base_url,
        }
    }
}

impl<C> App<C>
where
    C: ConfigSource + Clone,
{
    pub async fn run(&self, command: AppCommand) -> Result<AppOutcome, AgentPagerError> {
        match command {
            AppCommand::Send(command) => {
                let config = self.config_loader().load()?;
                let gateway = TelegramPagerGateway::with_base_url(
                    self.http.clone(),
                    config.telegram().clone(),
                    self.telegram_base_url.clone(),
                );
                let service = SendPageService::new(
                    SystemContextProvider::new(SystemProcessRunner, self.source.clone()),
                    StdioMessageReader,
                    FsDocumentStore::new(StdioMessageReader),
                    HeuristicSensitiveContentScanner,
                    gateway,
                    PageRenderer::default(),
                    PayloadPlanner::default(),
                );
                service.send(&command, &config).await.map(AppOutcome::Send)
            }
            AppCommand::Doctor => {
                let service = DoctorService::new(self.source.clone(), self.host_fallback.clone());
                Ok(AppOutcome::Doctor(service.run()))
            }
            AppCommand::InstallSkill(command) => {
                let service =
                    InstallSkillService::new(FsSkillStore, self.source.clone(), AGENT_PAGER_SKILL);
                service.install(&command).map(AppOutcome::InstallSkill)
            }
            AppCommand::Test => {
                let config = self.config_loader().load()?;
                let gateway = TelegramPagerGateway::with_base_url(
                    self.http.clone(),
                    config.telegram().clone(),
                    self.telegram_base_url.clone(),
                );
                let service = TestPageService::new(gateway, PageRenderer::default());
                service.send(&config).await.map(AppOutcome::Test)
            }
        }
    }

    fn config_loader(&self) -> PagerConfigLoader<C> {
        PagerConfigLoader::new(self.source.clone(), self.host_fallback.clone())
    }
}

fn host_fallback(source: &EnvConfigSource) -> Result<HostName, AgentPagerError> {
    let host = source
        .get(DEFAULT_HOST_ENV)
        .or_else(|| source.get(HOSTNAME_ENV))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    HostName::new(host)
}
