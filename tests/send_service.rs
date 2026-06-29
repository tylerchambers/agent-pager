use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use agent_pager::{
    AgentPagerError,
    app::{SendOutcome, SendPageService},
    command::{ContextOptions, DocumentSource, MessageSource, SendPageCommand, SensitivityMode},
    domain::{
        BotToken, ChatId, DisplayPath, Document, DocumentFileName, HostName, MessageFormat,
        OutboundPayload, PageContext, PagerConfig, Priority, TelegramConfig, TmuxSession,
    },
    ports::{ContextProvider, DocumentStore, MessageReader, PagerGateway, SensitiveContentScanner},
    render::{PageRenderer, PayloadPlanner, TelegramLimits},
    security::SensitiveReason,
};
use async_trait::async_trait;

#[tokio::test]
async fn sends_inline_text_payload() {
    let gateway = FakeGateway::default();
    let service = service_with(gateway.clone(), FakeScanner::allowing(), default_limits());

    let outcome = service
        .send(
            &command(MessageSource::Inline("hello operator".to_owned())),
            &config(),
        )
        .await
        .expect("inline text should send");

    assert_eq!(outcome, SendOutcome::SentText);
    let payloads = gateway.payloads();
    assert_eq!(payloads.len(), 1);
    match &payloads[0] {
        OutboundPayload::Text { text, format } => {
            assert_eq!(*format, MessageFormat::Plain);
            assert!(text.contains("🟡 Agent needs attention"));
            assert!(text.contains("host: configured-host"));
            assert!(text.contains("hello operator"));
        }
        other => panic!("expected text payload, got {other:?}"),
    }
}

#[tokio::test]
async fn reads_message_from_stdin_source() {
    let gateway = FakeGateway::default();
    let reader = FakeMessageReader::with_text("stdin body");
    let service = SendPageService::new(
        FakeContextProvider::default(),
        reader.clone(),
        FakeDocumentStore::default(),
        FakeScanner::allowing(),
        gateway.clone(),
        PageRenderer::default(),
        PayloadPlanner::default(),
    );

    let outcome = service
        .send(&command(MessageSource::Stdin), &config())
        .await
        .expect("stdin message should send");

    assert_eq!(outcome, SendOutcome::SentText);
    assert_eq!(reader.text_reads(), 1);
    match &gateway.payloads()[0] {
        OutboundPayload::Text { text, .. } => assert!(text.contains("stdin body")),
        other => panic!("expected text payload, got {other:?}"),
    }
}

#[tokio::test]
async fn sends_explicit_document_with_message_caption() {
    let gateway = FakeGateway::default();
    let document_store = FakeDocumentStore::with_document(
        Document::new(doc_name("report.txt"), b"document bytes".to_vec())
            .expect("document is valid"),
    );
    let service = SendPageService::new(
        FakeContextProvider::default(),
        FakeMessageReader::default(),
        document_store.clone(),
        FakeScanner::allowing(),
        gateway.clone(),
        PageRenderer::default(),
        PayloadPlanner::default(),
    );
    let mut send = command(MessageSource::Inline("caption text".to_owned()));
    send.document_source = Some(DocumentSource::Path(PathBuf::from("report.txt")));
    send.format = MessageFormat::Html;

    let outcome = service
        .send(&send, &config())
        .await
        .expect("document should send");

    assert_eq!(outcome, SendOutcome::SentDocument);
    assert_eq!(document_store.reads().len(), 1);
    match &gateway.payloads()[0] {
        OutboundPayload::Document {
            document,
            caption,
            format,
        } => {
            assert_eq!(*format, MessageFormat::Html);
            assert_eq!(document.file_name().as_str(), "report.txt");
            assert_eq!(document.bytes(), b"document bytes");
            let caption = caption.as_ref().expect("explicit documents are captioned");
            assert!(caption.contains("caption text"));
            assert!(caption.contains("host: configured-host"));
        }
        other => panic!("expected document payload, got {other:?}"),
    }
}

#[tokio::test]
async fn sends_explicit_document_without_message_with_context_caption() {
    let gateway = FakeGateway::default();
    let service = service_with(gateway.clone(), FakeScanner::allowing(), default_limits());
    let mut send = command(MessageSource::None);
    send.document_source = Some(DocumentSource::Path(PathBuf::from("only-document.txt")));
    send.priority = Priority::High;

    let outcome = service
        .send(&send, &config())
        .await
        .expect("document without message should send");

    assert_eq!(outcome, SendOutcome::SentDocument);
    match &gateway.payloads()[0] {
        OutboundPayload::Document { caption, .. } => {
            let caption = caption.as_ref().expect("context caption is present");
            assert!(caption.contains("🔴 Agent needs attention"));
            assert!(caption.contains("host: configured-host"));
            assert!(caption.contains("priority: high"));
            assert!(!caption.contains("only-document"));
        }
        other => panic!("expected document payload, got {other:?}"),
    }
}

