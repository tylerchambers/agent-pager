use async_trait::async_trait;
use reqwest::{
    Client, Url,
    multipart::{Form, Part},
};
use serde::Serialize;

use crate::{
    AgentPagerError,
    domain::{Document, MessageFormat, OutboundPayload, TelegramConfig},
    ports::PagerGateway,
    render::{TelegramLimits, validate_char_limit},
};

const DEFAULT_TELEGRAM_BASE_URL: &str = "https://api.telegram.org";
const ERROR_BODY_LIMIT_CHARS: usize = 512;

#[derive(Debug, Clone)]
pub struct TelegramPagerGateway {
    http: Client,
    config: TelegramConfig,
    base_url: Url,
    limits: TelegramLimits,
}

impl TelegramPagerGateway {
    pub fn new(config: TelegramConfig) -> Result<Self, AgentPagerError> {
        let base_url = Url::parse(DEFAULT_TELEGRAM_BASE_URL)
            .map_err(|error| AgentPagerError::Other(error.to_string()))?;
        Ok(Self::with_base_url(Client::new(), config, base_url))
    }

    pub fn with_base_url(http: Client, config: TelegramConfig, base_url: Url) -> Self {
        Self {
            http,
            config,
            base_url,
            limits: TelegramLimits::default(),
        }
    }

    async fn send_message(&self, text: &str, format: MessageFormat) -> Result<(), AgentPagerError> {
        validate_char_limit(
            text,
            self.limits.text_message_chars,
            "Telegram text message",
        )?;
        let endpoint = self.endpoint("sendMessage")?;
        let payload = TelegramSendMessage {
            chat_id: self.config.chat_id().as_str(),
            text,
            disable_web_page_preview: true,
            parse_mode: telegram_parse_mode(format),
        };
        let response = self
            .http
            .post(endpoint)
            .json(&payload)
            .send()
            .await
            .map_err(|error| self.transport_error("sendMessage", error))?;
        self.ensure_success(response, "sendMessage").await
    }

    async fn send_document(
        &self,
        document: Document,
        caption: Option<&str>,
        format: MessageFormat,
    ) -> Result<(), AgentPagerError> {
        if let Some(caption) = caption.map(str::trim).filter(|value| !value.is_empty()) {
            validate_char_limit(
                caption,
                self.limits.document_caption_chars,
                "Telegram document caption",
            )?;
        }

        let endpoint = self.endpoint("sendDocument")?;
        let (file_name, bytes) = document.into_parts();
        let document_part = Part::bytes(bytes).file_name(file_name.into_string());
        let mut form = Form::new()
            .text("chat_id", self.config.chat_id().as_str().to_owned())
            .part("document", document_part);

        if let Some(caption) = caption.map(str::trim).filter(|value| !value.is_empty()) {
            form = form.text("caption", caption.to_owned());
            if let Some(parse_mode) = telegram_parse_mode(format) {
                form = form.text("parse_mode", parse_mode.to_owned());
            }
        }

        let response = self
            .http
            .post(endpoint)
            .multipart(form)
            .send()
            .await
            .map_err(|error| self.transport_error("sendDocument", error))?;
        self.ensure_success(response, "sendDocument").await
    }

    fn endpoint(&self, method: &'static str) -> Result<Url, AgentPagerError> {
        let mut url = self.base_url.clone();
        {
            let mut segments = url.path_segments_mut().map_err(|_| {
                AgentPagerError::Other("Telegram base URL cannot be a base".to_owned())
            })?;
            segments.pop_if_empty();
            let bot_segment = format!("bot{}", self.config.bot_token().expose_secret());
            segments.push(&bot_segment);
            segments.push(method);
        }
        Ok(url)
    }

    async fn ensure_success(
        &self,
        response: reqwest::Response,
        method: &'static str,
    ) -> Result<(), AgentPagerError> {
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(AgentPagerError::TelegramStatus {
            method,
            status,
            body: truncate(
                &redact_token(&body, self.config.bot_token().expose_secret()),
                ERROR_BODY_LIMIT_CHARS,
            ),
        })
    }

    fn transport_error(&self, method: &'static str, error: reqwest::Error) -> AgentPagerError {
        AgentPagerError::TelegramTransport {
            method,
            message: redact_token(&error.to_string(), self.config.bot_token().expose_secret()),
        }
    }
}

#[async_trait]
impl PagerGateway for TelegramPagerGateway {
    async fn send(&self, payload: OutboundPayload) -> Result<(), AgentPagerError> {
        match payload {
            OutboundPayload::Text { text, format } => self.send_message(&text, format).await,
            OutboundPayload::Document {
                document,
                caption,
                format,
            } => {
                self.send_document(document, caption.as_deref(), format)
                    .await
            }
        }
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

pub fn telegram_parse_mode(format: MessageFormat) -> Option<&'static str> {
    match format {
        MessageFormat::Plain => None,
        MessageFormat::MarkdownV2 => Some("MarkdownV2"),
        MessageFormat::Html => Some("HTML"),
    }
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
