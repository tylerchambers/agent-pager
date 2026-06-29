use std::path::PathBuf;

use crate::domain::{DocumentFileName, MessageFormat, Priority};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    Send(SendPageCommand),
    Doctor,
    InstallSkill(InstallSkillCommand),
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendPageCommand {
    pub message_source: MessageSource,
    pub document_source: Option<DocumentSource>,
    pub document_name: Option<DocumentFileName>,
    pub priority: Priority,
    pub format: MessageFormat,
    pub context_options: ContextOptions,
    pub sensitivity_mode: SensitivityMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageSource {
    Inline(String),
    Stdin,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentSource {
    Path(PathBuf),
    Stdin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextOptions {
    pub include_cwd: bool,
    pub include_tmux: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityMode {
    Preflight,
    AllowSensitive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSkillCommand {
    pub path: Option<PathBuf>,
    pub dry_run: bool,
}
