use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use reqwest::multipart::{Form, Part};
use serde::Serialize;
use std::{env, fmt, path::Path, process::Command};

pub const BOT_TOKEN_ENV: &str = "AGENT_PAGER_TELEGRAM_BOT_TOKEN";
pub const CHAT_ID_ENV: &str = "AGENT_PAGER_TELEGRAM_CHAT_ID";
pub const DEFAULT_HOST_ENV: &str = "AGENT_PAGER_DEFAULT_HOST";
const BOT_TOKEN_PLACEHOLDER: &str = "replace-with-botfather-token";
const CHAT_ID_PLACEHOLDER: &str = "replace-with-chat-id";

const TELEGRAM_SEND_MESSAGE_PREFIX: &str = "https://api.telegram.org/bot";
pub const TELEGRAM_TEXT_MESSAGE_LIMIT: usize = 4096;
pub const TELEGRAM_DOCUMENT_CAPTION_LIMIT: usize = 1024;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MessageFormat {
    Plain,
    #[value(name = "markdown-v2")]
    MarkdownV2,
    Html,
}

impl MessageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::MarkdownV2 => "markdown-v2",
            Self::Html => "html",
        }
    }

    pub fn telegram_parse_mode(self) -> Option<&'static str> {
        match self {
            Self::Plain => None,
            Self::MarkdownV2 => Some("MarkdownV2"),
            Self::Html => Some("HTML"),
        }
    }
}

impl fmt::Display for MessageFormat {
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
    let message = trim_required_message(message)?;

    let mut lines = page_prefix_lines(priority, context);
    lines.push(message.to_owned());

    let text = lines.join("\n");
    validate_char_limit(&text, TELEGRAM_TEXT_MESSAGE_LIMIT, "Telegram text message")?;
    Ok(text)
}

pub fn build_document_caption(
    message: Option<&str>,
    priority: Priority,
    context: &PageContext,
) -> Result<String> {
    let mut lines = page_prefix_lines(priority, context);

    if let Some(message) = trim_optional_message(message) {
        lines.push(message.to_owned());
    }

    let caption = lines.join("\n");
    validate_char_limit(
        &caption,
        TELEGRAM_DOCUMENT_CAPTION_LIMIT,
        "Telegram document caption",
    )?;
    Ok(caption)
}

fn page_prefix_lines(priority: Priority, context: &PageContext) -> Vec<String> {
    let mut lines = Vec::with_capacity(6);
    lines.push(priority.header().to_owned());
    lines.push(format!("host: {}", context.host));

    if let Some(cwd) = &context.cwd {
        lines.push(format!("cwd: {cwd}"));
    }

    if let Some(tmux) = &context.tmux {
        lines.push(format!("tmux: {tmux}"));
    }

    lines.push(format!("priority: {priority}"));
    lines
}

fn trim_required_message(message: &str) -> Result<&str> {
    let message = message.trim();
    if message.is_empty() {
        bail!("page message cannot be empty");
    }

    Ok(message)
}

fn trim_optional_message(message: Option<&str>) -> Option<&str> {
    message.map(str::trim).filter(|message| !message.is_empty())
}

fn validate_char_limit(input: &str, max_chars: usize, label: &str) -> Result<()> {
    let char_count = input.chars().count();
    if char_count > max_chars {
        bail!("{label} is {char_count} characters; Telegram limit is {max_chars} characters");
    }

    Ok(())
}

