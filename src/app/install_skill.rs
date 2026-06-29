use std::path::{Path, PathBuf};

use crate::{
    AgentPagerError,
    command::InstallSkillCommand,
    ports::{ConfigSource, SkillStore},
};

const HOME_ENV: &str = "HOME";
const DEFAULT_SKILL_RELATIVE_PATH: &str = ".omp/agent/skills/agent-pager/SKILL.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallSkillOutcome {
    UpToDate { path: PathBuf, dry_run: bool },
    Installed { path: PathBuf },
    WouldInstall { path: PathBuf },
    WouldUpdate { path: PathBuf },
}

impl InstallSkillOutcome {
    pub fn status_lines(&self) -> Vec<String> {
        match self {
            Self::UpToDate { path, dry_run } if *dry_run => vec![
                format!("skill path: {}", path.display()),
                "status: up to date".to_owned(),
            ],
            Self::UpToDate { path, .. } => {
                vec![format!("skill already up to date at {}", path.display())]
            }
            Self::Installed { path } => vec![format!("installed skill to {}", path.display())],
            Self::WouldInstall { path } => vec![
                format!("skill path: {}", path.display()),
                "status: would install".to_owned(),
            ],
            Self::WouldUpdate { path } => vec![
                format!("skill path: {}", path.display()),
                "status: would update".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallSkillService<S, C> {
    store: S,
    source: C,
    bundled_skill: &'static str,
}

impl<S, C> InstallSkillService<S, C> {
    pub fn new(store: S, source: C, bundled_skill: &'static str) -> Self {
        Self {
            store,
            source,
            bundled_skill,
        }
    }
}

impl<S, C> InstallSkillService<S, C>
where
    S: SkillStore,
    C: ConfigSource,
{
    pub fn install(
        &self,
        command: &InstallSkillCommand,
    ) -> Result<InstallSkillOutcome, AgentPagerError> {
        let path = match &command.path {
            Some(path) => path.clone(),
            None => self.default_skill_path()?,
        };
        let current = self.store.read_to_string(&path)?;
        let matches_bundle = current.as_deref() == Some(self.bundled_skill);

        if matches_bundle {
            return Ok(InstallSkillOutcome::UpToDate {
                path,
                dry_run: command.dry_run,
            });
        }

        if command.dry_run {
            return Ok(if current.is_some() {
                InstallSkillOutcome::WouldUpdate { path }
            } else {
                InstallSkillOutcome::WouldInstall { path }
            });
        }

        self.store.write_string(&path, self.bundled_skill)?;
        Ok(InstallSkillOutcome::Installed { path })
    }

    fn default_skill_path(&self) -> Result<PathBuf, AgentPagerError> {
        let home = self
            .source
            .get(HOME_ENV)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AgentPagerError::Other("HOME is not set".to_owned()))?;
        Ok(Path::new(&home).join(DEFAULT_SKILL_RELATIVE_PATH))
    }
}
