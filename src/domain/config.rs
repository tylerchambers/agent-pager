use std::fmt;

use secrecy::{ExposeSecret, SecretString};

use crate::AgentPagerError;

pub const BOT_TOKEN_ENV: &str = "AGENT_PAGER_TELEGRAM_BOT_TOKEN";
pub const CHAT_ID_ENV: &str = "AGENT_PAGER_TELEGRAM_CHAT_ID";
pub const DEFAULT_HOST_ENV: &str = "AGENT_PAGER_DEFAULT_HOST";
pub const BOT_TOKEN_PLACEHOLDER: &str = "replace-with-botfather-token";
pub const CHAT_ID_PLACEHOLDER: &str = "replace-with-chat-id";

#[derive(Clone)]
pub struct BotToken(SecretString);

impl BotToken {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let value = input.into().trim().to_owned();
        if value.is_empty() {
            return Err(AgentPagerError::MissingEnv(BOT_TOKEN_ENV));
        }
        if value == BOT_TOKEN_PLACEHOLDER {
            return Err(AgentPagerError::PlaceholderEnv(BOT_TOKEN_ENV));
        }
        Ok(Self(SecretString::new(value)))
    }

    pub fn len_chars(&self) -> usize {
        self.0.expose_secret().chars().count()
    }

    pub(crate) fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for BotToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BotToken(<redacted>)")
    }
}

impl PartialEq for BotToken {
    fn eq(&self, other: &Self) -> bool {
        self.expose_secret() == other.expose_secret()
    }
}

impl Eq for BotToken {}

#[derive(Clone, PartialEq, Eq)]
pub struct ChatId(String);

impl ChatId {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let value = input.into().trim().to_owned();
        if value.is_empty() {
            return Err(AgentPagerError::MissingEnv(CHAT_ID_ENV));
        }
        if value == CHAT_ID_PLACEHOLDER {
            return Err(AgentPagerError::PlaceholderEnv(CHAT_ID_ENV));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn len_chars(&self) -> usize {
        self.0.chars().count()
    }
}

impl fmt::Debug for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatId")
            .field("chars", &self.0.chars().count())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HostName(String);

impl HostName {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let value = input.into().trim().to_owned();
        if value.is_empty() {
            return Err(AgentPagerError::InvalidCommand(
                "host name cannot be empty".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HostName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HostName").field(&self.0).finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TelegramConfig {
    bot_token: BotToken,
    chat_id: ChatId,
}

impl TelegramConfig {
    pub fn new(bot_token: BotToken, chat_id: ChatId) -> Self {
        Self { bot_token, chat_id }
    }

    pub fn bot_token(&self) -> &BotToken {
        &self.bot_token
    }

    pub fn chat_id(&self) -> &ChatId {
        &self.chat_id
    }
}

impl fmt::Debug for TelegramConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelegramConfig")
            .field("bot_token", &self.bot_token)
            .field("chat_id", &self.chat_id)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PagerConfig {
    telegram: TelegramConfig,
    default_host: HostName,
}

impl PagerConfig {
    pub fn new(telegram: TelegramConfig, default_host: HostName) -> Self {
        Self {
            telegram,
            default_host,
        }
    }

    pub fn telegram(&self) -> &TelegramConfig {
        &self.telegram
    }

    pub fn default_host(&self) -> &HostName {
        &self.default_host
    }
}

impl fmt::Debug for PagerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PagerConfig")
            .field("telegram", &self.telegram)
            .field("default_host", &self.default_host)
            .finish()
    }
}
