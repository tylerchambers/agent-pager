use std::path::PathBuf;

use crate::{
    AgentPagerError,
    command::ContextOptions,
    domain::{DisplayPath, HostName, PageContext, TmuxSession, display_path},
    ports::{ConfigSource, ContextProvider, ProcessRunner},
};

const HOME_ENV: &str = "HOME";
const TMUX_ENV: &str = "TMUX";

#[derive(Debug, Clone)]
pub struct SystemContextProvider<P, C> {
    process_runner: P,
    config_source: C,
}

impl<P, C> SystemContextProvider<P, C> {
    pub fn new(process_runner: P, config_source: C) -> Self {
        Self {
            process_runner,
            config_source,
        }
    }
}

impl<P, C> ContextProvider for SystemContextProvider<P, C>
where
    P: ProcessRunner,
    C: ConfigSource,
{
    fn gather(
        &self,
        default_host: HostName,
        options: ContextOptions,
    ) -> Result<PageContext, AgentPagerError> {
        let cwd = if options.include_cwd {
            Some(self.current_display_path()?)
        } else {
            None
        };
        let tmux = if options.include_tmux {
            Some(self.current_tmux_session())
        } else {
            None
        };
        Ok(PageContext::new(default_host, cwd, tmux))
    }
}

impl<P, C> SystemContextProvider<P, C>
where
    P: ProcessRunner,
    C: ConfigSource,
{
    fn current_display_path(&self) -> Result<DisplayPath, AgentPagerError> {
        let current =
            std::env::current_dir().map_err(|source| AgentPagerError::CurrentDir { source })?;
        let home = self
            .config_source
            .get(HOME_ENV)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        DisplayPath::new(display_path(&current, home.as_deref()))
    }

    fn current_tmux_session(&self) -> TmuxSession {
        if self.config_source.get(TMUX_ENV).is_none() {
            return TmuxSession::unavailable();
        }

        let Ok(output) = self
            .process_runner
            .output("tmux", &["display-message", "-p", "#S"])
        else {
            return TmuxSession::unavailable();
        };
        if !output.success {
            return TmuxSession::unavailable();
        }
        let session = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        TmuxSession::new(session).unwrap_or_else(|_| TmuxSession::unavailable())
    }
}
