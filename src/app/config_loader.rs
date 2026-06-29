use crate::{
    AgentPagerError,
    domain::{
        BOT_TOKEN_ENV, BotToken, CHAT_ID_ENV, ChatId, DEFAULT_HOST_ENV, HostName, PagerConfig,
        TelegramConfig,
    },
    ports::ConfigSource,
};

#[derive(Debug, Clone)]
pub struct PagerConfigLoader<C> {
    source: C,
    host_fallback: HostName,
}

impl<C> PagerConfigLoader<C> {
    pub fn new(source: C, host_fallback: HostName) -> Self {
        Self {
            source,
            host_fallback,
        }
    }
}

impl<C> PagerConfigLoader<C>
where
    C: ConfigSource,
{
    pub fn load(&self) -> Result<PagerConfig, AgentPagerError> {
        let bot_token = BotToken::new(required_value(&self.source, BOT_TOKEN_ENV)?)?;
        let chat_id = ChatId::new(required_value(&self.source, CHAT_ID_ENV)?)?;
        let default_host = optional_trimmed(&self.source, DEFAULT_HOST_ENV)
            .map(HostName::new)
            .transpose()?
            .unwrap_or_else(|| self.host_fallback.clone());

        Ok(PagerConfig::new(
            TelegramConfig::new(bot_token, chat_id),
            default_host,
        ))
    }

    pub fn source(&self) -> &C {
        &self.source
    }

    pub fn host_fallback(&self) -> &HostName {
        &self.host_fallback
    }
}

pub(crate) fn optional_trimmed<C>(source: &C, key: &str) -> Option<String>
where
    C: ConfigSource,
{
    source
        .get(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required_value<C>(source: &C, key: &'static str) -> Result<String, AgentPagerError>
where
    C: ConfigSource,
{
    optional_trimmed(source, key).ok_or(AgentPagerError::MissingEnv(key))
}
