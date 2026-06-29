use crate::{
    AgentPagerError,
    domain::{Document, DocumentFileName, MessageFormat, OutboundPayload, Page},
};

use super::{PageRenderer, TelegramLimits};

pub const AUTO_DOCUMENT_FILE_NAME: &str = "agent-pager-message.md";
pub const AUTO_DOCUMENT_CAPTION: &str = "Message attached as document";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadPlanKind {
    Text,
    ExplicitDocument,
    AutomaticDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadPlan {
    pub payload: OutboundPayload,
    pub kind: PayloadPlanKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadPlanner {
    limits: TelegramLimits,
    auto_document_file_name: DocumentFileName,
    auto_document_caption: String,
}

impl PayloadPlanner {
    pub fn new(
        limits: TelegramLimits,
        auto_document_file_name: DocumentFileName,
        auto_document_caption: impl Into<String>,
    ) -> Self {
        Self {
            limits,
            auto_document_file_name,
            auto_document_caption: auto_document_caption.into(),
        }
    }

    pub fn plan_page(
        &self,
        page: &Page,
        rendered_text: String,
        format: MessageFormat,
        file_name_override: Option<DocumentFileName>,
        renderer: &PageRenderer,
    ) -> Result<PayloadPlan, AgentPagerError> {
        if rendered_text.chars().count() <= self.limits.text_message_chars {
            return Ok(PayloadPlan {
                payload: OutboundPayload::Text {
                    text: rendered_text,
                    format,
                },
                kind: PayloadPlanKind::Text,
            });
        }

        let file_name = file_name_override.unwrap_or_else(|| self.auto_document_file_name.clone());
        let document = Document::new(file_name, page.body().as_bytes().to_vec())?;
        let caption = renderer.render_document_caption(
            Some(&self.auto_document_caption),
            page.priority(),
            page.context(),
        )?;

        Ok(PayloadPlan {
            payload: OutboundPayload::Document {
                document,
                caption: Some(caption),
                format,
            },
            kind: PayloadPlanKind::AutomaticDocument,
        })
    }

    pub fn plan_explicit_document(
        &self,
        document: Document,
        caption: String,
        format: MessageFormat,
    ) -> PayloadPlan {
        PayloadPlan {
            payload: OutboundPayload::Document {
                document,
                caption: Some(caption),
                format,
            },
            kind: PayloadPlanKind::ExplicitDocument,
        }
    }
}

impl Default for PayloadPlanner {
    fn default() -> Self {
        Self::new(
            TelegramLimits::default(),
            DocumentFileName::new(AUTO_DOCUMENT_FILE_NAME)
                .expect("default auto document file name is valid"),
            AUTO_DOCUMENT_CAPTION,
        )
    }
}