pub fn build_test_text(host: &str) -> String {
    format!("agent-pager test from {}", host.trim())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramDocument {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

impl TelegramDocument {
    pub fn new(file_name: impl Into<String>, bytes: Vec<u8>) -> Result<Self> {
        let file_name = file_name.into().trim().to_owned();
        if file_name.is_empty() {
            bail!("document file name cannot be empty");
        }

        if bytes.is_empty() {
            bail!("document cannot be empty");
        }

        Ok(Self { file_name, bytes })
    }
}

#[derive(Debug, Serialize)]
struct TelegramSendMessage<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'static str>,
}

pub async fn send_telegram_message(
    client: &reqwest::Client,
    config: &PagerConfig,
    text: &str,
) -> Result<()> {
    send_telegram_message_with_format(client, config, text, MessageFormat::Plain).await
}

pub async fn send_telegram_message_with_format(
    client: &reqwest::Client,
    config: &PagerConfig,
    text: &str,
    format: MessageFormat,
) -> Result<()> {
    validate_char_limit(text, TELEGRAM_TEXT_MESSAGE_LIMIT, "Telegram text message")?;

    let endpoint = telegram_endpoint(config, "sendMessage");
    let payload = TelegramSendMessage {
        chat_id: &config.chat_id,
        text,
        disable_web_page_preview: true,
        parse_mode: format.telegram_parse_mode(),
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

    ensure_telegram_success(response, config, "sendMessage").await
}

pub async fn send_telegram_document(
    client: &reqwest::Client,
    config: &PagerConfig,
    document: TelegramDocument,
    caption: Option<&str>,
    format: MessageFormat,
) -> Result<()> {
    let caption = trim_optional_message(caption);
    if let Some(caption) = caption {
        validate_char_limit(
            caption,
            TELEGRAM_DOCUMENT_CAPTION_LIMIT,
            "Telegram document caption",
        )?;
    }

    let endpoint = telegram_endpoint(config, "sendDocument");
    let TelegramDocument { file_name, bytes } = document;
    let document_part = Part::bytes(bytes).file_name(file_name);
    let mut form = Form::new()
        .text("chat_id", config.chat_id.clone())
        .part("document", document_part);

    if let Some(caption) = caption {
        form = form.text("caption", caption.to_owned());
        if let Some(parse_mode) = format.telegram_parse_mode() {
            form = form.text("parse_mode", parse_mode.to_owned());
        }
    }

    let response = client
        .post(endpoint)
        .multipart(form)
        .send()
        .await
        .map_err(|err| {
            anyhow!(
                "failed to call Telegram sendDocument: {}",
                redact_token(&err.to_string(), &config.bot_token)
            )
        })?;

    ensure_telegram_success(response, config, "sendDocument").await
}

fn telegram_endpoint(config: &PagerConfig, method: &str) -> String {
    format!(
        "{}{}/{}",
        TELEGRAM_SEND_MESSAGE_PREFIX, config.bot_token, method
    )
}

async fn ensure_telegram_success(
    response: reqwest::Response,
    config: &PagerConfig,
    method: &str,
) -> Result<()> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Telegram {method} failed with status {status}: {}",
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
            "🟡 Agent needs attention\nhost: desktop\ncwd: ~/src/walletd\ntmux: main\npriority: normal\nTests failed in descriptor parser."
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
            "🔴 Agent needs attention\nhost: buildbox\npriority: high\nNeed review"
        );
    }

    #[test]
    fn builds_document_caption_without_requiring_message() {
        let context = PageContext {
            host: "desktop".to_owned(),
            cwd: Some("~/src/walletd".to_owned()),
            tmux: None,
        };

        let caption =
            build_document_caption(None, Priority::Normal, &context).expect("valid caption");

        assert_eq!(
            caption,
            "🟡 Agent needs attention\nhost: desktop\ncwd: ~/src/walletd\npriority: normal"
        );
    }

    #[test]
    fn builds_document_caption_with_optional_message() {
        let context = PageContext {
            host: "desktop".to_owned(),
            cwd: None,
            tmux: Some("main".to_owned()),
        };

        let caption = build_document_caption(
            Some("  See attached markdown report.  "),
            Priority::High,
            &context,
        )
        .expect("valid caption");

        assert_eq!(
            caption,
            "🔴 Agent needs attention\nhost: desktop\ntmux: main\npriority: high\nSee attached markdown report."
        );
    }

    #[test]
    fn maps_message_formats_to_telegram_parse_modes() {
        assert_eq!(MessageFormat::Plain.telegram_parse_mode(), None);
        assert_eq!(
            MessageFormat::MarkdownV2.telegram_parse_mode(),
            Some("MarkdownV2")
        );
        assert_eq!(MessageFormat::Html.telegram_parse_mode(), Some("HTML"));
    }

    #[test]
    fn rejects_text_over_telegram_limits() {
        let error = validate_char_limit("abcd", 3, "Telegram text message").expect_err("too long");

        assert_eq!(
            error.to_string(),
            "Telegram text message is 4 characters; Telegram limit is 3 characters"
        );
    }

    #[test]
    fn rejects_invalid_document_inputs() {
        let error = TelegramDocument::new("   ", vec![1]).expect_err("empty file name");
        assert_eq!(error.to_string(), "document file name cannot be empty");

        let error = TelegramDocument::new("report.md", Vec::new()).expect_err("empty file");
        assert_eq!(error.to_string(), "document cannot be empty");
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
