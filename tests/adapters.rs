use std::{collections::HashMap, path::Path};

use agent_pager::{
    AgentPagerError,
    adapters::{
        EnvConfigSource, FsDocumentStore, FsSkillStore, SystemContextProvider, SystemProcessRunner,
    },
    command::{ContextOptions, DocumentSource},
    domain::{DocumentFileName, HostName},
    ports::{
        ConfigSource, ContextProvider, DocumentStore, MessageReader, ProcessOutput, ProcessRunner,
        SkillStore,
    },
};

#[derive(Debug, Clone)]
struct FakeReader {
    text: Result<String, &'static str>,
    bytes: Result<Vec<u8>, &'static str>,
}

impl MessageReader for FakeReader {
    fn read_text_stdin(&self) -> Result<String, AgentPagerError> {
        self.text
            .clone()
            .map_err(|message| AgentPagerError::Other(message.to_owned()))
    }

    fn read_document_stdin(&self) -> Result<Vec<u8>, AgentPagerError> {
        self.bytes
            .clone()
            .map_err(|message| AgentPagerError::Other(message.to_owned()))
    }
}

#[derive(Debug, Clone, Default)]
struct FakeConfig(HashMap<&'static str, String>);

impl ConfigSource for FakeConfig {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

#[derive(Debug, Clone)]
struct FakeProcess {
    output: Result<ProcessOutput, &'static str>,
}

impl ProcessRunner for FakeProcess {
    fn output(&self, _program: &str, _args: &[&str]) -> Result<ProcessOutput, AgentPagerError> {
        self.output
            .clone()
            .map_err(|message| AgentPagerError::Other(message.to_owned()))
    }
}

#[test]
fn fs_document_store_reads_path_and_names_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("report.md");
    std::fs::write(&path, b"# Report\n").expect("write document");
    let store = FsDocumentStore::new(FakeReader {
        text: Ok(String::new()),
        bytes: Ok(Vec::new()),
    });

    let document = store
        .read_document(&DocumentSource::Path(path.clone()), None)
        .expect("read document");

    assert_eq!(document.file_name().as_str(), "report.md");
    assert_eq!(document.bytes(), b"# Report\n");
}

#[test]
fn fs_document_store_uses_explicit_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("source.txt");
    std::fs::write(&path, b"body").expect("write document");
    let store = FsDocumentStore::new(FakeReader {
        text: Ok(String::new()),
        bytes: Ok(Vec::new()),
    });

    let document = store
        .read_document(
            &DocumentSource::Path(path),
            Some(DocumentFileName::new(" summary.md ").expect("name")),
        )
        .expect("read document");

    assert_eq!(document.file_name().as_str(), "summary.md");
    assert_eq!(document.bytes(), b"body");
}

#[test]
fn fs_document_store_reads_document_from_stdin() {
    let store = FsDocumentStore::new(FakeReader {
        text: Ok(String::new()),
        bytes: Ok(b"streamed".to_vec()),
    });

    let document = store
        .read_document(&DocumentSource::Stdin, None)
        .expect("read stdin document");

    assert_eq!(document.file_name().as_str(), "agent-pager-document.md");
    assert_eq!(document.bytes(), b"streamed");
}

#[test]
fn fs_document_store_rejects_missing_file_name() {
    let store = FsDocumentStore::new(FakeReader {
        text: Ok(String::new()),
        bytes: Ok(Vec::new()),
    });

    let error = store
        .read_document(&DocumentSource::Path(Path::new("/").to_path_buf()), None)
        .expect_err("missing file name");

    assert!(matches!(
        error,
        AgentPagerError::DocumentRead { .. } | AgentPagerError::DocumentFileNameMissing
    ));
}

#[test]
fn system_context_includes_tmux_session_when_available() {
    let provider = SystemContextProvider::new(
        FakeProcess {
            output: Ok(ProcessOutput {
                success: true,
                stdout: b"main\n".to_vec(),
            }),
        },
        FakeConfig(HashMap::from([("TMUX", "yes".to_owned())])),
    );

    let context = provider
        .gather(
            HostName::new("desktop").expect("host"),
            ContextOptions {
                include_cwd: false,
                include_tmux: true,
            },
        )
        .expect("context");

    assert_eq!(context.tmux().expect("tmux").as_str(), "main");
}

#[test]
fn system_context_tmux_failure_is_unavailable() {
    let provider = SystemContextProvider::new(
        FakeProcess {
            output: Ok(ProcessOutput {
                success: false,
                stdout: Vec::new(),
            }),
        },
        FakeConfig(HashMap::from([("TMUX", "yes".to_owned())])),
    );

    let context = provider
        .gather(
            HostName::new("desktop").expect("host"),
            ContextOptions {
                include_cwd: false,
                include_tmux: true,
            },
        )
        .expect("context");

    assert_eq!(context.tmux().expect("tmux").as_str(), "unavailable");
}

#[test]
fn system_context_without_tmux_env_is_unavailable() {
    let provider = SystemContextProvider::new(
        FakeProcess {
            output: Err("should not run"),
        },
        FakeConfig::default(),
    );

    let context = provider
        .gather(
            HostName::new("desktop").expect("host"),
            ContextOptions {
                include_cwd: false,
                include_tmux: true,
            },
        )
        .expect("context");

    assert_eq!(context.tmux().expect("tmux").as_str(), "unavailable");
}

#[test]
fn env_config_source_reads_process_environment() {
    let source = EnvConfigSource;
    unsafe {
        std::env::set_var("AGENT_PAGER_TEST_ENV_CONFIG_SOURCE", " present ");
    }

    assert_eq!(
        source.get("AGENT_PAGER_TEST_ENV_CONFIG_SOURCE").as_deref(),
        Some(" present ")
    );

    unsafe {
        std::env::remove_var("AGENT_PAGER_TEST_ENV_CONFIG_SOURCE");
    }
}

#[test]
fn fs_skill_store_reads_missing_and_written_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested/SKILL.md");
    let store = FsSkillStore;

    assert_eq!(store.read_to_string(&path).expect("missing file"), None);
    store
        .write_string(&path, "skill contents")
        .expect("write skill");
    assert_eq!(
        store.read_to_string(&path).expect("read skill"),
        Some("skill contents".to_owned())
    );
}

#[test]
fn system_process_runner_captures_successful_output() {
    let output = SystemProcessRunner
        .output("printf", &["agent-pager"])
        .expect("process output");

    assert!(output.success);
    assert_eq!(output.stdout, b"agent-pager");
}
