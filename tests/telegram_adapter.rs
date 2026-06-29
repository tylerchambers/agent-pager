use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use agent_pager::{
    AgentPagerError,
    adapters::TelegramPagerGateway,
    domain::{
        BotToken, ChatId, Document, DocumentFileName, MessageFormat, OutboundPayload,
        TelegramConfig,
    },
    ports::PagerGateway,
};
use reqwest::{Client, StatusCode, Url};
use serde_json::Value;

const BOT_TOKEN: &str = "123456:ABC-secret-token";
const CHAT_ID: &str = "-1004242";

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn start_server(status: StatusCode, body: impl Into<String>) -> (Url, Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock Telegram listener");
    let addr = listener
        .local_addr()
        .expect("mock Telegram listener address");
    let (tx, rx) = mpsc::channel();
    let response_body = body.into();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept Telegram request");
        let request = read_request(&mut stream);
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
            status.as_u16(),
            status.canonical_reason().unwrap_or(""),
            response_body.len(),
            response_body,
        );
        stream
            .write_all(response.as_bytes())
            .expect("write mock Telegram response");
        tx.send(request).expect("send captured Telegram request");
    });

    (
        Url::parse(&format!("http://{addr}")).expect("mock Telegram base URL"),
        rx,
    )
}

fn read_request(stream: &mut TcpStream) -> CapturedRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set request read timeout");

    let mut received = Vec::new();
    let header_end = loop {
        let mut byte = [0_u8; 1];
        stream
            .read_exact(&mut byte)
            .expect("read request header byte");
        received.push(byte[0]);
        if let Some(position) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let headers_text =
        String::from_utf8(received[..header_end].to_vec()).expect("request headers are utf8");
    let mut lines = headers_text.split("\r\n");
    let request_line = lines.next().expect("request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().expect("request method").to_owned();
    let path = request_parts.next().expect("request path").to_owned();

    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').expect("request header delimiter");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }

    let mut body = received[header_end..].to_vec();
    if let Some(length) = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().expect("content-length is usize"))
    {
        let remaining = length
            .checked_sub(body.len())
            .expect("body is not over-read");
        let mut rest = vec![0_u8; remaining];
        stream.read_exact(&mut rest).expect("read request body");
        body.extend(rest);
    } else if headers
        .get("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        body = read_chunked_body(stream, body);
    }

    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn read_chunked_body(stream: &mut TcpStream, mut buffered: Vec<u8>) -> Vec<u8> {
    let mut decoded = Vec::new();
    loop {
        let line = read_crlf_line(stream, &mut buffered);
        let size_hex = line.split(';').next().expect("chunk size").trim();
        let size = usize::from_str_radix(size_hex, 16).expect("chunk size is hex");
        if size == 0 {
            let _ = read_crlf_line(stream, &mut buffered);
            return decoded;
        }

        while buffered.len() < size + 2 {
            let mut chunk = vec![0_u8; size + 2 - buffered.len()];
            stream.read_exact(&mut chunk).expect("read chunk bytes");
            buffered.extend(chunk);
        }
        decoded.extend_from_slice(&buffered[..size]);
        buffered.drain(..size + 2);
    }
}

fn read_crlf_line(stream: &mut TcpStream, buffered: &mut Vec<u8>) -> String {
    loop {
        if let Some(position) = buffered.windows(2).position(|window| window == b"\r\n") {
            let line =
                String::from_utf8(buffered[..position].to_vec()).expect("chunk line is utf8");
            buffered.drain(..position + 2);
            return line;
        }
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).expect("read chunk line byte");
        buffered.push(byte[0]);
    }
}

fn gateway(base_url: Url) -> TelegramPagerGateway {
    TelegramPagerGateway::with_base_url(Client::new(), telegram_config(), base_url)
}

fn telegram_config() -> TelegramConfig {
    TelegramConfig::new(
        BotToken::new(BOT_TOKEN).expect("valid bot token"),
        ChatId::new(CHAT_ID).expect("valid chat id"),
    )
}

async fn send_text(format: MessageFormat) -> CapturedRequest {
    let (base_url, requests) = start_server(StatusCode::OK, r#"{"ok":true}"#);
    gateway(base_url)
        .send(OutboundPayload::Text {
            text: "hello from tests".to_owned(),
            format,
        })
        .await
        .expect("send text payload");
    requests
        .recv_timeout(Duration::from_secs(5))
        .expect("captured text request")
}

#[tokio::test]
async fn text_message_posts_expected_json_and_omits_plain_parse_mode() {
    let request = send_text(MessageFormat::Plain).await;

    assert_eq!(request.method, "POST");
    assert_eq!(request.path, format!("/bot{BOT_TOKEN}/sendMessage"));
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );

    let body: Value = serde_json::from_slice(&request.body).expect("JSON sendMessage body");
    assert_eq!(body["chat_id"], CHAT_ID);
    assert_eq!(body["text"], "hello from tests");
    assert_eq!(body["disable_web_page_preview"], true);
    assert!(body.get("parse_mode").is_none());
}

#[tokio::test]
async fn text_message_uses_html_parse_mode() {
    let request = send_text(MessageFormat::Html).await;
    let body: Value = serde_json::from_slice(&request.body).expect("JSON sendMessage body");

    assert_eq!(request.path, format!("/bot{BOT_TOKEN}/sendMessage"));
    assert_eq!(body["parse_mode"], "HTML");
}

