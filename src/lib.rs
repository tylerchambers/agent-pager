use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use serde::Serialize;
use std::{env, fmt, path::Path, process::Command};

pub const BOT_TOKEN_ENV: &str = "AGENT_PAGER_TELEGRAM_BOT_TOKEN";
pub const CHAT_ID_ENV: &str = "AGENT_PAGER_TELEGRAM_CHAT_ID";
pub const DEFAULT_HOST_ENV: &str = "AGENT_PAGER_DEFAULT_HOST";
const BOT_TOKEN_PLACEHOLDER: &str = "replace-with-botfather-token";
const CHAT_ID_PLACEHOLDER: &str = "replace-with-chat-id";

const TELEGRAM_SEND_MESSAGE_PREFIX: &str = "https://api.telegram.org/bot";
const ATTACH_INSTRUCTION: &str = "SSH in and attach tmux.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Priority {
    Normal,
    High,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
        }
    }

    fn header(self) -> &'static str {
        match self {
            Self::Normal => "🟡 Agent needs attention",
            Self::High => "🔴 Agent needs attention",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagerConfig {
    pub bot_token: String,
    pub chat_id: String,
    pub default_host: String,
}

impl PagerConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|key| env::var(key).ok(), fallback_host())
    }

    pub fn from_lookup<F>(lookup: F, fallback_host: impl Into<String>) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let bot_token = required_value(&lookup, BOT_TOKEN_ENV)?;
        let chat_id = required_value(&lookup, CHAT_ID_ENV)?;
        let default_host = lookup(DEFAULT_HOST_ENV)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback_host.into());

        Ok(Self {
            bot_token,
            chat_id,
            default_host,
        })
    }
}

fn required_value<F>(lookup: &F, key: &'static str) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    let value = lookup(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing required environment variable {key}"))?;
    reject_placeholder_value(key, &value)?;
    Ok(value)
}

fn reject_placeholder_value(key: &'static str, value: &str) -> Result<()> {
    let is_placeholder = matches!(
        (key, value),
        (BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER) | (CHAT_ID_ENV, CHAT_ID_PLACEHOLDER)
    );

    if is_placeholder {
        bail!("{key} still contains the example placeholder value");
    }

    Ok(())
}

fn fallback_host() -> String {
    env::var(DEFAULT_HOST_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            env::var("HOSTNAME")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageContext {
    pub host: String,
    pub cwd: Option<String>,
    pub tmux: Option<String>,
}

impl PageContext {
    pub fn gather(host: String, include_cwd: bool, include_tmux: bool) -> Result<Self> {
        let cwd = include_cwd.then(|| {
            let current = env::current_dir()?;
            let home = env::var_os("HOME");
            let home = home.as_deref().map(Path::new);
            Ok::<_, anyhow::Error>(display_path(&current, home))
        });

        Ok(Self {
            host,
            cwd: cwd.transpose()?,
            tmux: include_tmux
                .then(|| current_tmux_session().unwrap_or_else(|| "unavailable".to_owned())),
        })
    }
}

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home.filter(|home| !home.as_os_str().is_empty()) {
        if path == home {
            return "~".to_owned();
        }

        if let Ok(relative) = path.strip_prefix(home) {
            return format!("~/{}", relative.display());
        }
    }

    path.display().to_string()
}

fn current_tmux_session() -> Option<String> {
    env::var_os("TMUX")?;

    let output = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let session = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!session.is_empty()).then_some(session)
}

pub fn build_page_text(message: &str, priority: Priority, context: &PageContext) -> Result<String> {
    let message = message.trim();
    if message.is_empty() {
        bail!("page message cannot be empty");
    }

    let mut lines = Vec::with_capacity(7);
    lines.push(priority.header().to_owned());
    lines.push(format!("host: {}", context.host));

    if let Some(cwd) = &context.cwd {
        lines.push(format!("cwd: {cwd}"));
    }

    if let Some(tmux) = &context.tmux {
        lines.push(format!("tmux: {tmux}"));
    }

    lines.push(format!("priority: {priority}"));
    lines.push(message.to_owned());
    lines.push(ATTACH_INSTRUCTION.to_owned());

    Ok(lines.join("\n"))
}

pub fn build_test_text(host: &str) -> String {
    format!("agent-pager test from {}", host.trim())
}

#[derive(Debug, Serialize)]
struct TelegramSendMessage<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
}

pub async fn send_telegram_message(
    client: &reqwest::Client,
    config: &PagerConfig,
    text: &str,
) -> Result<()> {
    let endpoint = format!(
        "{}{}/sendMessage",
        TELEGRAM_SEND_MESSAGE_PREFIX, config.bot_token
    );
    let payload = TelegramSendMessage {
        chat_id: &config.chat_id,
        text,
        disable_web_page_preview: true,
    };

    let response = client
        .post(endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|err| {
            anyhow!(
                "failed to call Telegram sendMessage: {}",
                redact_token(&err.to_string(), &config.bot_token)
            )
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Telegram sendMessage failed with status {status}: {}",
            truncate(&redact_token(&body, &config.bot_token), 512)
        );
    }

    Ok(())
}

