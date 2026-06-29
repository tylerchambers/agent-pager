use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    AgentPagerError,
    command::{
        AppCommand, ContextOptions, DocumentSource, InstallSkillCommand, MessageSource,
        SendPageCommand, SensitivityMode,
    },
    domain::{DocumentFileName, MessageFormat, Priority},
};

#[derive(Debug, Parser)]
#[command(
    name = "agent-pager",
    version,
    about = "Send Telegram pages and documents from agent sessions",
    long_about = "Send Telegram pages and documents from agent sessions, shell scripts, tmux hooks, or tools.\n\nSecurity: Telegram is a pager, not a secure transport. Do not send secrets, credentials, tokenized URLs, customer data, or other sensitive content."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
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
pub struct SendArgs {
    /// Page message to send. When --document is set, this becomes the document caption.
    #[arg(conflicts_with = "stdin")]
    message: Option<String>,

    /// Page priority.
    #[arg(long, value_enum, default_value_t = CliPriority::Normal)]
    priority: CliPriority,

    /// Message or caption format accepted by Telegram.
    #[arg(long, value_enum, default_value_t = CliMessageFormat::Plain)]
    format: CliMessageFormat,

    /// Include the current working directory.
    #[arg(long)]
    cwd: bool,

    /// Include the current tmux session name when available.
    #[arg(long)]
    tmux: bool,

    /// Read the page message from stdin. Cannot be combined with MESSAGE or --document.
    #[arg(long, conflicts_with = "document")]
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
pub struct InstallSkillArgs {
    /// Skill destination. Defaults to ~/.omp/agent/skills/agent-pager/SKILL.md.
    #[arg(long, value_name = "PATH")]
    path: Option<PathBuf>,

    /// Show the destination and whether it would change, without writing.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliPriority {
    Normal,
    High,
}

impl From<CliPriority> for Priority {
    fn from(value: CliPriority) -> Self {
        match value {
            CliPriority::Normal => Self::Normal,
            CliPriority::High => Self::High,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CliMessageFormat {
    Plain,
    #[value(name = "markdown-v2")]
    MarkdownV2,
    Html,
}

impl From<CliMessageFormat> for MessageFormat {
    fn from(value: CliMessageFormat) -> Self {
        match value {
            CliMessageFormat::Plain => Self::Plain,
            CliMessageFormat::MarkdownV2 => Self::MarkdownV2,
            CliMessageFormat::Html => Self::Html,
        }
    }
}

impl TryFrom<Cli> for AppCommand {
    type Error = AgentPagerError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        match cli.command {
            CliCommand::Send(args) => SendPageCommand::try_from(args).map(AppCommand::Send),
            CliCommand::Doctor => Ok(AppCommand::Doctor),
            CliCommand::InstallSkill(args) => Ok(AppCommand::InstallSkill(InstallSkillCommand {
                path: args.path,
                dry_run: args.dry_run,
            })),
            CliCommand::Test => Ok(AppCommand::Test),
        }
    }
}

impl TryFrom<SendArgs> for SendPageCommand {
    type Error = AgentPagerError;

    fn try_from(args: SendArgs) -> Result<Self, Self::Error> {
        if args.stdin && args.message.is_some() {
            return Err(AgentPagerError::InvalidCommand(
                "use either MESSAGE or --stdin, not both".to_owned(),
            ));
        }
        if args.stdin && args.document.is_some() {
            return Err(AgentPagerError::InvalidCommand(
                "--stdin cannot be combined with --document".to_owned(),
            ));
        }

        let document_source = args.document.as_ref().map(|path| {
            if path == Path::new("-") {
                DocumentSource::Stdin
            } else {
                DocumentSource::Path(path.clone())
            }
        });
        if args.message.is_none() && !args.stdin && document_source.is_none() {
            return Err(AgentPagerError::InvalidCommand(
                "provide a message, --stdin, or --document <path>".to_owned(),
            ));
        }

        let message_source = if args.stdin {
            MessageSource::Stdin
        } else {
            args.message
                .map(MessageSource::Inline)
                .unwrap_or(MessageSource::None)
        };
        let document_name = args.document_name.map(DocumentFileName::new).transpose()?;

        Ok(SendPageCommand {
            message_source,
            document_source,
            document_name,
            priority: args.priority.into(),
            format: args.format.into(),
            context_options: ContextOptions {
                include_cwd: args.cwd,
                include_tmux: args.tmux,
            },
            sensitivity_mode: if args.allow_sensitive {
                SensitivityMode::AllowSensitive
            } else {
                SensitivityMode::Preflight
            },
        })
    }
}
