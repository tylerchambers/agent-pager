use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use agent_pager::{
    AUTO_DOCUMENT_FILE_NAME, BOT_TOKEN_ENV, CHAT_ID_ENV, DEFAULT_HOST_ENV, MessageFormat,
    PageContext, PagerConfig, Priority, TelegramDocument, build_document_caption,
    build_page_text_unlimited, build_test_text, fits_telegram_text_message, scan_sensitive_bytes,
    scan_sensitive_text, send_telegram_document, send_telegram_message,
    send_telegram_message_with_format,
};

const AGENT_PAGER_SKILL: &str = include_str!("../skills/agent-pager/SKILL.md");
const AUTO_DOCUMENT_CAPTION: &str = "Message attached as document";

#[derive(Debug, Parser)]
#[command(
    name = "agent-pager",
    version,
    about = "Send Telegram pages and documents from agent sessions",
    long_about = "Send Telegram pages and documents from agent sessions, shell scripts, tmux hooks, or tools.\n\nSecurity: Telegram is a pager, not a secure transport. Do not send secrets, credentials, tokenized URLs, customer data, or other sensitive content."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Send a Telegram page or document.
    Send(SendArgs),
    /// Check local configuration without printing secrets.
    Doctor,
    /// Install the bundled OMP skill into the local agent skill directory.
    InstallSkill(InstallSkillArgs),
    /// Send a raw Telegram API smoke-test message.
    Test,
}

#[derive(Debug, Args)]
struct SendArgs {
    /// Page message to send. When --document is set, this becomes the document caption.
    message: Option<String>,

    /// Page priority.
    #[arg(long, value_enum, default_value_t = Priority::Normal)]
    priority: Priority,

    /// Message or caption format accepted by Telegram.
    #[arg(long, value_enum, default_value_t = MessageFormat::Plain)]
    format: MessageFormat,

    /// Include the current working directory.
    #[arg(long)]
    cwd: bool,

    /// Include the current tmux session name when available.
    #[arg(long)]
    tmux: bool,

    /// Read the page message from stdin. Cannot be combined with MESSAGE or --document.
    #[arg(long)]
    stdin: bool,

    /// Send this file as a Telegram document instead of a short text message. Use '-' to read stdin.
    #[arg(long, value_name = "PATH")]
    document: Option<PathBuf>,

    /// Filename to show for document uploads, including automatic long-message uploads.
    #[arg(long, value_name = "NAME")]
    document_name: Option<String>,

    /// Skip the sensitive-content preflight. Use only after reviewing the payload.
    #[arg(long)]
    allow_sensitive: bool,
}

#[derive(Debug, Args)]
struct InstallSkillArgs {
    /// Skill destination. Defaults to ~/.omp/agent/skills/agent-pager/SKILL.md.
    #[arg(long, value_name = "PATH")]
    path: Option<PathBuf>,

    /// Show the destination and whether it would change, without writing.
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Send(args) => {
            let config = PagerConfig::from_env()?;
            let client = reqwest::Client::new();
            run_send(&client, &config, args).await?;
        }
        Command::Doctor => run_doctor()?,
        Command::InstallSkill(args) => install_skill(args)?,
        Command::Test => {
            let config = PagerConfig::from_env()?;
            let client = reqwest::Client::new();
            let text = build_test_text(&config.default_host);
            send_telegram_message(&client, &config, &text).await?;
            println!("sent test page to Telegram");
        }
    }

    Ok(())
}

async fn run_send(client: &reqwest::Client, config: &PagerConfig, args: SendArgs) -> Result<()> {
    let context = PageContext::gather(config.default_host.clone(), args.cwd, args.tmux)?;
    let message = read_message(&args)?;

    if let Some(document_path) = &args.document {
        let document = load_document(document_path, args.document_name.clone())?;
        guard_sensitive_payload(&args, message.as_deref(), Some(&document))?;
        let caption = build_document_caption(message.as_deref(), args.priority, &context)?;
        send_telegram_document(client, config, document, Some(&caption), args.format).await?;
        println!("sent document to Telegram");
        return Ok(());
    }

    let message = message
        .as_deref()
        .ok_or_else(|| anyhow!("provide a message, --stdin, or --document <path>"))?;
    guard_sensitive_payload(&args, Some(message), None)?;

    let text = build_page_text_unlimited(message, args.priority, &context)?;
    if fits_telegram_text_message(&text) {
        send_telegram_message_with_format(client, config, &text, args.format).await?;
        println!("sent page to Telegram");
        return Ok(());
    }

    let document = TelegramDocument::new(
        document_file_name(args.document_name.as_deref(), AUTO_DOCUMENT_FILE_NAME),
        message.as_bytes().to_vec(),
    )?;
    let caption = build_document_caption(Some(AUTO_DOCUMENT_CAPTION), args.priority, &context)?;
    send_telegram_document(client, config, document, Some(&caption), args.format).await?;
    println!("sent page as document to Telegram");
    Ok(())
}

