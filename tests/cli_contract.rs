use std::path::PathBuf;

use agent_pager::{
    command::{AppCommand, DocumentSource, MessageSource, SensitivityMode},
    domain::{DocumentFileName, MessageFormat, Priority},
};
use assert_cmd::Command;
use clap::Parser;
use predicates::prelude::*;

#[test]
fn top_level_help_describes_agent_pager_without_env() {
    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Send Telegram pages and documents from agent sessions",
        ))
        .stdout(predicate::str::contains("send"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("install-skill"));
}

#[test]
fn send_help_describes_send_options_without_env() {
    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .args(["send", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Send a Telegram page or document"))
        .stdout(predicate::str::contains("--stdin"))
        .stdout(predicate::str::contains("--document <PATH>"))
        .stdout(predicate::str::contains("--format <FORMAT>"))
        .stdout(predicate::str::contains("--allow-sensitive"));
}

#[test]
fn doctor_binary_uses_composition_and_redacts_secrets() {
    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .arg("doctor")
        .env(
            "AGENT_PAGER_TELEGRAM_BOT_TOKEN",
            "123456:abcdefghijklmnopqrstuvwxyz",
        )
        .env("AGENT_PAGER_TELEGRAM_CHAT_ID", "987654321")
        .env("AGENT_PAGER_DEFAULT_HOST", "desktop")
        .assert()
        .success()
        .stdout(predicate::str::contains("bot token: present, length 33"))
        .stdout(predicate::str::contains("chat id: present, length 9"))
        .stdout(predicate::str::contains("default host: desktop"))
        .stdout(predicate::str::contains("config: ok"))
        .stdout(predicate::str::contains("abcdefghijklmnopqrstuvwxyz").not());
}

#[test]
fn doctor_binary_exits_nonzero_for_missing_config() {
    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .arg("doctor")
        .env_remove("AGENT_PAGER_TELEGRAM_BOT_TOKEN")
        .env_remove("AGENT_PAGER_TELEGRAM_CHAT_ID")
        .env_remove("AGENT_PAGER_DEFAULT_HOST")
        .assert()
        .failure()
        .stdout(predicate::str::contains("bot token: missing"))
        .stdout(predicate::str::contains(
            "config: missing required environment variable AGENT_PAGER_TELEGRAM_BOT_TOKEN",
        ))
        .stderr(predicate::str::contains(
            "agent-pager doctor found configuration errors",
        ));
}

#[test]
fn install_skill_binary_writes_explicit_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("skills/agent-pager/SKILL.md");

    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .args(["install-skill", "--path"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("installed skill to"));

    let installed = std::fs::read_to_string(path).expect("installed skill");
    assert!(installed.contains("# agent-pager"));
}

#[test]
fn install_skill_binary_dry_run_uses_home_default_path() {
    let dir = tempfile::tempdir().expect("tempdir");

    Command::cargo_bin("agent-pager")
        .expect("agent-pager binary")
        .arg("install-skill")
        .arg("--dry-run")
        .env("HOME", dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("skill path:"))
        .stdout(predicate::str::contains(
            ".omp/agent/skills/agent-pager/SKILL.md",
        ))
        .stdout(predicate::str::contains("status: would install"));
}

#[test]
fn conflicting_send_message_and_stdin_is_parse_failure() {
    let error = agent_pager::cli::Cli::try_parse_from(["agent-pager", "send", "hello", "--stdin"])
        .expect_err("MESSAGE and --stdin conflict");

    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn conflicting_stdin_and_document_is_parse_failure() {
    let error = agent_pager::cli::Cli::try_parse_from([
        "agent-pager",
        "send",
        "--stdin",
        "--document",
        "report.txt",
    ])
    .expect_err("--stdin and --document conflict");

    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn parses_common_inline_send_invocation() {
    let command = parse_command(["agent-pager", "send", "hello"]);

    match command {
        AppCommand::Send(send) => {
            assert_eq!(
                send.message_source,
                MessageSource::Inline("hello".to_owned())
            );
            assert_eq!(send.document_source, None);
            assert_eq!(send.document_name, None);
            assert_eq!(send.priority, Priority::Normal);
            assert_eq!(send.format, MessageFormat::Plain);
            assert!(!send.context_options.include_cwd);
            assert!(!send.context_options.include_tmux);
            assert_eq!(send.sensitivity_mode, SensitivityMode::Preflight);
        }
        other => panic!("expected send command, got {other:?}"),
    }
}

#[test]
fn parses_common_stdin_send_invocation_with_context_and_sensitive_override() {
    let command = parse_command([
        "agent-pager",
        "send",
        "--stdin",
        "--priority",
        "high",
        "--format",
        "html",
        "--cwd",
        "--tmux",
        "--allow-sensitive",
    ]);

    match command {
        AppCommand::Send(send) => {
            assert_eq!(send.message_source, MessageSource::Stdin);
            assert_eq!(send.document_source, None);
            assert_eq!(send.document_name, None);
            assert_eq!(send.priority, Priority::High);
            assert_eq!(send.format, MessageFormat::Html);
            assert!(send.context_options.include_cwd);
            assert!(send.context_options.include_tmux);
            assert_eq!(send.sensitivity_mode, SensitivityMode::AllowSensitive);
        }
        other => panic!("expected send command, got {other:?}"),
    }
}

#[test]
fn parses_common_document_send_invocation() {
    let command = parse_command([
        "agent-pager",
        "send",
        "caption",
        "--document",
        "./report.txt",
        "--document-name",
        "renamed.txt",
        "--format",
        "markdown-v2",
    ]);

    match command {
        AppCommand::Send(send) => {
            assert_eq!(
                send.message_source,
                MessageSource::Inline("caption".to_owned())
            );
            assert_eq!(
                send.document_source,
                Some(DocumentSource::Path(PathBuf::from("./report.txt")))
            );
            assert_eq!(
                send.document_name,
                Some(DocumentFileName::new("renamed.txt").expect("valid document name"))
            );
            assert_eq!(send.priority, Priority::Normal);
            assert_eq!(send.format, MessageFormat::MarkdownV2);
            assert_eq!(send.sensitivity_mode, SensitivityMode::Preflight);
        }
        other => panic!("expected send command, got {other:?}"),
    }
}

#[test]
fn parses_document_from_stdin_invocation() {
    let command = parse_command(["agent-pager", "send", "--document", "-"]);

    match command {
        AppCommand::Send(send) => {
            assert_eq!(send.message_source, MessageSource::None);
            assert_eq!(send.document_source, Some(DocumentSource::Stdin));
            assert_eq!(send.document_name, None);
        }
        other => panic!("expected send command, got {other:?}"),
    }
}

#[test]
fn parses_common_non_send_invocations() {
    assert!(matches!(
        parse_command(["agent-pager", "doctor"]),
        AppCommand::Doctor
    ));
    assert!(matches!(
        parse_command(["agent-pager", "test"]),
        AppCommand::Test
    ));

    match parse_command([
        "agent-pager",
        "install-skill",
        "--path",
        "/tmp/skill.md",
        "--dry-run",
    ]) {
        AppCommand::InstallSkill(command) => {
            assert_eq!(command.path, Some(PathBuf::from("/tmp/skill.md")));
            assert!(command.dry_run);
        }
        other => panic!("expected install-skill command, got {other:?}"),
    }
}

fn parse_command<const N: usize>(args: [&str; N]) -> AppCommand {
    let cli = agent_pager::cli::Cli::try_parse_from(args).expect("parse CLI args");
    AppCommand::try_from(cli).expect("convert CLI command")
}
