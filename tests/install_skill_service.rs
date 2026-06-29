use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use agent_pager::{
    AgentPagerError,
    adapters::FsSkillStore,
    app::{InstallSkillOutcome, InstallSkillService},
    command::InstallSkillCommand,
    ports::{ConfigSource, SkillStore},
};
use tempfile::tempdir;

const BUNDLED_SKILL: &str = "# Agent Pager\nUse agent-pager to notify a human.\n";

#[test]
fn dry_run_missing_file_would_install() {
    let path = PathBuf::from("/tmp/new-skill.md");
    let store = FakeSkillStore::default();
    let service =
        InstallSkillService::new(store.clone(), FakeConfigSource::default(), BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: true,
        })
        .expect("dry run should inspect missing file");

    assert_eq!(outcome, InstallSkillOutcome::WouldInstall { path });
    assert_eq!(store.reads(), vec![PathBuf::from("/tmp/new-skill.md")]);
    assert!(store.writes().is_empty());
}

#[test]
fn dry_run_stale_file_would_update() {
    let path = PathBuf::from("/tmp/stale-skill.md");
    let store = FakeSkillStore::with_current("old contents");
    let service =
        InstallSkillService::new(store.clone(), FakeConfigSource::default(), BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: true,
        })
        .expect("dry run should inspect stale file");

    assert_eq!(outcome, InstallSkillOutcome::WouldUpdate { path });
    assert!(store.writes().is_empty());
}

#[test]
fn identical_file_is_up_to_date_without_write() {
    let path = PathBuf::from("/tmp/current-skill.md");
    let store = FakeSkillStore::with_current(BUNDLED_SKILL);
    let service =
        InstallSkillService::new(store.clone(), FakeConfigSource::default(), BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: false,
        })
        .expect("matching file should be up to date");

    assert_eq!(
        outcome,
        InstallSkillOutcome::UpToDate {
            path,
            dry_run: false
        }
    );
    assert!(store.writes().is_empty());
}

#[test]
fn write_creates_parent_dirs_using_fs_skill_store_and_tempfile() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("nested/skills/agent-pager/SKILL.md");
    let service =
        InstallSkillService::new(FsSkillStore, FakeConfigSource::default(), BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: false,
        })
        .expect("install should create parent directories");

    assert_eq!(
        outcome,
        InstallSkillOutcome::Installed { path: path.clone() }
    );
    assert!(path.parent().expect("parent").is_dir());
    assert_eq!(
        fs::read_to_string(path).expect("installed file"),
        BUNDLED_SKILL
    );
}

#[test]
fn writes_bundled_content_to_store() {
    let path = PathBuf::from("/tmp/install-skill.md");
    let store = FakeSkillStore::default();
    let service =
        InstallSkillService::new(store.clone(), FakeConfigSource::default(), BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: Some(path.clone()),
            dry_run: false,
        })
        .expect("install should write bundled content");

    assert_eq!(
        outcome,
        InstallSkillOutcome::Installed { path: path.clone() }
    );
    assert_eq!(store.writes(), vec![(path, BUNDLED_SKILL.to_owned())]);
}

#[test]
fn default_path_comes_from_home_config_source() {
    let home = PathBuf::from("/tmp/agent-pager-home");
    let expected = home.join(".omp/agent/skills/agent-pager/SKILL.md");
    let store = FakeSkillStore::default();
    let source = FakeConfigSource::new([("HOME", home.to_str().expect("utf-8 home"))]);
    let service = InstallSkillService::new(store.clone(), source, BUNDLED_SKILL);

    let outcome = service
        .install(&InstallSkillCommand {
            path: None,
            dry_run: true,
        })
        .expect("default path should resolve from HOME");

    assert_eq!(
        outcome,
        InstallSkillOutcome::WouldInstall {
            path: expected.clone()
        }
    );
    assert_eq!(store.reads(), vec![expected]);
    assert!(store.writes().is_empty());
}

#[derive(Clone, Default)]
struct FakeConfigSource {
    values: HashMap<&'static str, String>,
}

impl FakeConfigSource {
    fn new<const N: usize>(values: [(&'static str, &str); N]) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|(key, value)| (key, value.to_owned()))
                .collect(),
        }
    }
}

impl ConfigSource for FakeConfigSource {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

#[derive(Clone, Default)]
struct FakeSkillStore {
    current: Arc<Mutex<Option<String>>>,
    reads: Arc<Mutex<Vec<PathBuf>>>,
    writes: Arc<Mutex<Vec<(PathBuf, String)>>>,
}

impl FakeSkillStore {
    fn with_current(current: impl Into<String>) -> Self {
        Self {
            current: Arc::new(Mutex::new(Some(current.into()))),
            ..Self::default()
        }
    }

    fn reads(&self) -> Vec<PathBuf> {
        self.reads.lock().expect("reads").clone()
    }

    fn writes(&self) -> Vec<(PathBuf, String)> {
        self.writes.lock().expect("writes").clone()
    }
}

impl SkillStore for FakeSkillStore {
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, AgentPagerError> {
        self.reads.lock().expect("reads").push(path.to_path_buf());
        Ok(self.current.lock().expect("current").clone())
    }

    fn write_string(&self, path: &Path, contents: &str) -> Result<(), AgentPagerError> {
        self.writes
            .lock()
            .expect("writes")
            .push((path.to_path_buf(), contents.to_owned()));
        *self.current.lock().expect("current") = Some(contents.to_owned());
        Ok(())
    }
}
