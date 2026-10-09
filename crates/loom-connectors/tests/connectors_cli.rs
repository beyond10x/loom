//! Acceptance for `story:connectors-cli-reads`: `loom_connectors::cli` reads a data source through
//! the `connectors` command line, `operations describe` then one `operations invoke`.
//!
//! The program is `tests/fixtures/connectors/fake-connectors`, copied into each test's own
//! directory under `CARGO_TARGET_TMPDIR`. It appends its argv to `<state-dir>/argv.log`, writes
//! its environment to `<state-dir>/env.log` and the document it read on stdin to
//! `<state-dir>/stdin.log`, and answers from the JSON fixture files the test copies into the state
//! directory: shapes `connectors --output json` prints at tag `v0.38.0`
//! (`apps/connectors/src/local/operations.rs`: `describe` and `project`;
//! `crates/connectors-host/src/local/owner/supervisor.rs`: `read_answer`). Every test hands the
//! client its own environment, `HOME` a directory of the test, so nothing reads the operator's.

use std::fs;
use std::path::{Path, PathBuf};

use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, DataSource, OperationId, ReadKind, ReadResult,
    SourceEntity, SourceName,
};
use b10x_loom_connectors::cli::{CliError, ConnectorsCli, json};

const ADAPTER: &str = "chat";
const CONNECTION: &str = "conn-fixture";
const LIST: &str = "channel.history";
const SEARCH: &str = "message.search";
const GET: &str = "message.get";
const POST: &str = "message.post";
const REACTIONS: &str = "message.reactions";

/// One test's directory: the fake program, its `--config` file, its `--state-dir` and `HOME`.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("connectors_cli")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let fake = root.join("connectors");
        install_program(&fixtures().join("fake-connectors"), &fake);
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

    /// The environment every test hands the client: `HOME` isolated, a plain `PATH`.
    fn environment(&self) -> Vec<(String, String)> {
        vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ]
    }

    fn cli(&self, sources: Vec<DataSource>) -> ConnectorsCli {
        ConnectorsCli::new(self.config(), sources).with_environment(self.environment())
    }

    /// `operations <verb>` of `operation` answers with the fixture `file`.
    fn answer(&self, verb: &str, operation: &str, file: &str) {
        fs::copy(
            fixtures().join(file),
            self.state().join(format!("{verb}.{operation}.json")),
        )
        .unwrap();
    }

    /// `operations <verb>` exits with `status`.
    fn exit(&self, verb: &str, status: i32) {
        fs::write(
            self.state().join(format!("{verb}.exit")),
            status.to_string(),
        )
        .unwrap();
    }

    /// Every run's argv, one entry per run.
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

    /// The runs whose verb is `operations <verb>`.
    fn runs(&self, verb: &str) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| {
                argv.windows(2)
                    .any(|w| w[0] == "operations" && w[1] == verb)
            })
            .collect()
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

