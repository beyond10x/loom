//! `b10x-loom agent check --spec <FILE>` says whether a `loom.agent/1` document can be served,
//! before anything runs: exit 0 and one line on standard output when it can, exit 3 and the
//! refusal on standard error when it cannot, exit 1 when the file cannot be read. The documents
//! are the operator example and the fixtures of the authored ESS scenarios.

use std::path::PathBuf;
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn agent_check(spec: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(["agent", "check", "--spec"])
        .arg(spec)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .output()
        .expect("b10x-loom runs")
}

fn fixture(branch: &str) -> PathBuf {
    root().join(format!("ess/scenarios/agent-check/specs/{branch}.yaml"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_valid_spec_checks() {
    let output = agent_check(&root().join("examples/agents/operator.yaml"));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "checked platform-operator: 3 states, 3 bindings, protocol operator-reply@1\n"
    );
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn a_state_naming_an_unknown_action_is_refused() {
    let output = agent_check(&fixture("unknown-action"));
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("refused: UnknownAction at states[0].actions[2]: reply.escalate"),
        "{stderr}"
    );
}

#[test]
fn a_missing_file_exits_1() {
    let output = agent_check(&root().join("examples/agents/no-such-agent.yaml"));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
    assert!(
        text(&output.stderr).contains("no-such-agent.yaml"),
        "the refusal names the file: {}",
        text(&output.stderr)
    );
}

#[test]
fn a_secret_literal_is_refused_without_repeating_it() {
    let output = agent_check(&fixture("secret-literal"));
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains(
            "refused: SecretLiteral at bindings[1].base_url: a URL carrying a user name and password"
        ),
        "{stderr}"
    );
    for shown in [&stderr, &text(&output.stdout)] {
        assert!(
            !shown.contains("hunter2"),
            "the password is repeated: {shown}"
        );
    }
}

#[test]
fn a_directory_is_not_read() {
    let output = agent_check(&root().join("examples/agents"));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
}

/// Each value the decoder refuses is printed under its own reason, never `Malformed`, and the
/// variable name is not repeated.
#[test]
fn a_value_refusal_prints_its_own_reason() {
    for (branch, line) in [
        ("unknown-format", "refused: UnknownFormat at format: "),
        ("blank-text", "refused: BlankText at agent.name: "),
        (
            "not-a-variable-name",
            "refused: NotAVariableName at bindings[2].bearer_env: ",
        ),
        (
            "port-out-of-range",
            "refused: PortOutOfRange at expose.a2a.port: ",
        ),
        (
            "not-a-plain-path",
            "refused: NotAPlainPath at bindings[2].allow_paths[1]: ",
        ),
        (
            "not-an-instant",
            "refused: NotAnInstant at session.budget.deadline: ",
        ),
    ] {
        let output = agent_check(&fixture(branch));
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(3), "{branch}: {stderr}");
        assert!(stderr.contains(line), "{branch}: {stderr}");
        assert!(!stderr.contains("METRICS-TOKEN"), "{stderr}");
    }
}
