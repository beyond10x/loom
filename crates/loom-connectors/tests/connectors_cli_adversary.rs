//! Adversary cases for `story:connectors-cli-reads`: what `loom_connectors::cli` does with a
//! program that never answers, a configured path an operator may hold, and answers that name
//! another operation or contradict their exit status.
//!
//! Each test copies `tests/fixtures/connectors/fake-connectors` (or writes its own program) into
//! its own directory under `CARGO_TARGET_TMPDIR` and hands the client its own environment, `HOME`
//! a directory of the test.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, OperationId,
};
use b10x_loom_connectors::cli::{CliError, ConnectorsCli, json};

const ADAPTER: &str = "chat";
const CONNECTION: &str = "conn-fixture";
const LIST: &str = "channel.history";
const POST: &str = "message.post";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    /// A test directory; `segment` is one directory between the test's root and its files.
    fn new(test: &str, segment: &str) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("connectors_cli_adversary")
            .join(test);
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        let root = base.join(segment);
        for directory in ["state", "home"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let fake = root.join("connectors");
        fs::copy(fixtures().join("fake-connectors"), &fake).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(root.join("connectors.toml"), "# not read by the fake\n").unwrap();
        Self { root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn config(&self) -> ConnectorsCliConfig {
        ConnectorsCliConfig {
            program: self.root.join("connectors").display().to_string(),
            config: Some(self.root.join("connectors.toml").display().to_string()),
            state_dir: Some(self.state().display().to_string()),
            timeout_seconds: None,
        }
    }

    fn cli(&self) -> ConnectorsCli {
        ConnectorsCli::new(self.config(), Vec::new()).with_environment(vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ])
    }

    fn answer(&self, verb: &str, operation: &str, file: &str) {
        fs::copy(
            fixtures().join(file),
            self.state().join(format!("{verb}.{operation}.json")),
        )
        .unwrap();
    }

    fn answer_text(&self, verb: &str, operation: &str, text: &str) {
        fs::write(self.state().join(format!("{verb}.{operation}.json")), text).unwrap();
    }

    fn exit(&self, verb: &str, status: i32) {
        fs::write(
            self.state().join(format!("{verb}.exit")),
            status.to_string(),
        )
        .unwrap();
    }

    fn argv(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.state().join("argv.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.strip_suffix('\t')
                    .unwrap_or(line)
                    .split('\t')
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    fn runs(&self, verb: &str) -> usize {
        self.argv()
            .iter()
            .filter(|argv| {
                argv.windows(2)
                    .any(|w| w[0] == "operations" && w[1] == verb)
            })
            .count()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

fn adapter() -> AdapterAlias {
    AdapterAlias(ADAPTER.to_owned())
}

fn connection() -> ConnectionId {
    ConnectionId(CONNECTION.to_owned())
}

fn operation(id: &str) -> OperationId {
    OperationId(id.to_owned())
}

fn input() -> json::Value {
    json::parse(r#"{"channel":"C0FIXTURE1","limit":20}"#).unwrap()
}

/// A `connectors` that never answers (a locked keyring prompt, a stuck owner) blocks the caller
/// for as long as the program runs: there is no bound on a read.
#[test]
fn a_program_that_never_answers_does_not_block_the_read() {
    let fixture = Fixture::new("a_program_that_never_answers_does_not_block_the_read", "t");
    let program = fixture.root.join("connectors");
    // The program records its pid before it becomes `sleep`, so the test can ask whether it
    // still runs once the read returned.
    let pid_file = fixture.root.join("connectors.pid");
    fs::write(
        &program,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec sleep 60\n",
            pid_file.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let cli = ConnectorsCli::new(
        ConnectorsCliConfig {
            timeout_seconds: Some(1),
            ..fixture.config()
        },
        Vec::new(),
    )
    .with_environment(vec![
        (
            "HOME".to_owned(),
            fixture.root.join("home").display().to_string(),
        ),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ]);

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let answer = cli.invoke_read(&adapter(), &connection(), &operation(LIST), &input());
        let _ = sender.send(answer);
    });

    match receiver.recv_timeout(Duration::from_secs(15)) {
        Ok(answer) => {
            assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");
            let message = answer.unwrap_err().to_string();
            assert!(message.contains("timed out after 1 s"), "{message}");
            assert!(message.contains(LIST), "{message}");
        }
        Err(_) => panic!(
            "the read had not returned 15 s after it started: the caller is blocked for as long \
             as `connectors` runs"
        ),
    }
    // Killed and reaped: `kill -0` finds no process (a zombie would still answer it).
    let pid = fs::read_to_string(&pid_file).unwrap();
    let alive = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("kill -0 {}", pid.trim())])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!alive.success(), "pid {} still runs", pid.trim());
}

/// A `--config` or `--state-dir` path is one argv element and cannot read as a flag unless it
/// starts with `-`; a directory name with a space in it is still refused.
#[test]
fn a_configured_path_with_a_space_reaches_the_program() {
    let fixture = Fixture::new(
        "a_configured_path_with_a_space_reaches_the_program",
        "Application Support",
    );
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-read.json");

    let read = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());

    assert!(read.is_ok(), "{read:?}");
    let config = fixture.config().config.unwrap();
    assert_eq!(fixture.runs("invoke"), 1, "{:?}", fixture.argv());
    assert!(
        fixture.argv()[1]
            .windows(2)
            .any(|w| w[0] == "--config" && w[1] == config),
        "{:?}",
        fixture.argv()
    );
}