fn read_message(args: &SendArgs) -> Result<Option<String>> {
    if args.stdin && args.message.is_some() {
        bail!("use either MESSAGE or --stdin, not both");
    }

    if args.stdin && args.document.is_some() {
        bail!("--stdin cannot be combined with --document");
    }

    if !args.stdin {
        return Ok(args.message.clone());
    }

    let mut message = String::new();
    io::stdin()
        .read_to_string(&mut message)
        .context("failed to read page message from stdin")?;
    Ok(Some(message))
}

fn guard_sensitive_payload(
    args: &SendArgs,
    message: Option<&str>,
    document: Option<&TelegramDocument>,
) -> Result<()> {
    if args.allow_sensitive {
        return Ok(());
    }

    if let Some(message) = message {
        scan_sensitive_text("message", message)?;
    }

    if let Some(document) = document {
        scan_sensitive_bytes("document", &document.bytes)?;
    }

    Ok(())
}

fn document_file_name(explicit: Option<&str>, fallback: &str) -> String {
    explicit.unwrap_or(fallback).to_owned()
}

fn load_document(path: &Path, document_name: Option<String>) -> Result<TelegramDocument> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read document from stdin")?;
        let file_name = document_file_name(document_name.as_deref(), "agent-pager-document.md");
        return TelegramDocument::new(file_name, bytes);
    }

    let bytes =
        fs::read(path).with_context(|| format!("failed to read document {}", path.display()))?;
    let file_name = match document_name {
        Some(file_name) => file_name,
        None => path
            .file_name()
            .map(|file_name| file_name.to_string_lossy().into_owned())
            .ok_or_else(|| anyhow!("document path must have a file name or --document-name"))?,
    };

    TelegramDocument::new(file_name, bytes)
}

fn run_doctor() -> Result<()> {
    let lookup = |key: &str| env::var(key).ok();
    let fallback_host = fallback_doctor_host();
    let report = build_doctor_report(&lookup, &fallback_host);

    for line in &report.lines {
        println!("{line}");
    }

    if report.ok {
        Ok(())
    } else {
        bail!("agent-pager doctor found configuration errors")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DoctorReport {
    lines: Vec<String>,
    ok: bool,
}

fn build_doctor_report<F>(lookup: &F, fallback_host: &str) -> DoctorReport
where
    F: Fn(&str) -> Option<String>,
{
    let mut lines = vec![
        env_status_line("bot token", BOT_TOKEN_ENV, lookup),
        env_status_line("chat id", CHAT_ID_ENV, lookup),
    ];

    let default_host = lookup(DEFAULT_HOST_ENV)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_host.to_owned());
    lines.push(format!("default host: {default_host}"));

    match PagerConfig::from_lookup(|key| lookup(key), fallback_host.to_owned()) {
        Ok(_) => {
            lines.push("config: ok".to_owned());
            DoctorReport { lines, ok: true }
        }
        Err(error) => {
            lines.push(format!("config: {error}"));
            DoctorReport { lines, ok: false }
        }
    }
}

fn env_status_line<F>(label: &str, key: &str, lookup: &F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key).map(|value| value.trim().to_owned()) {
        Some(value) if !value.is_empty() => {
            format!("{label}: present, length {}", value.chars().count())
        }
        _ => format!("{label}: missing"),
    }
}

fn fallback_doctor_host() -> String {
    env::var("HOSTNAME")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn install_skill(args: InstallSkillArgs) -> Result<()> {
    let path = match args.path {
        Some(path) => path,
        None => default_skill_path()?,
    };
    let current = fs::read_to_string(&path).ok();
    let status = skill_install_status(current.as_deref());

    if args.dry_run {
        println!("skill path: {}", path.display());
        println!("status: {status}");
        return Ok(());
    }

    if status == "up to date" {
        println!("skill already up to date at {}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create skill directory {}", parent.display()))?;
    }
    fs::write(&path, AGENT_PAGER_SKILL)
        .with_context(|| format!("failed to write skill {}", path.display()))?;
    println!("installed skill to {}", path.display());
    Ok(())
}

