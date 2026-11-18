//! Binary-level CLI tests for `jeb`.
//!
//! These drive the built binary through error paths and help output.
//! They make no network calls: every case exits before evaluation.

use std::process::Command;

fn jeb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jeb"))
}

fn stderr_of(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("failed to run jeb binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_lists_quick_mode_flags() {
    let (ok, stdout, _) = stderr_of(jeb().arg("--help"));
    assert!(ok);
    assert!(stdout.contains("--state"));
    assert!(stdout.contains("--choice"));
    assert!(stdout.contains("--decision-only"));
}

#[test]
fn question_flag_requires_state() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--choice")
            .arg("task:Do this")
            .arg("--option")
            .arg("a=First"),
    );
    assert!(!ok);
    assert!(stderr.contains("require --state"));
}

#[test]
fn option_requires_choice() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--state")
            .arg("state")
            .arg("--option")
            .arg("a=First")
            .arg("--noul")
            .arg("safe:Is it safe?"),
    );
    assert!(!ok);
    assert!(stderr.contains("--option requires --choice"));
}

#[test]
fn file_conflicts_with_positional_input() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--file")
            .arg("fixtures/req_ping.json")
            .arg("{\"state\": \"x\"}"),
    );
    assert!(!ok);
    assert!(stderr.contains("--file cannot be combined"));
}

#[test]
fn zero_timeout_rejected_before_network() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--timeout")
            .arg("0")
            .arg("--state")
            .arg("state")
            .arg("--noul")
            .arg("safe:Is it safe?"),
    );
    assert!(!ok);
    assert!(stderr.contains("greater than zero"));
}

#[test]
fn list_models_rejects_typesafe_provider() {
    let (ok, _, stderr) = stderr_of(jeb().arg("--provider").arg("typesafe").arg("--list-models"));
    assert!(!ok);
    assert!(stderr.contains("--provider openrouter"));
}

#[test]
fn quiet_dry_run_prints_nothing() {
    let (ok, stdout, _) = stderr_of(
        jeb()
            .arg("--state")
            .arg("state")
            .arg("--noul")
            .arg("safe:Is it safe?")
            .arg("--dry-run")
            .arg("--quiet"),
    );
    assert!(ok);
    assert!(stdout.is_empty());
}

#[test]
fn dry_run_prints_request_without_calling() {
    let (ok, stdout, _) = stderr_of(
        jeb()
            .arg("--state")
            .arg("state")
            .arg("--noul")
            .arg("safe:Is it safe?")
            .arg("--dry-run"),
    );
    assert!(ok);
    let body: serde_json::Value =
        serde_json::from_str(&stdout).expect("dry run printed invalid JSON");
    assert!(body["questions"]["safe"]["instructions"] == "Is it safe?");
}

#[test]
fn dry_run_rejects_list_models() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--provider")
            .arg("openrouter")
            .arg("--list-models")
            .arg("--dry-run"),
    );
    assert!(!ok);
    assert!(stderr.contains("--dry-run cannot be combined"));
}

#[test]
fn list_models_rejects_evaluation_flags() {
    let (ok, _, stderr) = stderr_of(
        jeb()
            .arg("--provider")
            .arg("openrouter")
            .arg("--list-models")
            .arg("--option")
            .arg("foo"),
    );
    assert!(!ok);
    assert!(stderr.contains("--list-models cannot be combined"));
}