/// Installs `source` as the executable `target` through `install(1)`, in a child process, so this
/// process never holds a writable descriptor on a program a test runs. A child that a test running
/// in parallel forks inherits every descriptor open at that moment; while it holds one on the
/// program, executing the program fails with "Text file busy".
fn install_program(source: &Path, target: &Path) {
    let status = std::process::Command::new("/usr/bin/install")
        .args(["-m", "755"])
        .arg(source)
        .arg(target)
        .status()
        .unwrap();
    assert!(status.success(), "install {}: {status}", target.display());
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

fn source(name: &str, list: Option<&str>, search: Option<&str>, get: Option<&str>) -> DataSource {
    DataSource {
        name: SourceName(name.to_owned()),
        adapter: adapter(),
        connection: connection(),
        list: list.map(operation),
        search: search.map(operation),
        get: get.map(operation),
    }
}

fn input() -> json::Value {
    json::parse(r#"{"channel":"C0FIXTURE1","limit":20}"#).unwrap()
}

fn value_of(argv: &[String], flag: &str) -> Option<String> {
    argv.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

#[test]
fn describe_returns_schema_and_revision() {
    let fixture = Fixture::new("describe_returns_schema_and_revision");
    fixture.answer("describe", LIST, "describe-list.json");

    let entity = fixture
        .cli(Vec::new())
        .describe(&adapter(), &operation(LIST))
        .unwrap();

    assert_eq!(
        entity,
        SourceEntity {
            operation: operation(LIST),
            schema: "sch-list-1".to_owned(),
            input_schema: json::parse(
                r#"{"type":"object","properties":{"channel":{"type":"string"},"limit":{"type":"integer"}},"required":["channel"],"additionalProperties":false}"#
            )
            .unwrap(),
            revision: "rev-1".to_owned(),
        }
    );
    assert_eq!(fixture.argv().len(), 1, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("describe").len(), 1);
}

#[test]
fn invoke_read_runs_one_invoke() {
    let fixture = Fixture::new("invoke_read_runs_one_invoke");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-read.json");

    let read = fixture
        .cli(Vec::new())
        .invoke_read(&adapter(), &connection(), &operation(LIST), &input())
        .unwrap();

    assert_eq!(
        read.body,
        json::parse(
            r#"{"messages":[{"channel":"C0FIXTURE1","ts":"1700000000.000100","user":"U0ALICE","text":"Is the deploy finished?"}],"has_more":false}"#
        )
        .unwrap()
    );
    let invokes = fixture.runs("invoke");
    assert_eq!(invokes.len(), 1, "{:?}", fixture.argv());
    assert_eq!(value_of(&invokes[0], "--operation").as_deref(), Some(LIST));
    assert_eq!(
        value_of(&invokes[0], "--connection").as_deref(),
        Some(CONNECTION)
    );
    assert_eq!(
        value_of(&invokes[0], "--schema").as_deref(),
        Some("sch-list-1")
    );
    assert_eq!(
        value_of(&invokes[0], "--revision").as_deref(),
        Some("rev-1")
    );
    // The input goes on stdin, as the document it is.
    assert_eq!(
        fs::read_to_string(fixture.state().join("stdin.log")).unwrap(),
        "{\"channel\":\"C0FIXTURE1\",\"limit\":20}\n"
    );
}

#[test]
fn read_uses_the_declared_operation() {
    let fixture = Fixture::new("read_uses_the_declared_operation");
    for (id, described, invoked) in [
        (LIST, "describe-list.json", "invoke-read.json"),
        (SEARCH, "describe-search.json", "invoke-search.json"),
        (GET, "describe-get.json", "invoke-get.json"),
    ] {
        fixture.answer("describe", id, described);
        fixture.answer("invoke", id, invoked);
    }
    let declared = source("support-channel", Some(LIST), Some(SEARCH), Some(GET));
    let cli = fixture.cli(vec![declared.clone()]);

    for (kind, id) in [
        (ReadKind::Search, SEARCH),
        (ReadKind::Get, GET),
        (ReadKind::List, LIST),
    ] {
        let before = fixture.argv().len();
        let read = cli.read(&declared, kind, &input()).unwrap();
        assert!(
            read.body.member("messages").is_some() == (id == LIST),
            "{read:?}"
        );
        let runs = &fixture.argv()[before..];
        assert_eq!(runs.len(), 2, "{runs:?}");
        for run in runs {
            assert_eq!(value_of(run, "--operation").as_deref(), Some(id), "{run:?}");
            assert_eq!(
                value_of(run, "--adapter").as_deref(),
                Some(ADAPTER),
                "{run:?}"
            );
        }
    }
}

#[test]
fn undeclared_kind_starts_no_process() {
    let fixture = Fixture::new("undeclared_kind_starts_no_process");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-read.json");
    let declared = source("support-channel", Some(LIST), None, None);
    let cli = fixture.cli(vec![declared.clone()]);

    for kind in [ReadKind::Search, ReadKind::Get] {
        match cli.read(&declared, kind, &input()) {
            Err(CliError::Refused(refusal)) => {
                assert_eq!(refusal.code, "undeclared-kind");
                assert_eq!(
                    refusal.source,
                    Some(SourceName("support-channel".to_owned()))
                );
                assert_eq!(refusal.operation, None);
            }
            other => panic!("{kind:?} was not refused: {other:?}"),
        }
    }
    assert!(
        !fixture.state().join("argv.log").exists(),
        "a process started: {:?}",
        fixture.argv()
    );
}

#[test]
fn write_operation_is_never_invoked() {
    let fixture = Fixture::new("write_operation_is_never_invoked");
    fixture.answer("describe", POST, "describe-write.json");
    fixture.answer("invoke", POST, "invoke-read.json");
    let declared = source("support-channel", None, None, Some(POST));
    let cli = fixture.cli(vec![declared.clone()]);

    match cli.read(&declared, ReadKind::Get, &input()) {
        Err(CliError::Refused(refusal)) => {
            assert_eq!(refusal.code, "write-operation");
            assert_eq!(
                refusal.source,
                Some(SourceName("support-channel".to_owned()))
            );
            assert_eq!(refusal.operation, Some(operation(POST)));
        }
        other => panic!("a write was not refused: {other:?}"),
    }
    match cli.invoke_read(&adapter(), &connection(), &operation(POST), &input()) {
        Err(CliError::Refused(refusal)) => assert_eq!(refusal.code, "write-operation"),
        other => panic!("a write was not refused: {other:?}"),
    }
    assert_eq!(fixture.runs("describe").len(), 2, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("invoke").len(), 0, "{:?}", fixture.argv());
    assert_eq!(fixture.argv().len(), 2);
}

/// Adapters publish `resource` for reads, so a `resource` operation is read. What would hold a
/// write published under it is Connectors' own refusal of a write without `--approval-file`
/// (`crates/connectors-host/src/local/owner/mutation/execution.rs:161` at `v0.38.0`), and no
/// invoke this file's tests make ever carries that flag.
#[test]
fn a_resource_profile_read_is_invoked_without_an_approval_file() {
    let fixture = Fixture::new("a_resource_profile_read_is_invoked_without_an_approval_file");
    fixture.answer("describe", REACTIONS, "describe-resource.json");
    fixture.answer("invoke", REACTIONS, "invoke-resource.json");
    let declared = source("support-channel", None, None, Some(REACTIONS));
    let cli = fixture.cli(vec![declared.clone()]);

    let read = cli.read(&declared, ReadKind::Get, &input()).unwrap();

    assert_eq!(
        read.body,
        json::parse(r#"{"reactions":[{"name":"eyes","users":["U0ALICE"]}]}"#).unwrap()
    );
    let invokes = fixture.runs("invoke");
    assert_eq!(invokes.len(), 1, "{:?}", fixture.argv());
    assert_eq!(
        value_of(&invokes[0], "--operation").as_deref(),
        Some(REACTIONS)
    );
    // Every fake log this test file's tests have written, this one's included.
    let logs = Path::new(env!("CARGO_TARGET_TMPDIR")).join("connectors_cli");
    let mut scanned = 0;
    for test in fs::read_dir(&logs).unwrap() {
        let log = test.unwrap().path().join("state/argv.log");
        let Ok(text) = fs::read_to_string(&log) else {
            continue;
        };
        for line in text.lines().filter(|line| line.contains("\tinvoke\t")) {
            scanned += 1;
            assert!(
                !line
                    .split('\t')
                    .any(|argument| argument == "--approval-file"),
                "{}: {line}",
                log.display()
            );
        }
    }
    assert!(
        scanned >= 1,
        "no invoke line was scanned under {}",
        logs.display()
    );
}

/// The client's source builds no `--approval-file` (nor any `approval` argument), and every flag
/// literal it holds is one of the flags it declares, so no read it makes can carry a write's proof.
#[test]
fn no_invoke_ever_carries_an_approval_file() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cli.rs")).unwrap();
    let declared = [
        "--output",
        "--config",
        "--state-dir",
        "--adapter",
        "--connection",
        "--operation",
        "--schema",
        "--revision",
        "--input-stdin",
    ];
    let mut flags = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        assert!(
            !code.contains("approval"),
            "src/cli.rs:{}: {line}",
            number + 1
        );
        for (at, _) in code.match_indices("\"--") {
            let literal = &code[at + 1..];
            let flag = &literal[..literal.find('"').unwrap_or(literal.len())];
            assert!(
                declared.contains(&flag),
                "src/cli.rs:{}: undeclared flag `{flag}`",
                number + 1
            );
            flags.push(flag.to_owned());
        }
    }
    // The scan saw the flags the client does build.
    for flag in declared {
        assert!(flags.iter().any(|seen| seen == flag), "{flag} not found");
    }
}

#[test]
fn argv_holds_only_declared_flags() {
    let fixture = Fixture::new("argv_holds_only_declared_flags");
    fixture.answer("describe", GET, "describe-get.json");
    fixture.answer("invoke", GET, "invoke-get.json");
    let config = fixture.config();
    let (config_file, state) = (config.config.unwrap(), config.state_dir.unwrap());
    let cli = fixture.cli(Vec::new());

    cli.invoke_read(&adapter(), &connection(), &operation(GET), &input())
        .unwrap();

    let globals = |rest: &[&str]| -> Vec<String> {
        [
            "--output",
            "json",
            "--config",
            &config_file,
            "--state-dir",
            &state,
        ]
        .iter()
        .chain(rest)
        .map(|s| (*s).to_owned())
        .collect()
    };
    assert_eq!(
        fixture.argv(),
        vec![
            globals(&[
                "operations",
                "describe",
                "--adapter",
                ADAPTER,
                "--operation",
                GET
            ]),
            globals(&[
                "operations",
                "invoke",
                "--adapter",
                ADAPTER,
                "--connection",
                CONNECTION,
                "--operation",
                GET,
                "--schema",
                "sch-get-1",
                "--revision",
                "rev-1",
                "--input-stdin",
            ]),
        ]
    );

    // An id that would read as a flag starts no process.
    let runs = fixture.argv().len();
    for (adapter, connection, operation) in [
        ("--config", CONNECTION, GET),
        (ADAPTER, "--approval-file", GET),
        (ADAPTER, CONNECTION, "--input-file"),
        (ADAPTER, CONNECTION, ""),
        (ADAPTER, "conn fixture", GET),
    ] {
        let answer = cli.invoke_read(
            &AdapterAlias(adapter.to_owned()),
            &ConnectionId(connection.to_owned()),
            &OperationId(operation.to_owned()),
            &input(),
        );
        assert!(
            matches!(answer, Err(CliError::Failed(_))),
            "{adapter:?} {connection:?} {operation:?}: {answer:?}"
        );
    }
    assert_eq!(fixture.argv().len(), runs, "{:?}", fixture.argv());

    // A revision `describe` answers that would read as a flag is never passed to invoke.
    fixture.answer("describe", GET, "describe-flag-revision.json");
    let answer = cli.invoke_read(&adapter(), &connection(), &operation(GET), &input());
    assert!(matches!(answer, Err(CliError::Failed(_))), "{answer:?}");
    assert_eq!(fixture.runs("invoke").len(), 1, "{:?}", fixture.argv());
}

#[test]
fn environment_holds_no_token() {
    let fixture = Fixture::new("environment_holds_no_token");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-read.json");
    let secret = "fixture-secret-value";
    let mut environment = fixture.environment();
    for name in [
        "SLACK_BOT_TOKEN",
        "GITHUB_TOKEN",
        "CONNECTORS_TOKEN",
        "LOOM_API_KEY",
        "AWS_SECRET_ACCESS_KEY",
        "FIXTURE_PASSWORD",
        "CARGO_REGISTRY_TOKEN",
    ] {
        environment.push((name.to_owned(), secret.to_owned()));
    }
    let cli = ConnectorsCli::new(fixture.config(), Vec::new()).with_environment(environment);

    cli.invoke_read(&adapter(), &connection(), &operation(LIST), &input())
        .unwrap();

    let seen = fs::read_to_string(fixture.state().join("env.log")).unwrap();
    assert!(!seen.contains(secret), "{seen}");
    for line in seen.lines() {
        let name = line
            .split('=')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        for word in ["TOKEN", "SECRET", "KEY", "PASSWORD"] {
            assert!(!name.contains(word), "{line}");
        }
    }
    let home = format!("HOME={}", fixture.root.join("home").display());
    assert!(seen.lines().any(|line| line == home), "{seen}");
}

#[test]
fn not_granted_is_a_refusal() {
    let fixture = Fixture::new("not_granted_is_a_refusal");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("invoke", LIST, "invoke-not-granted.json");
    fixture.exit("invoke", 1);
    let declared = source("support-channel", Some(LIST), None, None);
    let cli = fixture.cli(vec![declared.clone()]);

    match cli.read(&declared, ReadKind::List, &input()) {
        Err(CliError::Refused(refusal)) => {
            assert_eq!(refusal.code, "not_granted");
            assert_eq!(
                refusal.source,
                Some(SourceName("support-channel".to_owned()))
            );
            assert_eq!(refusal.adapter, adapter());
            assert_eq!(refusal.operation, Some(operation(LIST)));
            assert!(
                refusal.message.contains("support-channel"),
                "{}",
                refusal.message
            );
            // The remedy, for the operator to run; the client never revalidates itself.
            assert!(
                refusal
                    .message
                    .contains("connectors connections revalidate --adapter chat"),
                "{}",
                refusal.message
            );
        }
        other => panic!("not_granted was not a refusal: {other:?}"),
    }
    assert!(
        fixture
            .argv()
            .iter()
            .all(|argv| !argv.iter().any(|argument| argument == "connections")),
        "{:?}",
        fixture.argv()
    );
    // Refused once, and not retried.
    assert_eq!(fixture.runs("invoke").len(), 1, "{:?}", fixture.argv());
}

#[test]
fn sources_lists_entities_with_schema() {
    let fixture = Fixture::new("sources_lists_entities_with_schema");
    fixture.answer("describe", LIST, "describe-list.json");
    fixture.answer("describe", SEARCH, "describe-search.json");
    fixture.answer("describe", GET, "describe-get.json");
    let history = source("support-channel", Some(LIST), None, Some(GET));
    let search = source("support-search", None, Some(SEARCH), None);
    let cli = fixture.cli(vec![history.clone(), search.clone()]);

    let listed = cli.sources().unwrap();

    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].0, history);
    assert_eq!(
        listed[0]
            .1
            .iter()
            .map(|(kind, entity)| (*kind, entity.operation.clone(), entity.schema.clone()))
            .collect::<Vec<_>>(),
        vec![
            (ReadKind::List, operation(LIST), "sch-list-1".to_owned()),
            (ReadKind::Get, operation(GET), "sch-get-1".to_owned()),
        ]
    );
    assert_eq!(listed[1].0, search);
    assert_eq!(listed[1].1.len(), 1);
    let (kind, entity) = &listed[1].1[0];
    assert_eq!(*kind, ReadKind::Search);
    assert_eq!(entity.revision, "rev-1");
    assert_eq!(
        entity.input_schema.member("required"),
        Some(&json::Value::Array(vec![json::Value::Text(
            "query".to_owned()
        )]))
    );
    assert_eq!(fixture.runs("describe").len(), 3, "{:?}", fixture.argv());
    assert_eq!(fixture.runs("invoke").len(), 0, "{:?}", fixture.argv());
}

#[test]
fn read_result_carries_the_audit_reference() {
    let fixture = Fixture::new("read_result_carries_the_audit_reference");
    fixture.answer("describe", LIST, "describe-list.json");
    let declared = source("support-channel", Some(LIST), None, None);
    let cli = fixture.cli(vec![declared.clone()]);

    fixture.answer("invoke", LIST, "invoke-read-audited.json");
    let audited = cli.read(&declared, ReadKind::List, &input()).unwrap();
    assert_eq!(
        audited,
        ReadResult {
            body: json::parse(r#"{"messages":[],"has_more":false}"#).unwrap(),
            audit_ref: Some("00000000-0000-4000-8000-0000000a0d17".to_owned()),
        }
    );

    fixture.answer("invoke", LIST, "invoke-read.json");
    let unaudited = cli.read(&declared, ReadKind::List, &input()).unwrap();
    assert_eq!(unaudited.audit_ref, None);
}
