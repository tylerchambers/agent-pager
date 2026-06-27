use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use agent_pager::{
    PageContext, PagerConfig, Priority, build_page_text, build_test_text, send_telegram_message,
};

#[derive(Debug, Parser)]
#[command(
    name = "agent-pager",
    version,
    about = "Send terse Telegram pages from agent sessions",
    long_about = "Send terse Telegram pages from agent sessions, shell scripts, tmux hooks, or tools.\n\nSecurity: Telegram is a pager, not a secure transport. Do not send secrets, logs, stack traces, diffs, credentials, tokenized URLs, or customer data."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Send a Telegram page.
    Send(SendArgs),
    /// Send a raw Telegram API smoke-test message.
    Test,
}

#[derive(Debug, Args)]
struct SendArgs {
    /// Terse attention message to send.
    message: String,

    /// Page priority.
    #[arg(long, value_enum, default_value_t = Priority::Normal)]
    priority: Priority,

    /// Include the current working directory.
    #[arg(long)]
    cwd: bool,

    /// Include the current tmux session name when available.
    #[arg(long)]
    tmux: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = PagerConfig::from_env()?;
    let client = reqwest::Client::new();

    match cli.command {
        Command::Send(args) => {
            let context = PageContext::gather(config.default_host.clone(), args.cwd, args.tmux)?;
            let text = build_page_text(&args.message, args.priority, &context)?;
            send_telegram_message(&client, &config, &text).await?;
            println!("sent page to Telegram");
        }
        Command::Test => {
            let text = build_test_text(&config.default_host);
            send_telegram_message(&client, &config, &text).await?;
            println!("sent test page to Telegram");
        }
    }

    Ok(())
}
