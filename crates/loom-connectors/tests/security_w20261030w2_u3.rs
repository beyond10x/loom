//! Security review of `story:connectors-cli-invoke-form` in `ConnectorsCli`: the invoke target
//! `<connector>/<instance>` is read from the answer a child program (`operations describe`)
//! printed and placed in the argv of a second process.
//!
//! Integrity checks against the change's own contract (AGENTS.md § Connectors, `src/cli.rs`
//! module docs):
//!
//! 1. no `connector` or `instance` a describe answers becomes a flag or an extra positional of the
//!    invoke, for a read or a governed write, and the invoke starts with only the variables
//!    `cli::INHERITED` names;
//! 2. a governed write takes its connection, approval proof and idempotency key from the admitted
//!    request and the host's configuration, never from the describe, and its target only from an
//!    answer about the configured alias;
//! 3. a read whose describe reports the `mutation` profile starts no invoke, whatever target the
//!    answer names, and no read carries `--approval-file` or `--idempotency-key`.
//!
//! The program is the fake `connectors` of `connectors_cli.rs`, copied into each test's directory
//! under `CARGO_TARGET_TMPDIR/security_w20261030w2_u3`; the admitted request is made by
//! Commission's own `run_until_blocked`, as in `connectors_cli_write.rs`. Describe answers are the
//! existing fixtures with the members under test rewritten in each test.

use std::fs;
use std::iter::repeat_n;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_connectors::cli::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, OperationId, WriteApproval, WriteOperation,
};
use b10x_loom_connectors::cli::{
    CliError, ConnectorsCli, INHERITED, WriteError, idempotency_key, json,
};

/// The read side: `channel.history` of alias `chat`, described as `chat/team`.
const READ_ADAPTER: &str = "chat";
const READ_CONNECTION: &str = "conn-fixture";
const LIST: &str = "channel.history";
const POST: &str = "message.post";

/// The write side: `ticket.route` of alias `desk`, described as `desk/support`.
const WRITE_ADAPTER: &str = "desk";
const WRITE_CONNECTION: &str = "conn-desk";
const ROUTE: &str = "ticket.route";
const ARGUMENTS: &str = r#"{"ticket":"48213","queue":"billing"}"#;
const NOW: &str = "2026-10-17T09:00:00Z";

/// A value standing in for a credential in the client's environment; it must reach no program.
const SECRET: &str = "sec-u3-fixture-secret-value";

/// Variables the fake's `/bin/sh` sets itself, so they are in its `env` whatever it was given.
const SHELL_SET: &[&str] = &["PWD", "SHLVL", "_", "OLDPWD"];

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("security_w20261030w2_u3")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home", "approvals"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        // Installed by a child process, so this one never holds a writable descriptor on a
        // program a parallel test runs ("Text file busy"; see `connectors_cli.rs`).
        let status = std::process::Command::new("/usr/bin/install")
            .args(["-m", "755"])
            .arg(fixtures().join("fake-connectors"))
            .arg(root.join("connectors"))
            .status()
            .unwrap();
        assert!(status.success(), "install: {status}");
        fs::write(root.join("connectors.toml"), "# not read by the fake\n").unwrap();
        Self { root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn config_file(&self) -> String {
        self.root.join("connectors.toml").display().to_string()
    }

    /// A client handed an environment holding credential-looking variables beside `HOME` and
    /// `PATH`.
    fn cli(&self) -> ConnectorsCli {
        let mut environment = vec![
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ];
        for name in [
            "CONNECTORS_TOKEN",
            "CONNECTORS_KEYRING_PASSPHRASE",
            "LOOM_API_KEY",
            "OAUTH_CLIENT_SECRET",
            "FIXTURE_PASSWORD",
            "SSH_AUTH_SOCK",
        ] {
            environment.push((name.to_owned(), SECRET.to_owned()));
        }
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: Some(self.config_file()),
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds: None,
            },
            Vec::new(),
        )
        .with_environment(environment)
    }

    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    /// `verb` (`describe` or `invoke`) of `operation` answers `document`.
    fn answer(&self, verb: &str, operation: &str, document: &str) {
        fs::write(
            self.state().join(format!("{verb}.{operation}.json")),
            document,
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

    /// The runs of the command `invoke`.
    fn invokes(&self) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| command(argv).first().is_some_and(|word| word == "invoke"))
            .collect()
    }

    /// The global flags every run starts with.
    fn globals(&self) -> Vec<String> {
        vec![
            "--output".to_owned(),
            "json".to_owned(),
            "--config".to_owned(),
            self.config_file(),
            "--state-dir".to_owned(),
            self.state().display().to_string(),
        ]
    }

    /// The last run's environment holds only [`INHERITED`] names (and what its shell sets), and
    /// no credential value.
    fn assert_inherited_only(&self, case: &str) {
        let seen = fs::read_to_string(self.state().join("env.log")).unwrap();
        assert!(
            !seen.contains(SECRET),
            "{case}: a credential reached the program: {seen}"
        );
        for line in seen.lines() {
            let name = line.split('=').next().unwrap_or_default();
            assert!(
                INHERITED.contains(&name) || SHELL_SET.contains(&name),
                "{case}: the program started with `{name}`, which `cli::INHERITED` does not name"
            );
        }
    }
}

