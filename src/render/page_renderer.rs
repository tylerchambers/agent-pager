use crate::{
    AgentPagerError,
    domain::{HostName, Page, PageContext, Priority},
};

use super::TelegramLimits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRenderer {
    limits: TelegramLimits,
}

impl PageRenderer {
    pub fn new(limits: TelegramLimits) -> Self {
        Self { limits }
    }

    pub fn limits(&self) -> TelegramLimits {
        self.limits
    }

    pub fn render_page(&self, page: &Page) -> Result<String, AgentPagerError> {
        let text = self.render_page_unlimited(page);
        validate_char_limit(
            &text,
            self.limits.text_message_chars,
            "Telegram text message",
        )?;
        Ok(text)
    }

    pub fn render_page_unlimited(&self, page: &Page) -> String {
        let mut lines = prefix_lines(page.priority(), page.context());
        lines.push(page.body().as_str().to_owned());
        lines.join("\n")
    }

    pub fn render_document_caption(
        &self,
        message: Option<&str>,
        priority: Priority,
        context: &PageContext,
    ) -> Result<String, AgentPagerError> {
        let mut lines = prefix_lines(priority, context);
        if let Some(message) = trim_optional(message) {
            lines.push(message.to_owned());
        }
        let caption = lines.join("\n");
        validate_char_limit(
            &caption,
            self.limits.document_caption_chars,
            "Telegram document caption",
        )?;
        Ok(caption)
    }

    pub fn render_test_text(&self, host: &HostName) -> String {
        format!("agent-pager test from {}", host.as_str())
    }
}

impl Default for PageRenderer {
    fn default() -> Self {
        Self::new(TelegramLimits::default())
    }
}

fn prefix_lines(priority: Priority, context: &PageContext) -> Vec<String> {
    let mut lines = Vec::with_capacity(6);
    lines.push(match priority {
        Priority::Normal => "🟡 Agent needs attention".to_owned(),
        Priority::High => "🔴 Agent needs attention".to_owned(),
    });
    lines.push(format!("host: {}", context.host().as_str()));

    if let Some(cwd) = context.cwd() {
        lines.push(format!("cwd: {}", cwd.as_str()));
    }

    if let Some(tmux) = context.tmux() {
        lines.push(format!("tmux: {}", tmux.as_str()));
    }

    lines.push(format!("priority: {priority}"));
    lines
}

pub(crate) fn validate_char_limit(
    input: &str,
    limit: usize,
    label: &'static str,
) -> Result<(), AgentPagerError> {
    let actual = input.chars().count();
    if actual > limit {
        return Err(AgentPagerError::CharacterLimitExceeded {
            label,
            actual,
            limit,
        });
    }
    Ok(())
}

fn trim_optional(input: Option<&str>) -> Option<&str> {
    input.map(str::trim).filter(|value| !value.is_empty())
}
