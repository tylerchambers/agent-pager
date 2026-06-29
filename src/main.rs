use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use agent_pager::{
    MessageFormat, PageContext, PagerConfig, Priority, TelegramDocument, build_document_caption,
    build_page_text, build_test_text, send_telegram_document, send_telegram_message,
    send_telegram_message_with_format,
};

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

    /// Send this file as a Telegram document instead of a short text message. Use '-' to read stdin.
    #[arg(long, value_name = "PATH")]
    document: Option<PathBuf>,

    /// Filename to show for --document. Useful with '--document -'.
    #[arg(long, value_name = "NAME", requires = "document")]
    document_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = PagerConfig::from_env()?;
    let client = reqwest::Client::new();

    match cli.command {
        Command::Send(args) => {
            let context = PageContext::gather(config.default_host.clone(), args.cwd, args.tmux)?;

            if let Some(document_path) = args.document {
                let document = load_document(&document_path, args.document_name)?;
                let caption =
                    build_document_caption(args.message.as_deref(), args.priority, &context)?;
                send_telegram_document(&client, &config, document, Some(&caption), args.format)
                    .await?;
                println!("sent document to Telegram");
            } else {
                let message = args
                    .message
                    .as_deref()
                    .ok_or_else(|| anyhow!("provide a message or --document <path>"))?;
                let text = build_page_text(message, args.priority, &context)?;
                send_telegram_message_with_format(&client, &config, &text, args.format).await?;
                println!("sent page to Telegram");
            }
        }
        Command::Test => {
            let text = build_test_text(&config.default_host);
            send_telegram_message(&client, &config, &text).await?;
            println!("sent test page to Telegram");
        }
    }

    Ok(())
}

fn load_document(path: &Path, document_name: Option<String>) -> Result<TelegramDocument> {
    if path == Path::new("-") {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read document from stdin")?;
        let file_name = document_name.unwrap_or_else(|| "agent-pager-document.md".to_owned());
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

    fn temp_document_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("agent-pager-{}-{nanos}-{name}", std::process::id()))
    }
}