/// The words of `argv` after the global flags and their values.
fn command(argv: &[String]) -> &[String] {
    let mut at = 0;
    while matches!(
        argv.get(at).map(String::as_str),
        Some("--output" | "--config" | "--state-dir")
    ) {
        at += 2;
    }
    &argv[at.min(argv.len())..]
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/connectors")
}

fn fixture_text(file: &str) -> String {
    fs::read_to_string(fixtures().join(file)).unwrap()
}

/// `value` as a JSON string literal.
fn quoted(value: &str) -> String {
    let mut out = String::new();
    json::push_text(&mut out, value);
    out
}

/// Values a describe answer could put in `connector` or `instance`: flag spellings, `=`,
/// whitespace and control characters (NUL among them), `/` and path words, Unicode lookalikes of
/// `-` and `/`, invisible format characters, and lengths past Connectors' 128-byte id and past
/// the kernel's bound on one argv string.
fn hostile() -> Vec<String> {
    let mut values: Vec<String> = [
        "-x",
        "-",
        "--",
        "--approval-file",
        "--connection=conn-other",
        "--idempotency-key",
        "=",
        "a=b",
        "x=--approval-file",
        "",
        " ",
        "a b",
        "a\tb",
        "a\nb",
        "a\rb",
        "a\0b",
        "a\u{1b}b",
        "a\u{85}b",
        "\u{a0}a",
        "a\u{2028}b",
        "a\u{3000}b",
        "/",
        "a/b",
        "../x",
        "..",
        ".",
        "./",
        "a\u{2215}b",
        "a\u{ff0f}b",
        "a\u{2044}b",
        "\u{2010}x",
        "\u{2212}x",
        "\u{ff0d}x",
        "\u{fe63}x",
        "\u{202e}x",
        "\u{200b}x",
        "\u{feff}x",
        "x\u{ad}",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    values.push("a".repeat(4096));
    values.push("a".repeat(200_000));
    values
}

/// Whether the change's argv rule must refuse `value` as a target part before any invoke: empty,
/// read as a flag, holding whitespace, a control character or `/`.
fn must_refuse(value: &str) -> bool {
    value.is_empty()
        || value.starts_with('-')
        || value.contains('/')
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

/// Every (connector, instance) pair with one hostile part and the fixture's other part.
fn cases(connector: &str, instance: &str) -> Vec<(String, String)> {
    hostile()
        .into_iter()
        .flat_map(|value| {
            [
                (value.clone(), instance.to_owned()),
                (connector.to_owned(), value),
            ]
        })
        .collect()
}

/// A short label for a case in an assertion message.
fn label(connector: &str, instance: &str) -> String {
    let short = |value: &str| {
        if value.len() > 40 {
            format!("<{} bytes>", value.len())
        } else {
            format!("{value:?}")
        }
    };
    format!(
        "connector {} instance {}",
        short(connector),
        short(instance)
    )
}

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime (as `connectors_cli_write.rs` makes it).

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(NOW.to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

#[derive(Default)]
struct Keep(Mutex<Vec<AdmittedRequest>>);

impl EffectPort for Keep {
    fn performs(&self, action: &str) -> bool {
        action == ROUTE
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: commission_json::Value::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

fn admitted() -> AdmittedRequest {
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x90)),
        case_id: CaseId("case-route".to_owned()),
        principal: PrincipalId("principal-desk".to_owned()),
        authority_context: AuthorityContext(commission_json::Value::Null),
    });
    let open = |revision| {
        Answer::at(revision).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: ROUTE.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        )
    };
    let governor = FakeGovernor::new();
    governor.script(
        commission.data().case_id.clone(),
        repeat_n(open(3), 5).chain([open(4), open(4).complete("routed")]),
    );
    let executor = ScriptedExecutor::new(vec![ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: ROUTE.to_owned(),
            arguments: ProposedActionArguments(commission_json::parse(ARGUMENTS).unwrap()),
        },
    )]);
    let keep = Keep::default();
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(uuid(0x100 + issued))
    }));
    let _ = run_until_blocked(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &keep,
        &commission,
        &mut runs,
        &mut Context {
            requests: 0,
            observations: 0,
        },
    );
    let mut kept = keep.0.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(kept.len(), 1, "the runtime admits the route once");
    kept.remove(0)
}