fn skill_install_status(current: Option<&str>) -> &'static str {
    match current {
        Some(current) if current == AGENT_PAGER_SKILL => "up to date",
        Some(_) => "would update",
        None => "would install",
    }
}

fn default_skill_path() -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".omp/agent/skills/agent-pager/SKILL.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_document_uses_path_file_name_and_bytes() {
        let path = temp_document_path("report.md");
        std::fs::write(&path, b"# Report\n").expect("write temp document");

        let document = load_document(&path, None).expect("load document");
        std::fs::remove_file(&path).expect("remove temp document");

        assert_eq!(
            document.file_name,
            path.file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        );
        assert_eq!(document.bytes, b"# Report\n");
    }

    #[test]
    fn load_document_uses_explicit_document_name() {
        let path = temp_document_path("source.txt");
        std::fs::write(&path, b"body").expect("write temp document");

        let document = load_document(&path, Some("summary.md".to_owned())).expect("load document");
        std::fs::remove_file(&path).expect("remove temp document");

        assert_eq!(document.file_name, "summary.md");
        assert_eq!(document.bytes, b"body");
    }

    #[test]
    fn read_message_rejects_message_and_stdin() {
        let args = send_args_with_message(Some("hello".to_owned()), true);

        let error = read_message(&args).expect_err("conflicting inputs");

        assert_eq!(error.to_string(), "use either MESSAGE or --stdin, not both");
    }

    #[test]
    fn build_doctor_report_redacts_config_values() {
        let vars = std::collections::HashMap::from([
            (BOT_TOKEN_ENV, "123456:real-token-shape"),
            (CHAT_ID_ENV, "123456789"),
            (DEFAULT_HOST_ENV, "desktop"),
        ]);

        let report = build_doctor_report(
            &|key| vars.get(key).map(|value| (*value).to_owned()),
            "fallback",
        );

        assert!(report.ok);
        assert_eq!(
            report.lines,
            vec![
                "bot token: present, length 23",
                "chat id: present, length 9",
                "default host: desktop",
                "config: ok",
            ]
        );
        assert!(!report.lines.join("\n").contains("real-token-shape"));
    }

    #[test]
    fn build_doctor_report_flags_placeholder_config() {
        let vars = std::collections::HashMap::from([
            (BOT_TOKEN_ENV, "replace-with-botfather-token"),
            (CHAT_ID_ENV, "replace-with-chat-id"),
            (DEFAULT_HOST_ENV, "desktop"),
        ]);

        let report = build_doctor_report(
            &|key| vars.get(key).map(|value| (*value).to_owned()),
            "fallback",
        );

        assert!(!report.ok);
        assert_eq!(
            report.lines,
            vec![
                "bot token: present, length 28",
                "chat id: present, length 20",
                "default host: desktop",
                "config: AGENT_PAGER_TELEGRAM_BOT_TOKEN still contains the example placeholder value",
            ]
        );
    }

    #[test]
    fn install_skill_writes_bundled_skill() {
        let path = temp_document_path("SKILL.md");

        install_skill(InstallSkillArgs {
            path: Some(path.clone()),
            dry_run: false,
        })
        .expect("install skill");

        let installed = std::fs::read_to_string(&path).expect("read installed skill");
        std::fs::remove_file(&path).expect("remove installed skill");

        assert_eq!(installed, AGENT_PAGER_SKILL);
        assert_eq!(skill_install_status(Some(&installed)), "up to date");
        assert_eq!(skill_install_status(Some("old")), "would update");
        assert_eq!(skill_install_status(None), "would install");
    }

    fn send_args_with_message(message: Option<String>, stdin: bool) -> SendArgs {
        SendArgs {
            message,
            priority: Priority::Normal,
            format: MessageFormat::Plain,
            cwd: false,
            tmux: false,
            stdin,
            document: None,
            document_name: None,
            allow_sensitive: false,
        }
    }

    fn temp_document_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-pager-{}-{nanos}-{name}", std::process::id()))
    }
}