#[tokio::test]
async fn sends_oversized_message_as_automatic_document() {
    let gateway = FakeGateway::default();
    let limits = TelegramLimits {
        text_message_chars: 8,
        document_caption_chars: 1024,
    };
    let service = SendPageService::new(
        FakeContextProvider::default(),
        FakeMessageReader::default(),
        FakeDocumentStore::default(),
        FakeScanner::allowing(),
        gateway.clone(),
        PageRenderer::new(limits),
        PayloadPlanner::new(limits, doc_name("auto-message.md"), "attached body"),
    );

    let outcome = service
        .send(
            &command(MessageSource::Inline("body too large".to_owned())),
            &config(),
        )
        .await
        .expect("oversized text should become a document");

    assert_eq!(outcome, SendOutcome::SentTextAsDocument);
    match &gateway.payloads()[0] {
        OutboundPayload::Document {
            document, caption, ..
        } => {
            assert_eq!(document.file_name().as_str(), "auto-message.md");
            assert_eq!(document.bytes(), b"body too large");
            assert!(caption.as_ref().expect("caption").contains("attached body"));
        }
        other => panic!("expected automatic document payload, got {other:?}"),
    }
}

#[tokio::test]
async fn sensitive_message_preflight_prevents_gateway_call() {
    let gateway = FakeGateway::default();
    let scanner = FakeScanner::rejecting();
    let service = service_with(gateway.clone(), scanner.clone(), default_limits());

    let error = service
        .send(
            &command(MessageSource::Inline("SECRET=value".to_owned())),
            &config(),
        )
        .await
        .expect_err("sensitive message should fail preflight");

    assert!(matches!(error, AgentPagerError::SensitivePayload { .. }));
    assert_eq!(scanner.text_scans().len(), 1);
    assert!(gateway.payloads().is_empty());
}

#[tokio::test]
async fn allow_sensitive_skips_scanner_and_permits_gateway_call() {
    let gateway = FakeGateway::default();
    let scanner = FakeScanner::rejecting();
    let service = service_with(gateway.clone(), scanner.clone(), default_limits());
    let mut send = command(MessageSource::Inline("SECRET=value".to_owned()));
    send.sensitivity_mode = SensitivityMode::AllowSensitive;

    let outcome = service
        .send(&send, &config())
        .await
        .expect("allow-sensitive should bypass scanner");

    assert_eq!(outcome, SendOutcome::SentText);
    assert!(scanner.text_scans().is_empty());
    assert_eq!(gateway.payloads().len(), 1);
}

#[tokio::test]
async fn forwards_context_options_and_configured_default_host() {
    let gateway = FakeGateway::default();
    let context_provider = FakeContextProvider::default();
    let service = SendPageService::new(
        context_provider.clone(),
        FakeMessageReader::default(),
        FakeDocumentStore::default(),
        FakeScanner::allowing(),
        gateway,
        PageRenderer::default(),
        PayloadPlanner::default(),
    );
    let mut send = command(MessageSource::Inline("context please".to_owned()));
    send.context_options = ContextOptions {
        include_cwd: true,
        include_tmux: true,
    };

    service
        .send(&send, &config())
        .await
        .expect("message should send");

    let calls = context_provider.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.as_str(), "configured-host");
    assert_eq!(calls[0].1, send.context_options);
}

fn service_with(
    gateway: FakeGateway,
    scanner: FakeScanner,
    limits: TelegramLimits,
) -> SendPageService<
    FakeContextProvider,
    FakeMessageReader,
    FakeDocumentStore,
    FakeScanner,
    FakeGateway,
> {
    SendPageService::new(
        FakeContextProvider::default(),
        FakeMessageReader::default(),
        FakeDocumentStore::default(),
        scanner,
        gateway,
        PageRenderer::new(limits),
        PayloadPlanner::new(
            limits,
            doc_name("agent-pager-message.md"),
            "Message attached as document",
        ),
    )
}

fn command(message_source: MessageSource) -> SendPageCommand {
    SendPageCommand {
        message_source,
        document_source: None,
        document_name: None,
        priority: Priority::Normal,
        format: MessageFormat::Plain,
        context_options: ContextOptions::default(),
        sensitivity_mode: SensitivityMode::Preflight,
    }
}

fn config() -> PagerConfig {
    PagerConfig::new(
        TelegramConfig::new(
            BotToken::new("123456:ABCDEF").expect("token"),
            ChatId::new("424242").expect("chat id"),
        ),
        host("configured-host"),
    )
}

fn default_limits() -> TelegramLimits {
    TelegramLimits::default()
}

fn host(value: &str) -> HostName {
    HostName::new(value).expect("host")
}

fn doc_name(value: &str) -> DocumentFileName {
    DocumentFileName::new(value).expect("document file name")
}