fn redact_token(input: &str, token: &str) -> String {
    if token.is_empty() {
        input.to_owned()
    } else {
        input.replace(token, "<redacted-bot-token>")
    }
}

fn truncate(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, path::PathBuf};

    #[test]
    fn formats_normal_page_with_requested_context() {
        let context = PageContext {
            host: "desktop".to_owned(),
            cwd: Some("~/src/walletd".to_owned()),
            tmux: Some("main".to_owned()),
        };

        let text = build_page_text(
            "Tests failed in descriptor parser.",
            Priority::Normal,
            &context,
        )
        .expect("valid page text");

        assert_eq!(
            text,
            "🟡 Agent needs attention\nhost: desktop\ncwd: ~/src/walletd\ntmux: main\npriority: normal\nTests failed in descriptor parser.\nSSH in and attach tmux."
        );
    }

    #[test]
    fn formats_high_page_without_optional_context() {
        let context = PageContext {
            host: "buildbox".to_owned(),
            cwd: None,
            tmux: None,
        };

        let text =
            build_page_text("Need review", Priority::High, &context).expect("valid page text");

        assert_eq!(
            text,
            "🔴 Agent needs attention\nhost: buildbox\npriority: high\nNeed review\nSSH in and attach tmux."
        );
    }

    #[test]
    fn rejects_empty_page_message() {
        let context = PageContext {
            host: "desktop".to_owned(),
            cwd: None,
            tmux: None,
        };

        let error = build_page_text("   ", Priority::Normal, &context).unwrap_err();
        assert_eq!(error.to_string(), "page message cannot be empty");
    }

    #[test]
    fn displays_home_relative_paths_with_tilde() {
        let home = PathBuf::from("/home/tyler");

        assert_eq!(display_path(Path::new("/home/tyler"), Some(&home)), "~");
        assert_eq!(
            display_path(Path::new("/home/tyler/src/walletd"), Some(&home)),
            "~/src/walletd"
        );
        assert_eq!(
            display_path(Path::new("/var/tmp/walletd"), Some(&home)),
            "/var/tmp/walletd"
        );
    }

    #[test]
    fn loads_required_env_and_default_host() {
        let vars = HashMap::from([
            (BOT_TOKEN_ENV, " token "),
            (CHAT_ID_ENV, " 123456789 "),
            (DEFAULT_HOST_ENV, " desktop "),
        ]);
        let config = PagerConfig::from_lookup(
            |key| vars.get(key).map(|value| (*value).to_owned()),
            "fallback",
        )
        .expect("config loads");

        assert_eq!(
            config,
            PagerConfig {
                bot_token: "token".to_owned(),
                chat_id: "123456789".to_owned(),
                default_host: "desktop".to_owned(),
            }
        );
    }

    #[test]
    fn requires_token_and_chat_id() {
        let error = PagerConfig::from_lookup(|_| None, "fallback").unwrap_err();
        assert_eq!(
            error.to_string(),
            "missing required environment variable AGENT_PAGER_TELEGRAM_BOT_TOKEN"
        );

        let error = PagerConfig::from_lookup(
            |key| (key == BOT_TOKEN_ENV).then(|| "token".to_owned()),
            "fallback",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "missing required environment variable AGENT_PAGER_TELEGRAM_CHAT_ID"
        );
    }

    #[test]
    fn rejects_example_placeholder_credentials() {
        let vars = HashMap::from([
            (BOT_TOKEN_ENV, BOT_TOKEN_PLACEHOLDER),
            (CHAT_ID_ENV, CHAT_ID_PLACEHOLDER),
        ]);
        let error = PagerConfig::from_lookup(
            |key| vars.get(key).map(|value| (*value).to_owned()),
            "fallback",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "AGENT_PAGER_TELEGRAM_BOT_TOKEN still contains the example placeholder value"
        );

        let vars = HashMap::from([
            (BOT_TOKEN_ENV, "123456:real-token-shape"),
            (CHAT_ID_ENV, CHAT_ID_PLACEHOLDER),
        ]);
        let error = PagerConfig::from_lookup(
            |key| vars.get(key).map(|value| (*value).to_owned()),
            "fallback",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "AGENT_PAGER_TELEGRAM_CHAT_ID still contains the example placeholder value"
        );
    }

    #[test]
    fn builds_raw_test_message() {
        assert_eq!(build_test_text("desktop"), "agent-pager test from desktop");
    }
}
