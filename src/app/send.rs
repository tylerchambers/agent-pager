use crate::{
    AgentPagerError,
    command::{MessageSource, SendPageCommand, SensitivityMode},
    domain::{MessageBody, Page, PagerConfig},
    ports::{ContextProvider, DocumentStore, MessageReader, PagerGateway, SensitiveContentScanner},
    render::{PageRenderer, PayloadPlanKind, PayloadPlanner},
};

use super::SendOutcome;

#[derive(Debug, Clone)]
pub struct SendPageService<C, R, D, S, G> {
    context_provider: C,
    message_reader: R,
    document_store: D,
    scanner: S,
    gateway: G,
    renderer: PageRenderer,
    planner: PayloadPlanner,
}

impl<C, R, D, S, G> SendPageService<C, R, D, S, G> {
    pub fn new(
        context_provider: C,
        message_reader: R,
        document_store: D,
        scanner: S,
        gateway: G,
        renderer: PageRenderer,
        planner: PayloadPlanner,
    ) -> Self {
        Self {
            context_provider,
            message_reader,
            document_store,
            scanner,
            gateway,
            renderer,
            planner,
        }
    }
}

impl<C, R, D, S, G> SendPageService<C, R, D, S, G>
where
    C: ContextProvider,
    R: MessageReader,
    D: DocumentStore,
    S: SensitiveContentScanner,
    G: PagerGateway,
{
    pub async fn send(
        &self,
        command: &SendPageCommand,
        config: &PagerConfig,
    ) -> Result<SendOutcome, AgentPagerError> {
        let context = self
            .context_provider
            .gather(config.default_host().clone(), command.context_options)?;
        let message = self.resolve_message(&command.message_source)?;

        if let Some(document_source) = &command.document_source {
            let document = self
                .document_store
                .read_document(document_source, command.document_name.clone())?;
            self.scan_if_needed(
                command.sensitivity_mode,
                message.as_deref(),
                Some(document.bytes()),
            )?;
            let caption = self.renderer.render_document_caption(
                message.as_deref(),
                command.priority,
                &context,
            )?;
            let plan = self
                .planner
                .plan_explicit_document(document, caption, command.format);
            self.gateway.send(plan.payload).await?;
            return Ok(SendOutcome::SentDocument);
        }

        let raw_message = message.as_deref().ok_or_else(|| {
            AgentPagerError::InvalidCommand(
                "provide a message, --stdin, or --document <path>".to_owned(),
            )
        })?;
        self.scan_if_needed(command.sensitivity_mode, Some(raw_message), None)?;
        let body = MessageBody::new(raw_message)?;
        let page = Page::new(body, command.priority, context);
        let rendered_text = self.renderer.render_page_unlimited(&page);
        let plan = self.planner.plan_page(
            &page,
            rendered_text,
            command.format,
            command.document_name.clone(),
            &self.renderer,
        )?;
        let outcome = match plan.kind {
            PayloadPlanKind::Text => SendOutcome::SentText,
            PayloadPlanKind::AutomaticDocument => SendOutcome::SentTextAsDocument,
            PayloadPlanKind::ExplicitDocument => SendOutcome::SentDocument,
        };
        self.gateway.send(plan.payload).await?;
        Ok(outcome)
    }

    fn resolve_message(&self, source: &MessageSource) -> Result<Option<String>, AgentPagerError> {
        match source {
            MessageSource::Inline(message) => Ok(Some(message.clone())),
            MessageSource::Stdin => self.message_reader.read_text_stdin().map(Some),
            MessageSource::None => Ok(None),
        }
    }

    fn scan_if_needed(
        &self,
        mode: SensitivityMode,
        message: Option<&str>,
        document: Option<&[u8]>,
    ) -> Result<(), AgentPagerError> {
        if mode == SensitivityMode::AllowSensitive {
            return Ok(());
        }

        if let Some(message) = message {
            self.scanner.scan_text("message", message)?;
        }
        if let Some(document) = document {
            self.scanner.scan_bytes("document", document)?;
        }
        Ok(())
    }
}
