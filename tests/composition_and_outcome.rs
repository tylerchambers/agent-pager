use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc,
    thread,
};

use agent_pager::{
    app::{AppOutcome, DoctorReport, InstallSkillOutcome, SendOutcome, TestOutcome},
    command::{
        AppCommand, ContextOptions, InstallSkillCommand, MessageSource, SendPageCommand,
        SensitivityMode,
    },
    composition::App,
    domain::{HostName, MessageFormat, Priority},
    ports::ConfigSource,
};
use reqwest::Url;

const TOKEN: &str = "123456:abcdefghijklmnopqrstuvwxyz";
const CHAT_ID: &str = "987654321";

#[derive(Debug, Clone)]
struct FakeConfig(HashMap<&'static str, String>);

impl FakeConfig {
    fn valid() -> Self {
        Self(HashMap::from([
            ("AGENT_PAGER_TELEGRAM_BOT_TOKEN", TOKEN.to_owned()),
            ("AGENT_PAGER_TELEGRAM_CHAT_ID", CHAT_ID.to_owned()),
            ("AGENT_PAGER_DEFAULT_HOST", "desktop".to_owned()),
            (
                "HOME",
                tempfile::tempdir()
                    .expect("tempdir")
                    .path()
                    .display()
                    .to_string(),
            ),
        ]))
    }
}

impl ConfigSource for FakeConfig {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    body: Vec<u8>,
}

#[tokio::test]
async fn app_composition_sends_inline_page_through_gateway() {
    let (base_url, requests) = spawn_server(
        1,
        "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}",
    );
    let app = test_app(base_url);

    let outcome = app
        .run(AppCommand::Send(SendPageCommand {
            message_source: MessageSource::Inline("composition send".to_owned()),
            document_source: None,
            document_name: None,
            priority: Priority::Normal,
            format: MessageFormat::Plain,
            context_options: ContextOptions::default(),
            sensitivity_mode: SensitivityMode::Preflight,
        }))
        .await
        .expect("send succeeds");

    assert_eq!(outcome, AppOutcome::Send(SendOutcome::SentText));
    let request = requests.recv().expect("request");
    assert_eq!(request.path, format!("/bot{TOKEN}/sendMessage"));
    let body = String::from_utf8(request.body).expect("json body");
    assert!(body.contains("composition send"));
    assert!(body.contains(CHAT_ID));
}

#[tokio::test]
async fn app_composition_sends_test_page_through_gateway() {
    let (base_url, requests) = spawn_server(
        1,
        "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\n{\"ok\":true}",
    );
    let app = test_app(base_url);

    let outcome = app.run(AppCommand::Test).await.expect("test succeeds");

    assert_eq!(outcome, AppOutcome::Test(TestOutcome::Sent));
    let request = requests.recv().expect("request");
    assert_eq!(request.path, format!("/bot{TOKEN}/sendMessage"));
    let body = String::from_utf8(request.body).expect("json body");
    assert!(body.contains("agent-pager test from desktop"));
}

#[tokio::test]
async fn app_composition_runs_doctor_without_transport() {
    let app = test_app(Url::parse("http://127.0.0.1:9").expect("url"));

    let outcome = app.run(AppCommand::Doctor).await.expect("doctor runs");

    let AppOutcome::Doctor(report) = outcome else {
        panic!("doctor outcome expected");
    };
    assert!(report.ok);
    assert!(report.lines.contains(&"config: ok".to_owned()));
    assert!(!report.lines.join("\n").contains(TOKEN));
}

#[tokio::test]
async fn app_composition_installs_skill_with_explicit_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested/agent-pager/SKILL.md");
    let app = test_app(Url::parse("http://127.0.0.1:9").expect("url"));

    let outcome = app
        .run(AppCommand::InstallSkill(InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: false,
        }))
        .await
        .expect("install succeeds");

    assert_eq!(
        outcome,
        AppOutcome::InstallSkill(InstallSkillOutcome::Installed { path: path.clone() })
    );
    let installed = std::fs::read_to_string(path).expect("installed skill");
    assert!(installed.contains("# agent-pager"));
}

#[test]
fn app_outcome_status_lines_cover_all_variants() {
    assert_eq!(
        AppOutcome::Send(SendOutcome::SentText).status_lines(),
        vec!["sent page to Telegram"]
    );
    assert_eq!(
        AppOutcome::Send(SendOutcome::SentDocument).status_lines(),
        vec!["sent document to Telegram"]
    );
    assert_eq!(
        AppOutcome::Send(SendOutcome::SentTextAsDocument).status_lines(),
        vec!["sent page as document to Telegram"]
    );
    assert_eq!(
        AppOutcome::Test(TestOutcome::Sent).status_lines(),
        vec!["sent test page to Telegram"]
    );

    let path = PathBuf::from("/tmp/SKILL.md");
    assert_eq!(
        AppOutcome::InstallSkill(InstallSkillOutcome::WouldInstall { path: path.clone() })
            .status_lines(),
        vec!["skill path: /tmp/SKILL.md", "status: would install"]
    );
    assert_eq!(
        AppOutcome::InstallSkill(InstallSkillOutcome::WouldUpdate { path: path.clone() })
            .status_lines(),
        vec!["skill path: /tmp/SKILL.md", "status: would update"]
    );
    assert_eq!(
        AppOutcome::InstallSkill(InstallSkillOutcome::UpToDate {
            path: path.clone(),
            dry_run: true,
        })
        .status_lines(),
        vec!["skill path: /tmp/SKILL.md", "status: up to date"]
    );
    assert_eq!(
        AppOutcome::InstallSkill(InstallSkillOutcome::UpToDate {
            path,
            dry_run: false,
        })
        .status_lines(),
        vec!["skill already up to date at /tmp/SKILL.md"]
    );

    let doctor = AppOutcome::Doctor(DoctorReport {
        lines: vec!["config: missing".to_owned()],
        ok: false,
    });
    assert_eq!(doctor.status_lines(), vec!["config: missing"]);
    assert_eq!(
        doctor.failure_message(),
        Some("agent-pager doctor found configuration errors")
    );
}

fn test_app(base_url: Url) -> App<FakeConfig> {
    App::new(
        FakeConfig::valid(),
        reqwest::Client::new(),
        HostName::new("fallback").expect("fallback host"),
        base_url,
    )
}

fn spawn_server(count: usize, response: &'static str) -> (Url, mpsc::Receiver<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let base_url =
        Url::parse(&format!("http://{}", listener.local_addr().expect("addr"))).expect("url");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for _ in 0..count {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_request(&mut stream);
            sender.send(request).expect("send request");
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });
    (base_url, receiver)
}

fn read_request(stream: &mut TcpStream) -> CapturedRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        assert_ne!(read, 0, "connection closed before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("header end");
    let headers = String::from_utf8(buffer[..header_end].to_vec()).expect("headers utf8");
    let mut lines = headers.lines();
    let request_line = lines.next().expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let content_length = lines
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().expect("content length"))
        })
        .unwrap_or(0);

    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    if body.len() < content_length {
        let mut rest = vec![0_u8; content_length - body.len()];
        stream.read_exact(&mut rest).expect("read body");
        body.extend(rest);
    }

    CapturedRequest { path, body }
}
