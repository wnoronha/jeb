//! Live end-to-end tests for `jeb`.
//!
//! These hit the real OpenRouter API. They stay ignored by default:
//! run with `JEB_E2E=1 cargo test --test e2e -- --ignored`.
//! The binary loads its own key from `.env`; this file never reads it.
//! Assertions cover response shape only, never exact model values.

use std::process::Command;

fn live_enabled() -> bool {
    std::env::var("JEB_E2E").map(|v| v == "1").unwrap_or(false)
}

fn run_jeb(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_jeb"))
        .args(args)
        .output()
        .expect("failed to run jeb binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
#[ignore]
fn live_openrouter_ping_shape() {
    if !live_enabled() {
        eprintln!("skipped: set JEB_E2E=1 to run live e2e");
        return;
    }
    let (ok, stdout) = run_jeb(&[
        "--provider",
        "openrouter",
        "--file",
        "fixtures/req_ping.json",
    ]);
    assert!(ok, "live ping exited nonzero");
    let body: serde_json::Value =
        serde_json::from_str(&stdout).expect("live ping printed invalid JSON");
    assert!(
        body["model"].as_str().is_some_and(|m| !m.is_empty()),
        "model must be non-empty"
    );
    assert_eq!(body["answers"]["proceed"]["type"], "noul");
    let noul = body["answers"]["proceed"]["noul"]
        .as_f64()
        .expect("noul must be a number");
    assert!((0.0..=1.0).contains(&noul), "noul out of range");
}

#[test]
#[ignore]
fn live_fail_flag_follows_noul() {
    if !live_enabled() {
        eprintln!("skipped: set JEB_E2E=1 to run live e2e");
        return;
    }
    let (ok, stdout) = run_jeb(&[
        "--provider",
        "openrouter",
        "--file",
        "fixtures/req_ping.json",
    ]);
    assert!(ok, "live ping exited nonzero");
    let body: serde_json::Value =
        serde_json::from_str(&stdout).expect("live ping printed invalid JSON");
    let noul = body["answers"]["proceed"]["noul"]
        .as_f64()
        .expect("noul must be a number");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jeb"))
        .args([
            "--provider",
            "openrouter",
            "--file",
            "fixtures/req_ping.json",
            "--fail",
        ])
        .output()
        .expect("failed to run jeb binary");
    let expected = if noul < 0.5 { Some(1) } else { Some(0) };
    assert_eq!(out.status.code(), expected);
}

#[test]
#[ignore]
fn live_openrouter_decision_only_shape() {
    if !live_enabled() {
        eprintln!("skipped: set JEB_E2E=1 to run live e2e");
        return;
    }
    let (ok, stdout) = run_jeb(&[
        "--provider",
        "openrouter",
        "--file",
        "fixtures/req_ping.json",
        "--decision-only",
    ]);
    assert!(ok, "live decision-only exited nonzero");
    let body: serde_json::Value =
        serde_json::from_str(&stdout).expect("decision-only printed invalid JSON");
    assert_eq!(body["decisions"]["proceed"]["type"], "noul");
    assert!(body["decisions"]["proceed"]["decision"]
        .as_str()
        .is_some_and(|d| d == "yes" || d == "no"));
}
