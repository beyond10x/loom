//! Adversary cases for `story:connectors-cli-invoke-form` (wave 2026-10-30-w2, unit 3).
//!
//! Each case drives `ConnectorsCli` against the fake `tests/fixtures/connectors/fake-connectors`,
//! copied into the case's own directory under `CARGO_TARGET_TMPDIR/adversary_w20261030w2_u3`, and
//! asserts on the argv it logged. Describe and invoke answers are the shapes the existing fixtures
//! hold, edited in the case so each edit is visible beside its assertion.

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
use b10x_loom_connectors::cli::{CliError, ConnectorsCli, WriteError, json};

const CHAT: &str = "chat";
const CHAT_CONNECTION: &str = "conn-fixture";
const LIST: &str = "channel.history";
const DESK: &str = "desk";
const DESK_CONNECTION: &str = "conn-desk";
const ROUTE: &str = "ticket.route";
const NOW: &str = "2026-10-17T09:00:00Z";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(test: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("adversary_w20261030w2_u3")
            .join(test);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for directory in ["state", "home", "approvals"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let status = std::process::Command::new("/usr/bin/install")
            .args(["-m", "755"])
            .arg(fixtures().join("fake-connectors"))
            .arg(root.join("connectors"))
            .status()
            .unwrap();
        assert!(status.success());
        fs::write(root.join("connectors.toml"), "# not read by the fake\n").unwrap();
        Self { root }
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cli(&self) -> ConnectorsCli {
        ConnectorsCli::new(
            ConnectorsCliConfig {
                program: self.root.join("connectors").display().to_string(),
                config: Some(self.root.join("connectors.toml").display().to_string()),
                state_dir: Some(self.state().display().to_string()),
                timeout_seconds: None,
            },
            Vec::new(),
        )
        .with_environment([
            (
                "HOME".to_owned(),
                self.root.join("home").display().to_string(),
            ),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ])
    }

    fn proof(&self) -> String {
        self.root
            .join("approvals/route.proof")
            .display()
            .to_string()
    }

    /// `verb` (`describe` or `invoke`) of `operation` answers `text`.
    fn answer_text(&self, verb: &str, operation: &str, text: &str) {
        fs::write(self.state().join(format!("{verb}.{operation}.json")), text).unwrap();
    }

    fn answer(&self, verb: &str, operation: &str, file: &str) {
        self.answer_text(verb, operation, &fixture_text(file));
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

    fn invokes(&self) -> Vec<Vec<String>> {
        self.argv()
            .into_iter()
            .filter(|argv| command(argv).first().map(String::as_str) == Some("invoke"))
            .collect()
    }

    fn describes(&self) -> usize {
        self.argv()
            .iter()
            .filter(|argv| command(argv).starts_with(&["operations".into(), "describe".into()]))
            .count()
    }
}

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

/// `text` with `from` replaced by `to`, asserting `from` was there.
fn edited(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the fixture changed: {text}");
    text.replacen(from, to, 1)
}

fn read_input() -> json::Value {
    json::parse(r#"{"channel":"C0FIXTURE1","limit":20}"#).unwrap()
}

fn read(fixture: &Fixture) -> Result<b10x_loom_connectors::cli::datasource::ReadResult, CliError> {
    fixture.cli().invoke_read(
        &AdapterAlias(CHAT.to_owned()),
        &ConnectionId(CHAT_CONNECTION.to_owned()),
        &OperationId(LIST.to_owned()),
        &read_input(),
    )
}

fn route() -> WriteOperation {
    WriteOperation {
        action: ROUTE.to_owned(),
        adapter: AdapterAlias(DESK.to_owned()),
        connection: ConnectionId(DESK_CONNECTION.to_owned()),
        operation: OperationId(ROUTE.to_owned()),
    }
}

fn write_input() -> json::Value {
    json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap()
}

fn approval(path: impl Into<String>) -> WriteApproval {
    WriteApproval {
        proof_file: path.into(),
    }
}

// ---------------------------------------------------------------------------------------------
// Reads.

/// A read of an operation described with the `mutation` profile is refused `write-operation`,
/// unchanged by the target check, even when the describe names no target: the profile refusal
/// comes first. Catches the mutant that checks the target before the profile.
#[test]
fn adv_u3_a_mutation_read_without_a_target_is_still_refused_as_a_write() {
    let fixture = Fixture::new("mutation_read_without_target");
    let described = edited(
        &fixture_text("describe-route.json"),
        r#""connector":"desk","instance":"support","#,
        "",
    );
    fixture.answer_text("describe", ROUTE, &described);
    fixture.answer("invoke", ROUTE, "invoke-route-applied.json");

    let answer = fixture.cli().invoke_read(
        &AdapterAlias(DESK.to_owned()),
        &ConnectionId(DESK_CONNECTION.to_owned()),
        &OperationId(ROUTE.to_owned()),
        &write_input(),
    );

    match answer {
        Err(CliError::Refused(refusal)) => assert_eq!(refusal.code, "write-operation"),
        other => panic!("expected the write-operation refusal, got {other:?}"),
    }
    assert_eq!(fixture.describes(), 1, "{:?}", fixture.argv());
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
}

/// `describe` and `sources` need no target: only an invoke does.
#[test]
fn adv_u3_describe_answers_without_a_target() {
    let fixture = Fixture::new("describe_without_target");
    let described = edited(
        &fixture_text("describe-list.json"),
        r#""connector":"chat","instance":"team","#,
        "",
    );
    fixture.answer_text("describe", LIST, &described);

    let entity = fixture
        .cli()
        .describe(
            &AdapterAlias(CHAT.to_owned()),
            &OperationId(LIST.to_owned()),
        )
        .unwrap();

    assert_eq!(entity.schema, "sch-list-1");
    assert_eq!(entity.revision, "rev-1");
    assert_eq!(fixture.argv().len(), 1, "{:?}", fixture.argv());
}

/// A connector or instance that is no string, or a string holding a control character or a
/// whitespace character outside ASCII, starts no invoke and fails naming the member.
#[test]
fn adv_u3_unusable_target_parts_start_no_invoke() {
    let fixture = Fixture::new("unusable_target_parts");
    fixture.answer("invoke", LIST, "invoke-read.json");
    let described = fixture_text("describe-list.json");
    let connector = r#""connector":"chat""#;
    let instance = r#""instance":"team""#;
    let cases = [
        (
            edited(&described, connector, r#""connector":null"#),
            "connector",
        ),
        (
            edited(&described, connector, r#""connector":["chat"]"#),
            "connector",
        ),
        (
            edited(&described, connector, r#""connector":{"id":"chat"}"#),
            "connector",
        ),
        (
            edited(&described, connector, r#""connector":true"#),
            "connector",
        ),
        (
            edited(&described, instance, r#""instance":"team\u0007""#),
            "instance",
        ),
        (
            edited(&described, instance, r#""instance":"team\u0000""#),
            "instance",
        ),
        (
            edited(&described, instance, r#""instance":"te am""#),
            "instance",
        ),
        (
            edited(&described, instance, r#""instance":"team ""#),
            "instance",
        ),
        (
            edited(&described, instance, r#""instance":"-team""#),
            "instance",
        ),
        (
            edited(&described, instance, r#""instance":"/""#),
            "instance",
        ),
    ];

    for (answer, named) in &cases {
        fixture.answer_text("describe", LIST, answer);
        match read(&fixture) {
            Err(CliError::Failed(why)) => assert!(why.contains(named), "{named}: {why}"),
            other => panic!("{answer}: the read was not refused: {other:?}"),
        }
    }
    assert_eq!(fixture.describes(), cases.len(), "{:?}", fixture.argv());
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
}

/// A refused invoke is never retried: Connectors' refusal code comes back after exactly one
/// invoke, for a parse refusal (exit 2) and for `v0.60.0`'s `stale_description` (exit 1).
#[test]
fn adv_u3_a_refused_read_invoke_runs_once() {
    for (status, code) in [(2, "cli_parse"), (1, "stale_description")] {
        let fixture = Fixture::new(&format!("refused_read_invoke_{code}"));
        fixture.answer("describe", LIST, "describe-list.json");
        fixture.answer_text(
            "invoke",
            LIST,
            &format!(r#"{{"ok":false,"error":{{"code":"failure","data":{{"code":"{code}"}}}}}}"#),
        );
        fixture.exit("invoke", status);

        match read(&fixture) {
            Err(CliError::Refused(refusal)) => assert_eq!(refusal.code, code),
            other => panic!("{code}: expected a refusal, got {other:?}"),
        }
        assert_eq!(fixture.describes(), 1, "{code}: {:?}", fixture.argv());
        assert_eq!(fixture.invokes().len(), 1, "{code}: {:?}", fixture.argv());
    }
}

/// The target is taken from the describe answer, so the answer's tie to the alias Loom asked
/// about is what keeps the invoke on that alias's instance. Connectors `v0.60.0` declares
/// `adapter` required in `connectors.cli.OperationDescribeResult`; an answer that drops it
/// names a target Loom cannot tie to the alias, and should start no invoke. The client checks
/// `adapter` only when present (`src/cli.rs`, `described`), so it invokes the unbound target.
#[test]
fn adv_u3_a_describe_without_the_adapter_echo_starts_no_invoke() {
    let fixture = Fixture::new("describe_without_adapter_echo");
    let described = edited(
        &fixture_text("describe-list.json"),
        r#""adapter":"chat","connector":"chat","instance":"team""#,
        r#""connector":"other","instance":"elsewhere""#,
    );
    fixture.answer_text("describe", LIST, &described);
    fixture.answer("invoke", LIST, "invoke-read.json");

    let answer = read(&fixture);

    let invoked: Vec<String> = fixture
        .invokes()
        .iter()
        .map(|argv| command(argv).get(1).cloned().unwrap_or_default())
        .collect();
    assert!(
        invoked.is_empty() && matches!(answer, Err(CliError::Failed(_))),
        "a describe naming no adapter steered the read at {invoked:?}: {answer:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Writes.

/// A write whose describe reports a read profile is refused `not-a-write`, unchanged by the
/// target check, even when the describe names no target. Catches the mutant that checks the
/// target before the profile.
#[test]
fn adv_u3_a_read_profile_write_without_a_target_is_still_not_a_write() {
    let fixture = Fixture::new("read_profile_write_without_target");
    let described = edited(
        &fixture_text("describe-route-read.json"),
        r#""connector":"desk","instance":"support","#,
        "",
    );
    fixture.answer_text("describe", ROUTE, &described);
    fixture.answer("invoke", ROUTE, "invoke-route-applied.json");

    let answer = fixture.cli().invoke_write(
        &admitted(1),
        &route(),
        &write_input(),
        &approval(fixture.proof()),
    );

    match answer {
        Err(WriteError::Refused(refusal)) => assert_eq!(refusal.code, "not-a-write"),
        other => panic!("expected not-a-write, got {other:?}"),
    }
    assert_eq!(fixture.describes(), 1, "{:?}", fixture.argv());
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
}

/// An action or input the runtime did not admit starts no process, whatever the describe would
/// have named.
#[test]
fn adv_u3_unadmitted_writes_start_no_process() {
    let fixture = Fixture::new("unadmitted_writes");
    fixture.answer("describe", ROUTE, "describe-route.json");
    fixture.answer("invoke", ROUTE, "invoke-route-applied.json");
    let request = admitted(1);
    let mut other_action = route();
    other_action.action = "ticket.close".to_owned();

    for (operation, input, code) in [
        (other_action, write_input(), "action-not-admitted"),
        (
            route(),
            json::parse(r#"{"ticket":"48213","queue":"other"}"#).unwrap(),
            "input-not-admitted",
        ),
    ] {
        match fixture
            .cli()
            .invoke_write(&request, &operation, &input, &approval(fixture.proof()))
        {
            Err(WriteError::Refused(refusal)) => assert_eq!(refusal.code, code),
            other => panic!("{code}: expected a refusal, got {other:?}"),
        }
    }
    assert!(fixture.argv().is_empty(), "{:?}", fixture.argv());
}

/// A write's target parts are held to the same rules as a read's: a non-string or a part
/// holding `/` starts no invoke.
#[test]
fn adv_u3_a_write_with_an_unusable_target_part_starts_no_invoke() {
    let fixture = Fixture::new("write_unusable_target_part");
    fixture.answer("invoke", ROUTE, "invoke-route-applied.json");
    let described = fixture_text("describe-route.json");
    let cases = [
        (
            edited(&described, r#""connector":"desk""#, r#""connector":3"#),
            "connector",
        ),
        (
            edited(
                &described,
                r#""instance":"support""#,
                r#""instance":"support/x""#,
            ),
            "instance",
        ),
        (
            edited(&described, r#""instance":"support""#, r#""instance":"""#),
            "instance",
        ),
    ];

    for (answer, named) in &cases {
        fixture.answer_text("describe", ROUTE, answer);
        match fixture.cli().invoke_write(
            &admitted(1),
            &route(),
            &write_input(),
            &approval(fixture.proof()),
        ) {
            Err(WriteError::Failed(why)) => assert!(why.contains(named), "{named}: {why}"),
            other => panic!("{answer}: the write was not refused: {other:?}"),
        }
    }
    assert_eq!(fixture.describes(), cases.len(), "{:?}", fixture.argv());
    assert!(fixture.invokes().is_empty(), "{:?}", fixture.argv());
}

/// A write refused by Connectors before any provider call (`v0.60.0`: `stale_description`,
/// `approval_required`, a parse refusal) runs exactly one invoke and carries the code.
#[test]
fn adv_u3_a_refused_write_invoke_runs_once() {
    for (status, code) in [
        (1, "stale_description"),
        (1, "approval_required"),
        (2, "cli_parse"),
    ] {
        let fixture = Fixture::new(&format!("refused_write_invoke_{code}"));
        fixture.answer("describe", ROUTE, "describe-route.json");
        fixture.answer_text(
            "invoke",
            ROUTE,
            &format!(r#"{{"ok":false,"error":{{"code":"failure","data":{{"code":"{code}"}}}}}}"#),
        );
        fixture.exit("invoke", status);

        let written = fixture
            .cli()
            .invoke_write(
                &admitted(1),
                &route(),
                &write_input(),
                &approval(fixture.proof()),
            )
            .unwrap();

        assert_eq!(written.code.as_deref(), Some(code));
        assert_eq!(fixture.describes(), 1, "{code}: {:?}", fixture.argv());
        assert_eq!(fixture.invokes().len(), 1, "{code}: {:?}", fixture.argv());
        let invoke = command(&fixture.invokes()[0]).to_vec();
        assert_eq!(
            &invoke[..3],
            ["invoke", "desk/support", ROUTE],
            "{invoke:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The admitted request, made by the runtime (as in `connectors_cli_write.rs`).

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

fn admitted(first: u64) -> AdmittedRequest {
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
            arguments: ProposedActionArguments(
                commission_json::parse(r#"{"ticket":"48213","queue":"billing"}"#).unwrap(),
            ),
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
            requests: first - 1,
            observations: 0,
        },
    );
    let mut kept = keep.0.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(kept.len(), 1, "the runtime admits the route once");
    kept.remove(0)
}