/// A describe that answers a read operation when a write was asked for is never invoked.
#[test]
fn describe_answering_another_operation_is_never_invoked() {
    let fixture = Fixture::new("describe_answering_another_operation_is_never_invoked", "t");
    fixture.answer("describe", POST, "describe-get.json");
    fixture.answer("invoke", POST, "invoke-get.json");

    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(POST), &input());

    assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");
    assert_eq!(fixture.runs("describe"), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("invoke"), 0, "{:?}", fixture.argv());
}

/// An invoke answer naming another operation is not returned as this operation's read.
#[test]
fn invoke_answering_another_operation_is_not_a_read() {
    let fixture = Fixture::new("invoke_answering_another_operation_is_not_a_read", "t");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-get.json");

    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());

    assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");
}

/// A non-zero exit is never a read, even with a success answer on stdout; a zero exit with a
/// refusal on stdout is never a read either.
#[test]
fn an_answer_contradicting_its_exit_status_is_not_a_read() {
    let fixture = Fixture::new("an_answer_contradicting_its_exit_status_is_not_a_read", "t");
    fixture.answer("describe", LIST, "describe-list.json");
    let failing = fixture.root.join("connectors");
    // The fake prints a non-zero answer on stderr; this program prints it on stdout.
    let original = fs::read_to_string(&failing).unwrap();
    let to_stderr = "    cat \"$answer\" >&2\n";
    assert!(original.contains(to_stderr), "the fake changed: {original}");
    fs::write(
        &failing,
        original.replace(to_stderr, "    cat \"$answer\"\n"),
    )
    .unwrap();

    fixture.answer("invoke", LIST, "invoke-read.json");
    fixture.exit("invoke", 1);
    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());
    assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");

    fixture.answer("invoke", LIST, "invoke-not-granted.json");
    fixture.exit("invoke", 0);
    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());
    assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");
}

/// A truncated answer, or a refusal carrying a token in its text, never puts the token in the
/// error the caller sees.
#[test]
fn an_answer_never_carries_its_text_into_the_error() {
    let fixture = Fixture::new("an_answer_never_carries_its_text_into_the_error", "t");
    fixture.answer("describe", LIST, "describe-list.json");
    let secret = "fixture-secret-value";

    fixture.answer_text(
        "invoke",
        LIST,
        &format!(r#"{{"ok":true,"result":{{"operation":"{LIST}","result":{{"token":"{secret}""#),
    );
    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());
    match &answer {
        Err(error @ CliError::Failed(_)) => {
            assert!(!error.to_string().contains(secret), "{error}");
        }
        other => panic!("a truncated answer was read: {other:?}"),
    }

    fixture.answer_text(
        "invoke",
        LIST,
        &format!(
            r#"{{"ok":false,"error":{{"code":"failure","message":"{secret}","data":{{"code":"not_granted","detail":"{secret}"}}}}}}"#
        ),
    );
    fixture.exit("invoke", 1);
    let answer = fixture
        .cli()
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input());
    match &answer {
        Err(error @ CliError::Refused(refusal)) => {
            assert_eq!(refusal.code, "not_granted");
            assert!(
                !format!("{error} {refusal:?}").contains(secret),
                "{refusal:?}"
            );
        }
        other => panic!("not_granted was not a refusal: {other:?}"),
    }
}