#[tokio::test]
async fn text_message_uses_markdown_v2_parse_mode() {
    let request = send_text(MessageFormat::MarkdownV2).await;
    let body: Value = serde_json::from_slice(&request.body).expect("JSON sendMessage body");

    assert_eq!(request.path, format!("/bot{BOT_TOKEN}/sendMessage"));
    assert_eq!(body["parse_mode"], "MarkdownV2");
}

#[tokio::test]
async fn document_upload_posts_multipart_with_file_caption_and_parse_mode() {
    let (base_url, requests) = start_server(StatusCode::OK, r#"{"ok":true}"#);
    let document = Document::new(
        DocumentFileName::new("report.txt").expect("valid document name"),
        b"document bytes from test".to_vec(),
    )
    .expect("valid document");

    gateway(base_url)
        .send(OutboundPayload::Document {
            document,
            caption: Some("<b>caption</b>".to_owned()),
            format: MessageFormat::Html,
        })
        .await
        .expect("send document payload");

    let request = requests
        .recv_timeout(Duration::from_secs(5))
        .expect("captured document request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, format!("/bot{BOT_TOKEN}/sendDocument"));
    assert!(
        request
            .headers
            .get("content-type")
            .is_some_and(|value| value.starts_with("multipart/form-data; boundary=")),
        "document request should be multipart: {:?}",
        request.headers.get("content-type")
    );

    let multipart =
        String::from_utf8(request.body).expect("multipart body is utf8 in test payload");
    assert!(multipart.contains("name=\"chat_id\""));
    assert!(multipart.contains(CHAT_ID));
    assert!(multipart.contains("name=\"document\"; filename=\"report.txt\""));
    assert!(multipart.contains("document bytes from test"));
    assert!(multipart.contains("name=\"caption\""));
    assert!(multipart.contains("<b>caption</b>"));
    assert!(multipart.contains("name=\"parse_mode\""));
    assert!(multipart.contains("HTML"));
}

#[tokio::test]
async fn non_success_status_is_structured_truncated_and_redacts_bot_token() {
    let long_error_body = format!("telegram rejected token {BOT_TOKEN}: {}", "x".repeat(900));
    let (base_url, requests) = start_server(StatusCode::BAD_REQUEST, long_error_body);

    let error = gateway(base_url)
        .send(OutboundPayload::Text {
            text: "will fail".to_owned(),
            format: MessageFormat::Plain,
        })
        .await
        .expect_err("non-2xx Telegram response should fail");

    let request = requests
        .recv_timeout(Duration::from_secs(5))
        .expect("captured failed text request");
    assert_eq!(request.path, format!("/bot{BOT_TOKEN}/sendMessage"));
    let rendered_error = error.to_string();
    assert!(rendered_error.contains("Telegram sendMessage failed with status 400 Bad Request"));
    assert!(rendered_error.contains("<redacted-bot-token>"));
    assert!(!rendered_error.contains(BOT_TOKEN));

    match error {
        AgentPagerError::TelegramStatus {
            method,
            status,
            body,
        } => {
            assert_eq!(method, "sendMessage");
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert!(body.ends_with("..."));
            assert!(body.chars().count() <= 515);
            assert!(body.contains("<redacted-bot-token>"));
            assert!(!body.contains(BOT_TOKEN));
        }
        other => panic!("expected TelegramStatus error, got {other:?}"),
    }
}

#[test]
fn default_telegram_gateway_constructor_uses_redacted_config() {
    let gateway = TelegramPagerGateway::new(telegram_config()).expect("default gateway");

    let debug = format!("{gateway:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains(BOT_TOKEN));
}

#[tokio::test]
async fn text_message_limit_is_checked_before_transport() {
    let error = gateway(Url::parse("http://127.0.0.1:9").expect("url"))
        .send(OutboundPayload::Text {
            text: "x".repeat(4097),
            format: MessageFormat::Plain,
        })
        .await
        .expect_err("limit error");

    assert!(matches!(
        error,
        AgentPagerError::CharacterLimitExceeded {
            label: "Telegram text message",
            actual: 4097,
            limit: 4096,
        }
    ));
}

#[tokio::test]
async fn document_caption_limit_is_checked_before_transport() {
    let document = Document::new(
        DocumentFileName::new("report.txt").expect("name"),
        b"body".to_vec(),
    )
    .expect("document");

    let error = gateway(Url::parse("http://127.0.0.1:9").expect("url"))
        .send(OutboundPayload::Document {
            document,
            caption: Some("x".repeat(1025)),
            format: MessageFormat::Plain,
        })
        .await
        .expect_err("limit error");

    assert!(matches!(
        error,
        AgentPagerError::CharacterLimitExceeded {
            label: "Telegram document caption",
            actual: 1025,
            limit: 1024,
        }
    ));
}

#[tokio::test]
async fn transport_errors_redact_bot_token() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unused port");
    let base_url =
        Url::parse(&format!("http://{}", listener.local_addr().expect("addr"))).expect("url");
    drop(listener);

    let error = gateway(base_url)
        .send(OutboundPayload::Text {
            text: "transport failure".to_owned(),
            format: MessageFormat::Plain,
        })
        .await
        .expect_err("transport error");

    let rendered = error.to_string();
    assert!(!rendered.contains(BOT_TOKEN));
    if rendered.contains("<redacted-bot-token>") {
        assert!(rendered.contains("sendMessage"));
    }
    assert!(matches!(
        error,
        AgentPagerError::TelegramTransport {
            method: "sendMessage",
            ..
        }
    ));
}
