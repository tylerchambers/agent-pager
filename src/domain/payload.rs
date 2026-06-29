use std::fmt;

use super::{Document, MessageBody, MessageFormat, PageContext, Priority};

#[derive(Clone, PartialEq, Eq)]
pub struct Page {
    body: MessageBody,
    priority: Priority,
    context: PageContext,
}

impl Page {
    pub fn new(body: MessageBody, priority: Priority, context: PageContext) -> Self {
        Self {
            body,
            priority,
            context,
        }
    }

    pub fn body(&self) -> &MessageBody {
        &self.body
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn context(&self) -> &PageContext {
        &self.context
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("body", &self.body)
            .field("priority", &self.priority)
            .field("context", &self.context)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum OutboundPayload {
    Text {
        text: String,
        format: MessageFormat,
    },
    Document {
        document: Document,
        caption: Option<String>,
        format: MessageFormat,
    },
}

impl fmt::Debug for OutboundPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text { text, format } => f
                .debug_struct("Text")
                .field("text_chars", &text.chars().count())
                .field("format", format)
                .finish(),
            Self::Document {
                document,
                caption,
                format,
            } => f
                .debug_struct("Document")
                .field("document", document)
                .field(
                    "caption_chars",
                    &caption.as_ref().map(|value| value.chars().count()),
                )
                .field("format", format)
                .finish(),
        }
    }
}