fn route() -> WriteOperation {
    WriteOperation {
        action: ROUTE.to_owned(),
        adapter: AdapterAlias(WRITE_ADAPTER.to_owned()),
        connection: ConnectionId(WRITE_CONNECTION.to_owned()),
        operation: OperationId(ROUTE.to_owned()),
    }
}

fn write_input() -> json::Value {
    json::parse(ARGUMENTS).unwrap()
}

// ---------------------------------------------------------------------------------------------
// 1. No describe value becomes a flag or an extra positional.

/// A read: every hostile `connector` or `instance` either starts no invoke (and a value the argv
/// rule names is always refused so), or reaches the invoke as one half of the one target element,
/// with argv exactly the declared command line, no approval flag, and only inherited variables.
#[test]
fn no_describe_value_becomes_a_read_flag_or_positional() {
    let fixture = Fixture::new("no_describe_value_becomes_a_read_flag_or_positional");
    let described = fixture_text("describe-list.json");
    fixture.answer("invoke", LIST, &fixture_text("invoke-read.json"));
    let cli = fixture.cli();

    for (connector, instance) in cases("chat", "team") {
        let case = label(&connector, &instance);
        let answer = described
            .replace(
                r#""connector":"chat""#,
                &format!(r#""connector":{}"#, quoted(&connector)),
            )
            .replace(
                r#""instance":"team""#,
                &format!(r#""instance":{}"#, quoted(&instance)),
            );
        assert_ne!(answer, described, "{case}: the fixture changed");
        fixture.answer("describe", LIST, &answer);
        let before = fixture.invokes().len();

        let read = cli.invoke_read(
            &AdapterAlias(READ_ADAPTER.to_owned()),
            &ConnectionId(READ_CONNECTION.to_owned()),
            &OperationId(LIST.to_owned()),
            &write_input(),
        );

        let invokes = fixture.invokes();
        let refuse = must_refuse(&connector) || must_refuse(&instance);
        match read {
            Err(CliError::Failed(_)) => {
                assert_eq!(
                    invokes.len(),
                    before,
                    "{case}: a failed read started an invoke"
                );
            }
            Ok(_) if !refuse => {
                assert_eq!(invokes.len(), before + 1, "{case}: not exactly one invoke");
                let target = format!("{connector}/{instance}");
                let mut expected = fixture.globals();
                expected.extend(
                    [
                        "invoke",
                        target.as_str(),
                        LIST,
                        "--connection",
                        READ_CONNECTION,
                        "--schema",
                        "sch-list-1",
                        "--revision",
                        "rev-1",
                        "--input-stdin",
                    ]
                    .map(str::to_owned),
                );
                assert_eq!(invokes[before], expected, "{case}: the invoke's argv");
                assert_eq!(target.matches('/').count(), 1, "{case}: {target}");
                fixture.assert_inherited_only(&case);
            }
            other => panic!("{case}: {other:?}"),
        }
    }
    for argv in fixture.argv() {
        assert!(
            !argv
                .iter()
                .any(|word| word.starts_with("--approval-file")
                    || word.starts_with("--idempotency-key")),
            "a read carried a write's flag: {argv:?}"
        );
    }
}

/// A governed write: the same hostile values either start no invoke, or reach it as one half of
/// the target with the configured connection, the admitted proof path and key in their places.
#[test]
fn no_describe_value_becomes_a_write_flag_or_positional() {
    let fixture = Fixture::new("no_describe_value_becomes_a_write_flag_or_positional");
    let described = fixture_text("describe-route.json");
    fixture.answer("invoke", ROUTE, &fixture_text("invoke-route-applied.json"));
    let cli = fixture.cli();
    let request = admitted();
    let key = idempotency_key(&request);
    let proof = fixture.proof();

    for (connector, instance) in cases("desk", "support") {
        let case = label(&connector, &instance);
        let answer = described
            .replace(
                r#""connector":"desk""#,
                &format!(r#""connector":{}"#, quoted(&connector)),
            )
            .replace(
                r#""instance":"support""#,
                &format!(r#""instance":{}"#, quoted(&instance)),
            );
        assert_ne!(answer, described, "{case}: the fixture changed");
        fixture.answer("describe", ROUTE, &answer);
        let before = fixture.invokes().len();

        let written = cli.invoke_write(
            &request,
            &route(),
            &write_input(),
            &WriteApproval {
                proof_file: proof.clone(),
            },
        );

        let invokes = fixture.invokes();
        let refuse = must_refuse(&connector) || must_refuse(&instance);
        match written {
            Err(WriteError::Failed(_)) => {
                assert_eq!(
                    invokes.len(),
                    before,
                    "{case}: a failed write started an invoke"
                );
            }
            Ok(_) if !refuse => {
                assert_eq!(invokes.len(), before + 1, "{case}: not exactly one invoke");
                let target = format!("{connector}/{instance}");
                let mut expected = fixture.globals();
                expected.extend(
                    [
                        "invoke",
                        target.as_str(),
                        ROUTE,
                        "--connection",
                        WRITE_CONNECTION,
                        "--schema",
                        "sch-route-1",
                        "--revision",
                        "rev-7",
                        "--input-stdin",
                        "--approval-file",
                        proof.as_str(),
                        "--idempotency-key",
                        key.as_str(),
                    ]
                    .map(str::to_owned),
                );
                assert_eq!(invokes[before], expected, "{case}: the invoke's argv");
                fixture.assert_inherited_only(&case);
            }
            other => panic!("{case}: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 2. A governed write stays bound to what was configured and admitted.

/// The connection, proof path and key a write carries are the configured and admitted ones,
/// whatever members of those names the describe answer adds.
#[test]
fn a_write_takes_no_connection_proof_or_key_from_the_describe() {
    let fixture = Fixture::new("a_write_takes_no_connection_proof_or_key_from_the_describe");
    let described = fixture_text("describe-route.json");
    let answer = described.replace(
        r#""adapter":"desk","#,
        r#""adapter":"desk","connection":"conn-other","approval_file":"/elsewhere/other.proof","idempotency_key":"other-key","#,
    );
    assert_ne!(answer, described, "the fixture changed");
    fixture.answer("describe", ROUTE, &answer);
    fixture.answer("invoke", ROUTE, &fixture_text("invoke-route-applied.json"));
    let request = admitted();
    let proof = fixture.proof();

    fixture
        .cli()
        .invoke_write(
            &request,
            &route(),
            &write_input(),
            &WriteApproval {
                proof_file: proof.clone(),
            },
        )
        .unwrap();

    let invokes = fixture.invokes();
    assert_eq!(invokes.len(), 1, "{:?}", fixture.argv());
    let invoke = command(&invokes[0]);
    let after = |flag: &str| -> Vec<&str> {
        invoke
            .windows(2)
            .filter(|w| w[0] == flag)
            .map(|w| w[1].as_str())
            .collect()
    };
    assert_eq!(after("--connection"), [WRITE_CONNECTION]);
    assert_eq!(after("--approval-file"), [proof.as_str()]);
    assert_eq!(
        after("--idempotency-key"),
        [idempotency_key(&request).as_str()]
    );
}

/// A describe that answers about another alias than the configured one is never invoked: the
/// `adapter` member is what ties the answer's `connector` and `instance` to the alias.
#[test]
fn a_write_whose_describe_names_another_alias_is_never_invoked() {
    let fixture = Fixture::new("a_write_whose_describe_names_another_alias_is_never_invoked");
    let described = fixture_text("describe-route.json");
    let answer = described
        .replace(r#""adapter":"desk""#, r#""adapter":"desk-other""#)
        .replace(r#""instance":"support""#, r#""instance":"elsewhere""#);
    assert_ne!(answer, described, "the fixture changed");
    fixture.answer("describe", ROUTE, &answer);
    fixture.answer("invoke", ROUTE, &fixture_text("invoke-route-applied.json"));

    let written = fixture.cli().invoke_write(
        &admitted(),
        &route(),
        &write_input(),
        &WriteApproval {
            proof_file: fixture.proof(),
        },
    );

    assert!(
        matches!(&written, Err(WriteError::Failed(why)) if why.contains("another adapter")),
        "{written:?}"
    );
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
}

/// The target is the `connector` and `instance` the describe names "beside the alias" (AGENTS.md
/// § Connectors; Connectors `v0.60.0` `connectors.cli.OperationDescribeResult` requires
/// `adapter`, and `apps/connectors/src/local/select.rs` `fill` names a connector and instance
/// only beside an `adapter`). An answer that names a target and no alias is no answer about the
/// configured alias, so a governed write must not take its target from it.
#[test]
fn a_write_takes_no_target_from_a_describe_that_names_no_alias() {
    let fixture = Fixture::new("a_write_takes_no_target_from_a_describe_that_names_no_alias");
    let described = fixture_text("describe-route.json");
    let answer = described
        .replace(r#""adapter":"desk","#, "")
        .replace(r#""instance":"support""#, r#""instance":"elsewhere""#);
    assert_ne!(answer, described, "the fixture changed");
    fixture.answer("describe", ROUTE, &answer);
    fixture.answer("invoke", ROUTE, &fixture_text("invoke-route-applied.json"));

    let written = fixture.cli().invoke_write(
        &admitted(),
        &route(),
        &write_input(),
        &WriteApproval {
            proof_file: fixture.proof(),
        },
    );

    assert!(
        fixture.invokes().is_empty(),
        "a write was invoked on a target from an answer naming no alias: {:?} -> {written:?}",
        fixture.invokes()
    );
    assert!(matches!(written, Err(WriteError::Failed(_))), "{written:?}");
}

// ---------------------------------------------------------------------------------------------
// 3. A mutation-profile read starts no invoke, whatever target it names.

/// The profile is refused before the target is looked at: a `mutation` read is `write-operation`
/// whether its target is the fixture's, another instance, flag-shaped or absent.
#[test]
fn a_mutation_read_starts_no_invoke_whatever_target_it_names() {
    let fixture = Fixture::new("a_mutation_read_starts_no_invoke_whatever_target_it_names");
    let described = fixture_text("describe-write.json");
    fixture.answer("invoke", POST, &fixture_text("invoke-read.json"));
    let cli = fixture.cli();
    let targets = [
        described.clone(),
        described.replace(r#""instance":"team""#, r#""instance":"elsewhere""#),
        described.replace(r#""connector":"chat""#, r#""connector":"--approval-file""#),
        described
            .replace(r#","connector":"chat""#, "")
            .replace(r#","instance":"team""#, ""),
    ];

    for answer in &targets {
        fixture.answer("describe", POST, answer);
        match cli.invoke_read(
            &AdapterAlias(READ_ADAPTER.to_owned()),
            &ConnectionId(READ_CONNECTION.to_owned()),
            &OperationId(POST.to_owned()),
            &write_input(),
        ) {
            Err(CliError::Refused(refusal)) => assert_eq!(refusal.code, "write-operation"),
            other => panic!("{answer}: a mutation read was not refused: {other:?}"),
        }
    }
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
    assert_eq!(fixture.argv().len(), targets.len(), "{:?}", fixture.argv());
}
