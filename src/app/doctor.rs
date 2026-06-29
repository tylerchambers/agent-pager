use crate::{
    domain::{BOT_TOKEN_ENV, CHAT_ID_ENV, DEFAULT_HOST_ENV, HostName},
    ports::ConfigSource,
};

use super::{PagerConfigLoader, config_loader::optional_trimmed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub lines: Vec<String>,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct DoctorService<C> {
    source: C,
    host_fallback: HostName,
}

impl<C> DoctorService<C> {
    pub fn new(source: C, host_fallback: HostName) -> Self {
        Self {
            source,
            host_fallback,
        }
    }
}

impl<C> DoctorService<C>
where
    C: ConfigSource + Clone,
{
    pub fn run(&self) -> DoctorReport {
        let mut lines = vec![
            env_status_line("bot token", BOT_TOKEN_ENV, &self.source),
            env_status_line("chat id", CHAT_ID_ENV, &self.source),
        ];

        let default_host = optional_trimmed(&self.source, DEFAULT_HOST_ENV)
            .unwrap_or_else(|| self.host_fallback.as_str().to_owned());
        lines.push(format!("default host: {default_host}"));

        let loader = PagerConfigLoader::new(self.source.clone(), self.host_fallback.clone());
        match loader.load() {
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
}

fn env_status_line<C>(label: &str, key: &str, source: &C) -> String
where
    C: ConfigSource,
{
    match optional_trimmed(source, key) {
        Some(value) => format!("{label}: present, length {}", value.chars().count()),
        None => format!("{label}: missing"),
    }
}
