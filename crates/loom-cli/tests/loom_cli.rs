//! Acceptance for `story:loom-cli`: `b10x-loom run` replaces `b10x-intake run`, with the same
//! flags, built with clap derive.
//!
//! The binary is `CARGO_BIN_EXE_b10x-loom`; while the crate still builds `b10x-intake`, this file
//! does not compile, which is its red run. No case reaches a model, the network or a credential:
//! only `--help` output is read.

use std::process::Command;

#[test]
fn confinement_defaults_to_substrate_and_requires_explicit_opt_out() {
    let help = help(&["run", "--help"]);
    assert!(help.contains("--confinement"), "{help}");
    assert!(help.contains("[default: substrate]"), "{help}");
    assert!(help.contains("--cgroup-root"), "{help}");
    assert!(help.contains("ConfinementUnavailable"), "{help}");
}

#[test]
fn reexec_without_private_route_handoff_fails_before_model_access() {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args([
            "run",
            "--workspace",
            ".",
            "--cgroup-root",
            "/loom-nonexistent-cgroup",
            "fix tests",
        ])
        .env("B10X_LOOM_CONFINEMENT_REEXEC", "1")
        .env("HOME", "/loom-no-credentials")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("private routing handoff"), "{error}");
    assert!(!error.contains("picked "), "{error}");
}

/// Runs `b10x-loom` with `args` and returns its standard output, asserting it succeeded.
fn help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(args)
        .output()
        .expect("run b10x-loom");
    assert!(
        output.status.success(),
        "`b10x-loom {}` succeeds: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("help is UTF-8")
}

/// The words of `text`, split at white space and commas.
fn tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|token| !token.is_empty())
        .collect()
}

/// `b10x-loom --help` names Loom and its `run` command, and nothing of Intake.
#[test]
fn b10x_loom_help_names_loom_not_intake() {
    let help = help(&["--help"]);
    assert!(help.contains("Loom"), "`--help` names Loom: {help}");
    assert!(
        help.contains("Usage: b10x-loom"),
        "the usage line is b10x-loom's: {help}"
    );
    assert!(
        tokens(&help).contains(&"run"),
        "`--help` lists the run command: {help}"
    );
    assert!(
        !help.to_lowercase().contains("intake"),
        "`--help` names nothing of Intake: {help}"
    );
}

/// `b10x-loom run --help` lists every flag `b10x-intake run` had, and the positional intent.
#[test]
fn b10x_loom_run_lists_its_flags() {
    let help = help(&["run", "--help"]);
    assert!(
        help.contains("Usage: b10x-loom run"),
        "the usage line is b10x-loom run's: {help}"
    );
    let tokens = tokens(&help);
    for flag in [
        "--workspace",
        "--test-cmd",
        "--max-steps",
        "--model",
        "--classifier-model",
        "--threshold",
    ] {
        assert!(tokens.contains(&flag), "`run --help` lists {flag}: {help}");
    }
    assert!(
        help.contains("<INTENT>"),
        "the intent is positional: {help}"
    );
}