#[derive(Clone)]
struct FakeContextProvider {
    calls: Arc<Mutex<Vec<(HostName, ContextOptions)>>>,
}

impl Default for FakeContextProvider {
    fn default() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl FakeContextProvider {
    fn calls(&self) -> Vec<(HostName, ContextOptions)> {
        self.calls.lock().expect("context calls").clone()
    }
}

impl ContextProvider for FakeContextProvider {
    fn gather(
        &self,
        default_host: HostName,
        options: ContextOptions,
    ) -> Result<PageContext, AgentPagerError> {
        self.calls
            .lock()
            .expect("context calls")
            .push((default_host.clone(), options));
        Ok(PageContext::new(
            default_host,
            options
                .include_cwd
                .then(|| DisplayPath::new("~/repo").expect("cwd")),
            options
                .include_tmux
                .then(|| TmuxSession::new("agent-session").expect("tmux")),
        ))
    }
}

#[derive(Clone)]
struct FakeMessageReader {
    text: Arc<String>,
    text_reads: Arc<Mutex<usize>>,
}

impl Default for FakeMessageReader {
    fn default() -> Self {
        Self::with_text("stdin text")
    }
}

impl FakeMessageReader {
    fn with_text(text: impl Into<String>) -> Self {
        Self {
            text: Arc::new(text.into()),
            text_reads: Arc::new(Mutex::new(0)),
        }
    }

    fn text_reads(&self) -> usize {
        *self.text_reads.lock().expect("text reads")
    }
}

impl MessageReader for FakeMessageReader {
    fn read_text_stdin(&self) -> Result<String, AgentPagerError> {
        *self.text_reads.lock().expect("text reads") += 1;
        Ok((*self.text).clone())
    }

    fn read_document_stdin(&self) -> Result<Vec<u8>, AgentPagerError> {
        Ok(b"stdin document".to_vec())
    }
}

type DocumentReadLog = Arc<Mutex<Vec<(DocumentSource, Option<DocumentFileName>)>>>;
type TextScanLog = Arc<Mutex<Vec<(String, String)>>>;
type ByteScanLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone)]
struct FakeDocumentStore {
    document: Arc<Document>,
    reads: DocumentReadLog,
}

impl Default for FakeDocumentStore {
    fn default() -> Self {
        Self::with_document(
            Document::new(doc_name("document.txt"), b"document body".to_vec()).expect("document"),
        )
    }
}

impl FakeDocumentStore {
    fn with_document(document: Document) -> Self {
        Self {
            document: Arc::new(document),
            reads: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn reads(&self) -> Vec<(DocumentSource, Option<DocumentFileName>)> {
        self.reads.lock().expect("document reads").clone()
    }
}

impl DocumentStore for FakeDocumentStore {
    fn read_document(
        &self,
        source: &DocumentSource,
        explicit_name: Option<DocumentFileName>,
    ) -> Result<Document, AgentPagerError> {
        self.reads
            .lock()
            .expect("document reads")
            .push((source.clone(), explicit_name));
        Ok((*self.document).clone())
    }
}

#[derive(Clone, Default)]
struct FakeScanner {
    reject: bool,
    text_scans: TextScanLog,
    byte_scans: ByteScanLog,
}

impl FakeScanner {
    fn allowing() -> Self {
        Self::default()
    }

    fn rejecting() -> Self {
        Self {
            reject: true,
            ..Self::default()
        }
    }

    fn text_scans(&self) -> Vec<(String, String)> {
        self.text_scans.lock().expect("text scans").clone()
    }
}

impl SensitiveContentScanner for FakeScanner {
    fn scan_text(&self, label: &str, text: &str) -> Result<(), AgentPagerError> {
        self.text_scans
            .lock()
            .expect("text scans")
            .push((label.to_owned(), text.to_owned()));
        if self.reject {
            return Err(AgentPagerError::SensitivePayload {
                label: label.to_owned(),
                reason: SensitiveReason::SecretLookingAssignment,
            });
        }
        Ok(())
    }

    fn scan_bytes(&self, label: &str, bytes: &[u8]) -> Result<(), AgentPagerError> {
        self.byte_scans
            .lock()
            .expect("byte scans")
            .push((label.to_owned(), bytes.to_vec()));
        if self.reject {
            return Err(AgentPagerError::SensitivePayload {
                label: label.to_owned(),
                reason: SensitiveReason::SecretLookingAssignment,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeGateway {
    payloads: Arc<Mutex<Vec<OutboundPayload>>>,
}

impl FakeGateway {
    fn payloads(&self) -> Vec<OutboundPayload> {
        self.payloads.lock().expect("payloads").clone()
    }
}

#[async_trait]
impl PagerGateway for FakeGateway {
    async fn send(&self, payload: OutboundPayload) -> Result<(), AgentPagerError> {
        self.payloads.lock().expect("payloads").push(payload);
        Ok(())
    }
}
