//! Adversary cases for `story:agent-check`: `b10x-loom agent check --spec` driven against its
//! own `after_help` (exit 0 serveable, 3 refused, 1 unreadable, 2 bad command line) and the brief's
//! "standard error never carries the secret value", on documents its own tests do not cover.
//! Documents are written under `CARGO_TARGET_TMPDIR`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn operator() -> String {
    std::fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("operator example")
}

/// A spec file holding `bytes`, unique to this test and process.
fn spec_file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-agent-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write the spec");
    path
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_b10x-loom"))
        .args(args)
        .env_remove("B10X_LOOM_CONFINEMENT_REEXEC")
        .output()
        .expect("b10x-loom runs")
}

fn agent_check(spec: &Path) -> Output {
    run(&[
        "agent".as_ref(),
        "check".as_ref(),
        "--spec".as_ref(),
        spec.as_os_str(),
    ])
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The brief: "stderr never carries the secret value". A credential in an enum member is refused
/// by the decoder first, and its message, printed as the refusal's detail, repeats it.
#[test]
fn adv_a_credential_in_an_enum_member_never_reaches_stderr() {
    let spec = spec_file(
        "enum-credential.yaml",
        operator()
            .replacen("mode: auto", "mode: xoxb-hunter2", 1)
            .as_bytes(),
    );
    let output = agent_check(&spec);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        !stderr.contains("hunter2"),
        "standard error repeats the credential: {stderr}"
    );
}

/// `after_help`: 1 "the file cannot be read"; a file that is not UTF-8 is not read as a spec.
#[test]
fn adv_a_spec_that_is_not_utf8_exits_1() {
    let mut bytes = operator().into_bytes();
    bytes.extend_from_slice(b"# \xff\xfe\n");
    let output = agent_check(&spec_file("not-utf8.yaml", &bytes));
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty());
}

/// `after_help`: 2 "the command line is not valid": `--spec` missing.
#[test]
fn adv_no_spec_argument_exits_2() {
    let output = run(&["agent".as_ref(), "check".as_ref()]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
}

/// An empty file decodes to no document: refused (3) as malformed, naming `document`.
#[test]
fn adv_an_empty_spec_is_refused_as_malformed() {
    let output = agent_check(&spec_file("empty.yaml", b""));
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("refused: Malformed at document: "),
        "{stderr}"
    );
}

/// YAML admits a byte order mark at the start of a stream: the operator example behind one is the
/// operator example.
#[test]
fn adv_a_spec_behind_a_byte_order_mark_checks() {
    let mut bytes = "\u{feff}".as_bytes().to_vec();
    bytes.extend_from_slice(operator().as_bytes());
    let output = agent_check(&spec_file("bom.yaml", &bytes));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stdout),
        "checked platform-operator: 3 states, 3 bindings, protocol operator-reply@1\n"
    );
}
