use std::{io, path::PathBuf};

use crate::security::SensitiveReason;

#[derive(thiserror::Error, Debug)]
pub enum AgentPagerError {
    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),

    #[error("{0} still contains the example placeholder value")]
    PlaceholderEnv(&'static str),

    #[error("page message cannot be empty")]
    EmptyMessage,

    #[error("document cannot be empty")]
    EmptyDocument,

    #[error("document file name cannot be empty")]
    EmptyDocumentFileName,

    #[error("refusing to send {label}: detected {reason}; pass --allow-sensitive to send anyway")]
    SensitivePayload {
        label: String,
        reason: SensitiveReason,
    },

    #[error("{label} is {actual} characters; limit is {limit} characters")]
    CharacterLimitExceeded {
        label: &'static str,
        actual: usize,
        limit: usize,
    },

    #[error("failed to read stdin: {0}")]
    StdinRead(#[source] io::Error),

    #[error("failed to read document {path}: {source}")]
    DocumentRead { path: PathBuf, source: io::Error },

    #[error("document path must have a file name or --document-name")]
    DocumentFileNameMissing,

    #[error("failed to get current working directory: {source}")]
    CurrentDir { source: io::Error },

    #[error("failed to read skill {path}: {source}")]
    SkillRead { path: PathBuf, source: io::Error },

    #[error("failed to create skill directory {path}: {source}")]
    SkillCreateDir { path: PathBuf, source: io::Error },

    #[error("failed to write skill {path}: {source}")]
    SkillWrite { path: PathBuf, source: io::Error },

    #[error("failed to run {program}: {source}")]
    ProcessLaunch { program: String, source: io::Error },

    #[error("Telegram {method} failed with status {status}: {body}")]
    TelegramStatus {
        method: &'static str,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to call Telegram {method}: {message}")]
    TelegramTransport {
        method: &'static str,
        message: String,
    },

    #[error("{0}")]
    InvalidCommand(String),

    #[error("{0}")]
    Other(String),
}
