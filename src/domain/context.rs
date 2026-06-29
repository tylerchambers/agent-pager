use std::{fmt, path::Path};

use crate::AgentPagerError;

use super::HostName;

#[derive(Clone, PartialEq, Eq)]
pub struct DisplayPath(String);

impl DisplayPath {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let path = input.into().trim().to_owned();
        if path.is_empty() {
            return Err(AgentPagerError::InvalidCommand(
                "display path cannot be empty".to_owned(),
            ));
        }
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DisplayPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DisplayPath").field(&self.0).finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TmuxSession(String);

impl TmuxSession {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let session = input.into().trim().to_owned();
        if session.is_empty() {
            return Err(AgentPagerError::InvalidCommand(
                "tmux session cannot be empty".to_owned(),
            ));
        }
        Ok(Self(session))
    }

    pub fn unavailable() -> Self {
        Self("unavailable".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TmuxSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TmuxSession").field(&self.0).finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageContext {
    host: HostName,
    cwd: Option<DisplayPath>,
    tmux: Option<TmuxSession>,
}

impl PageContext {
    pub fn new(host: HostName, cwd: Option<DisplayPath>, tmux: Option<TmuxSession>) -> Self {
        Self { host, cwd, tmux }
    }

    pub fn host(&self) -> &HostName {
        &self.host
    }

    pub fn cwd(&self) -> Option<&DisplayPath> {
        self.cwd.as_ref()
    }

    pub fn tmux(&self) -> Option<&TmuxSession> {
        self.tmux.as_ref()
    }
}

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home.filter(|home| !home.as_os_str().is_empty()) {
        if path == home {
            return "~".to_owned();
        }

        if let Ok(relative) = path.strip_prefix(home) {
            return format!("~/{}", relative.display());
        }
    }

    path.display().to_string()
}
