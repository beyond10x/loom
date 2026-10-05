//! Loom chooses, the slice performs, and only a trusted verifier turns what happened into evidence
//! (story `selector-executor`; Atlas ADR 0074).
//!
//! The fixture workspace is a git repository under `CARGO_TARGET_TMPDIR` with one failing check:
//! the configured test command is `grep -qx fixed check.txt` (no shell; `grep` is on every Linux
//! runner), and `check.txt` holds `broken`. A sibling directory, `outside`, is not part of the
//! workspace. The fixture's own git calls run with no system or global configuration and give the
//! repository a local identity, which the executor's commits use. The git that the executor and
//! `case::open` run reads the operator's normal git configuration, as the slice does in use; the
//! fixture's local settings take precedence over it.
//!
//! The model is a recording fake `llm_core::Model`: it answers each request with the next recorded
//! reply, as a forced call of the one tool the request names (`ToolChoice::Named`). A selection is
//! the call's arguments `{"action": <id>}`; an argument reply is the action's arguments verbatim:
//!
//! - `repository.inspect`: `{"paths": [<path>, ...]}`;
//! - `repository.edit`: `{"files": [{"path": <path>, "contents": <text>}, ...], "message": <text>}`;
//! - `tests.run`: `{}`.
//!
//! The recorded run: inspect; tests.run while the check still fails, under a model answer claiming
//! it passes; an edit that reaches outside the workspace; the edit that fixes the check; tests.run;
//! and a choice of `repository.merge` once the governor holds the implementation verified.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EvidenceData, ExecutorOutcome, ExecutorOutcomeProposedAction, PrincipalId,
    ProposedActionArguments, Truth, commission_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor as _;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_executor::Loom;
use b10x_loom_intake_references::{ReferenceKind, references};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::executor::{ExecuteError, LocalExecutor, Report, TestCommand};
use b10x_loom_intake_slice::selector::{Briefing, ModelArguments, ModelSelector};
use b10x_loom_intake_slice::verifier::{TestResultVerifier, VerifyError};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolChoice, TurnObservation, TurnOutcome, TurnRequest,
};
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::json;

const PICK: &str = "software-change@1";
/// The intent names a tracker key and a GitHub issue in mixed case, so the issue's canonical value
/// is not a substring of the intent: a selector that only quotes the intent does not carry it.
const INTENT: &str = "Make the failing check pass (OPS-42), tracked in HTTPS://GitHub.com/Beyond10x/Intake/issues/7.";
const PRODUCER: &str = "intake-slice-test-verifier";
const CLAIM: &str = "I ran the tests: test_result pass, exit status 0, all tests pass.";

#[test]
fn a_failing_test_is_edited_and_then_passes() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    let outside = fixture.outside();
    let first = fixture.head();

    let found = references(INTENT);
    assert!(
        found
            .iter()
            .any(|reference| reference.kind == ReferenceKind::JiraIssue
                && reference.value == "OPS-42"),
        "{found:?}"
    );
    let canonical_only: Vec<&str> = found
        .iter()
        .filter(|reference| reference.kind == ReferenceKind::GithubIssue)
        .map(|reference| reference.value.as_str())
        .filter(|value| !INTENT.contains(value))
        .collect();
    assert_eq!(
        canonical_only.len(),
        1,
        "the GitHub issue's canonical value is not in the intent text: {found:?}"
    );

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, workspace).expect("the case opens");
    let commission = commission(&case);

    let model = Recorded::new(vec![
        // 1. inspect, under a selection that claims the tests already pass.
        Reply::select("repository.inspect", Some(CLAIM)),
        Reply::arguments(json!({"paths": ["check.txt"]}), None),
        // 2. tests.run while the check still fails, under answers that claim it passes.
        Reply::select("tests.run", Some(CLAIM)),
        Reply::arguments(json!({}), Some(CLAIM)),
        // 3. an edit with one path inside and one outside the workspace.
        Reply::select("repository.edit", None),
        Reply::arguments(
            json!({
                "files": [
                    {"path": "check.txt", "contents": "fixed\n"},
                    {"path": "../outside/escaped.txt", "contents": "escaped\n"}
                ],
                "message": "reach outside"
            }),
            None,
        ),
        // 4. the edit that fixes the check.
        Reply::select("repository.edit", None),
        Reply::arguments(
            json!({
                "files": [{"path": "check.txt", "contents": "fixed\n"}],
                "message": "fix the check"
            }),
            None,
        ),
        // 5. tests.run on the fixed revision.
        Reply::select("tests.run", None),
        Reply::arguments(json!({}), None),
        // 6. merge, once the implementation is verified.
        Reply::select("repository.merge", None),
        Reply::arguments(json!({"revision": "main"}), None),
    ]);
    let briefing = Briefing::new(INTENT, found.clone());
    let loom = Loom::new(
        ModelSelector::new(&model, briefing.clone()),
        ModelArguments::new(&model, briefing.clone()),
        INTENT,
    );
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        workspace,
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    );
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);
    let next = || {
        let frontier = governor
            .frontier(&case)
            .expect("the governor issues a frontier");
        proposed(loom.run(&commission, &frontier))
    };

    // 1. Inspect returns the file; a model answer claiming a pass produces no evidence.
    let inspect = next();
    assert_eq!(inspect.action, "repository.inspect");
    let report = executor.execute(&inspect).expect("inspect is performed");
    let Report::Inspected(files) = &report else {
        panic!("inspect reports the files it read: {report:?}");
    };
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(files[0].path, "check.txt");
    assert_eq!(files[0].contents, "broken\n");
    assert_eq!(
        verifier
            .verify(&report)
            .expect("the verifier reads the report"),
        None,
        "an inspection is no test result"
    );
    assert!(
        evidence(&governor, &case).is_empty(),
        "a recorded claim of a passing test is no evidence"
    );
    briefing.record(&inspect, &report);

    // 2. tests.run while the check fails: the verifier reads the command's exit status, not the
    // model's claim, and submits `fail`.
    let run = next();
    assert_eq!(run.action, "tests.run");
    let report = executor.execute(&run).expect("tests.run is performed");
    assert!(matches!(report, Report::TestsRun(_)), "{report:?}");
    let failed = verifier
        .verify(&report)
        .expect("the verifier submits the result")
        .expect("a test run is a test result");
    let held = evidence(&governor, &case);
    assert_eq!(held.len(), 1, "one test run, one record: {held:?}");
    assert_eq!(held[0].evidence_id, failed);
    assert_test_result(&held[0], "fail", &first);
    assert!(
        !held
            .iter()
            .any(|record| text(&record.facts, "result") == "pass"),
        "a recorded claim of a passing test is no evidence: {held:?}"
    );
    briefing.record(&run, &report);

    // 3. An edit that names a path outside the workspace is refused, and writes nothing.
    let reach = next();
    assert_eq!(reach.action, "repository.edit");
    let before = governor.current_revision(&case).expect("revision");
    assert!(
        matches!(
            executor.execute(&reach),
            Err(ExecuteError::OutsideWorkspace { .. })
        ),
        "an edit path outside the workspace is refused"
    );
    assert!(!outside.join("escaped.txt").exists(), "nothing escaped");
    assert_eq!(fixture.read("check.txt"), "broken\n", "nothing was written");
    assert_eq!(fixture.status(), "", "the work tree is untouched");
    assert_eq!(fixture.head(), first, "nothing was committed");
    assert_eq!(governor.current_revision(&case).expect("revision"), before);

    // 4. The edit commits a new revision and reports it to the governor.
    let edit = next();
    assert_eq!(edit.action, "repository.edit");
    let report = executor.execute(&edit).expect("the edit is performed");
    let fixed = fixture.head();
    assert_ne!(fixed, first, "the edit commits a new revision");
    let Report::Edited { revision } = &report else {
        panic!("an edit reports its revision: {report:?}");
    };
    assert_eq!(revision, &fixed);
    assert_eq!(fixture.git(&["rev-parse", "HEAD^"]).trim(), first);
    assert_eq!(fixture.git(&["show", "HEAD:check.txt"]), "fixed\n");
    assert_eq!(fixture.status(), "", "the edit is committed whole");
    assert_eq!(
        fixture.git(&["log", "-1", "--format=%ae"]).trim(),
        "fixture@example.invalid",
        "the commit uses the workspace's own identity"
    );
    assert_eq!(
        governor.revisions(&case).expect("held")["implementation"],
        fixed,
        "the governor holds the new implementation revision"
    );
    assert_eq!(
        governor.current_revision(&case).expect("revision"),
        before + 1
    );
    briefing.record(&edit, &report);

    // 5. tests.run on the fixed revision: the verifier submits `pass` about it.
    let run = next();
    assert_eq!(run.action, "tests.run");
    let report = executor.execute(&run).expect("tests.run is performed");
    let passed = verifier
        .verify(&report)
        .expect("the verifier submits the result")
        .expect("a test run is a test result");
    let held = evidence(&governor, &case);
    assert_eq!(held.len(), 2, "{held:?}");
    let pass = &held[1];
    assert_eq!(pass.evidence_id, passed);
    assert_test_result(pass, "pass", &fixed);
    assert_eq!(
        pass.subject_revision,
        governor.current_revision(&case).expect("revision"),
        "the pass is about the current case revision"
    );
    let observed: Vec<_> = governor
        .observations()
        .into_iter()
        .map(|observation| observation.observation_id)
        .collect();
    for record in &held {
        assert!(!record.observation_ids.is_empty(), "{record:?}");
        for id in &record.observation_ids {
            assert!(
                observed.contains(id),
                "the evidence interprets an observation the governor received: {id:?}"
            );
        }
    }
    let frontier = governor.frontier(&case).expect("frontier").into_data();
    assert!(
        frontier
            .claims
            .iter()
            .any(|claim| claim.claim == "implementation.verified" && claim.value == Truth::True),
        "the pass applies to the current implementation: {:?}",
        frontier.claims
    );
    briefing.record(&run, &report);

    // 6. A recorded choice of `repository.merge` is not executed.
    let merge_status = frontier
        .actions
        .iter()
        .find(|action| action.action == "repository.merge")
        .map(|action| action.status);
    assert_eq!(merge_status, Some(ActionStatus::ApprovalRequired));
    let merge = next();
    assert_eq!(merge.action, "repository.merge");
    let branches = fixture.git(&["branch", "--list"]);
    assert!(
        matches!(
            executor.execute(&merge),
            Err(ExecuteError::NotExecuted { .. })
        ),
        "repository.merge is never executed"
    );
    assert_eq!(fixture.head(), fixed);
    assert_eq!(fixture.git(&["branch", "--list"]), branches);
    assert_eq!(fixture.status(), "");
    assert_eq!(evidence(&governor, &case).len(), 2);
    assert_eq!(model.remaining(), 0, "every recorded reply was asked for");

    // The selector's context: the intent, its references, the catalogue and the transcript so far.
    let selections = model.asked(Asked::Selection);
    assert_eq!(selections.len(), 6, "one selection per Loom run");
    for (n, request) in selections.iter().enumerate() {
        let sent = serde_json::to_string(request).expect("a request serializes");
        assert!(sent.contains(INTENT), "selection {n} carries the intent");
        for reference in &found {
            assert!(
                sent.contains(&reference.value),
                "selection {n} carries the reference {reference:?}"
            );
        }
        for action in ["repository.inspect", "repository.edit", "tests.run"] {
            assert!(sent.contains(action), "selection {n} lists {action}");
        }
    }
    let after_inspect = serde_json::to_string(&selections[1]).expect("serializes");
    assert!(
        after_inspect.contains("broken"),
        "the transcript so far carries what inspect returned"
    );
    let generations = model.asked(Asked::Arguments);
    assert_eq!(generations.len(), 6, "one argument request per proposal");

    // Every other edit path outside the workspace is refused too, and nothing else is executed.
    let refused = [
        outside.join("absolute.txt").to_string_lossy().into_owned(),
        "link/through-link.txt".to_owned(),
        ".git/hooks/post-commit".to_owned(),
        "nested/../../outside/dotdot.txt".to_owned(),
    ];
    for path in &refused {
        let edit = proposal(
            "repository.edit",
            &json!({"files": [{"path": path, "contents": "escaped\n"}], "message": "escape"}),
        );
        assert!(
            matches!(
                executor.execute(&edit),
                Err(ExecuteError::OutsideWorkspace { .. })
            ),
            "`{path}` is outside the workspace"
        );
    }
    for written in ["absolute.txt", "through-link.txt", "dotdot.txt"] {
        assert!(!outside.join(written).exists(), "{written} escaped");
    }
    assert!(!workspace.join(".git/hooks/post-commit").exists());
    for action in ["repository.merge", "deployment.rollout", "release.publish"] {
        assert!(
            matches!(
                executor.execute(&proposal(action, &json!({}))),
                Err(ExecuteError::NotExecuted { .. })
            ),
            "{action} is never executed"
        );
    }
    assert_eq!(fixture.head(), fixed);
    assert_eq!(fixture.status(), "");
}

/// A test command still running at its timeout is killed there, and the run is a `fail` marked as
/// timed out: `sleep 5` under a 300 ms timeout ends well before five seconds.
#[test]
fn a_test_command_past_its_timeout_is_killed_and_fails() {
    let fixture = Fixture::new();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        fixture.workspace(),
        TestCommand::new("sleep", ["5"]).with_timeout(Duration::from_millis(300)),
    );
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    let started = Instant::now();
    let report = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(4),
        "the command is killed at its timeout, not run out: {elapsed:?}"
    );
    let Report::TestsRun(run) = &report else {
        panic!("tests.run reports a test run: {report:?}");
    };
    assert!(run.timed_out(), "the run is marked as timed out: {run:?}");
    assert_ne!(run.exit_code(), Some(0));
    assert!(
        report.to_string().contains("timed out"),
        "the report says so: {report}"
    );
    verifier
        .verify(&report)
        .expect("the verifier submits the result")
        .expect("a test run is a test result");
    let held = evidence(&governor, &case);
    assert_eq!(held.len(), 1, "{held:?}");
    assert_test_result(&held[0], "fail", &fixture.head());
}

/// A test command's output reaches the report as its tail only: `seq 1 200000` prints about 1.3 MB.
#[test]
fn a_test_commands_output_is_kept_as_its_tail() {
    let fixture = Fixture::new();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        fixture.workspace(),
        TestCommand::new("seq", ["1", "200000"]),
    );
    let report = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    let shown = report.to_string();
    assert!(shown.contains("199999\n200000"), "the tail is kept");
    assert!(!shown.contains("\n1\n2\n3\n"), "the head is not");
    assert!(shown.len() < 20 * 1024, "{} bytes", shown.len());
}

/// A path the workspace's git ignores is neither read by `repository.inspect` (an ignored secret
/// never reaches the model) nor written by `repository.edit`, and an edit naming one writes nothing.
#[test]
fn an_ignored_path_is_neither_read_nor_written() {
    let fixture = Fixture::new();
    std::fs::write(fixture.workspace().join(".gitignore"), "*.local\n").expect("write .gitignore");
    fixture.git(&["add", ".gitignore"]);
    fixture.git(&["commit", "--quiet", "--message", "ignore local files"]);
    std::fs::write(fixture.workspace().join("secret.local"), "token\n").expect("write a secret");
    let head = fixture.head();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        fixture.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    );

    let inspected = executor.execute(&proposal(
        "repository.inspect",
        &json!({"paths": ["check.txt", "secret.local"]}),
    ));
    assert!(
        matches!(inspected, Err(ExecuteError::Ignored { ref path }) if path == "secret.local"),
        "an ignored file is not read: {inspected:?}"
    );
    let edited = executor.execute(&proposal(
        "repository.edit",
        &json!({
            "files": [
                {"path": "check.txt", "contents": "fixed\n"},
                {"path": "new.local", "contents": "fixed\n"}
            ],
            "message": "write an ignored file"
        }),
    ));
    assert!(
        matches!(edited, Err(ExecuteError::Ignored { ref path }) if path == "new.local"),
        "an ignored file is not written: {edited:?}"
    );
    assert!(!fixture.workspace().join("new.local").exists());
    assert_eq!(fixture.read("check.txt"), "broken\n");
    assert_eq!(fixture.head(), head);
    let inspected = executor
        .execute(&proposal(
            "repository.inspect",
            &json!({"paths": ["check.txt", ".gitignore"]}),
        ))
        .expect("files that are not ignored are read");
    assert!(matches!(inspected, Report::Inspected(ref files) if files.len() == 2));
}

/// One test run yields one evidence record: verifying it again, through the same verifier or
/// another for the same case, is refused and submits nothing. A new run is verified.
#[test]
fn a_test_run_is_verified_once() {
    let fixture = Fixture::new();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        fixture.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    );
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);
    let run = || {
        executor
            .execute(&proposal("tests.run", &json!({})))
            .expect("tests.run is performed")
    };

    let report = run();
    verifier
        .verify(&report)
        .expect("the first verification submits")
        .expect("a test run is a test result");
    let again = verifier.verify(&report);
    assert!(
        matches!(again, Err(VerifyError::AlreadyVerified { .. })),
        "a second verification of one run is refused: {again:?}"
    );
    let other = TestResultVerifier::new(&governor, case.clone(), PRODUCER).verify(&report);
    assert!(
        matches!(other, Err(VerifyError::AlreadyVerified { .. })),
        "another verifier of the case refuses it too: {other:?}"
    );
    assert_eq!(evidence(&governor, &case).len(), 1, "one run, one record");

    let next = run();
    verifier
        .verify(&next)
        .expect("a new run is verified")
        .expect("a test run is a test result");
    assert_eq!(evidence(&governor, &case).len(), 2);
}

/// `record` is a `test_result` about `implementation` at `head`, with `result`, from the trusted
/// producer.
fn assert_test_result(record: &EvidenceData, result: &str, head: &str) {
    assert_eq!(record.kind, "test_result", "{record:?}");
    assert_eq!(record.producer, PRODUCER, "{record:?}");
    assert_eq!(text(&record.facts, "format"), "canon-evidence/1");
    assert_eq!(text(&record.facts, "kind"), "test_result");
    assert_eq!(text(&record.facts, "result"), result, "{record:?}");
    assert_eq!(text(&record.facts, "subject"), "implementation");
    assert_eq!(text(&record.facts, "subject_revision"), head, "{record:?}");
}

fn text<'a>(value: &'a cjson::Value, member: &str) -> &'a str {
    match value.member(member) {
        Some(cjson::Value::Text(text)) => text,
        other => panic!("`{member}` is not text: {other:?}"),
    }
}

fn evidence(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Vec<EvidenceData> {
    governor
        .evidence(case)
        .expect("the governor holds the case")
}

fn proposed(outcome: ExecutorOutcome) -> ExecutorOutcomeProposedAction {
    match outcome {
        ExecutorOutcome::ProposedAction(proposal) => proposal,
        other => panic!("Loom proposed nothing: {other:?}"),
    }
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".into()),
        authority_context: AuthorityContext(cjson::Value::Null),
    })
}

/// Which kind of reply answered a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    Selection,
    Arguments,
}

/// One recorded model reply: the forced call's arguments and, optionally, text said beside it.
struct Reply {
    asked: Asked,
    arguments: serde_json::Value,
    says: Option<&'static str>,
}

impl Reply {
    fn select(action: &str, says: Option<&'static str>) -> Self {
        Self {
            asked: Asked::Selection,
            arguments: json!({ "action": action }),
            says,
        }
    }

    fn arguments(arguments: serde_json::Value, says: Option<&'static str>) -> Self {
        Self {
            asked: Asked::Arguments,
            arguments,
            says,
        }
    }
}

/// A model that answers with recorded replies, in order, and keeps every request it was sent.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    script: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<(Asked, TurnRequest)>>,
}

impl Recorded {
    fn new(script: Vec<Reply>) -> Self {
        let id = |value: &str| Id::new(value).expect("a fixture id");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("fixture"),
                endpoint: id("in-process"),
                model: id("recorded-model"),
                binding_revision: id("recorded-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                ..Capabilities::text(128_000, 8_192)
            },
            script: Mutex::new(script.into()),
            seen: Mutex::default(),
        }
    }

    fn remaining(&self) -> usize {
        self.script.lock().expect("script").len()
    }

    fn asked(&self, asked: Asked) -> Vec<TurnRequest> {
        self.seen
            .lock()
            .expect("seen")
            .iter()
            .filter(|(kind, _)| *kind == asked)
            .map(|(_, request)| request.clone())
            .collect()
    }
}

impl Model for Recorded {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            if let Err(error) = request.validate_for(&self.provenance, &self.capabilities) {
                panic!("the slice sent a request the model refuses: {error}");
            }
            let ToolChoice::Named(tool) = &request.tool_choice else {
                panic!(
                    "the recorded model answers only a forced call of one named tool: {:?}",
                    request.tool_choice
                );
            };
            let Some(reply) = self.script.lock().expect("script").pop_front() else {
                panic!("the model was asked more often than recorded");
            };
            let mut seen = self.seen.lock().expect("seen");
            seen.push((reply.asked, request.clone()));
            let call_id = CallId::new(format!("call-{}", seen.len())).expect("a call id");
            let mut items = Vec::new();
            if let Some(text) = reply.says {
                items.push(Item::assistant(text));
            }
            items.push(Item::ToolCall(ToolCall {
                call_id,
                name: tool.clone(),
                arguments: reply.arguments,
            }));
            Ok(TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items,
                observation: TurnObservation {
                    final_usage: true,
                    ..TurnObservation::new(self.provenance.clone())
                },
            })
        })
    }
}

/// A scratch directory holding the workspace (a git repository with a local identity, one failing
/// check and a link to `outside`) and `outside`, a sibling directory that is not the workspace.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("selector-executor-{}-{nanos}", std::process::id()));
        let workspace = root.join("workspace");
        let outside = root.join("outside");
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        std::fs::create_dir_all(&outside).expect("create the outside directory");
        let fixture = Self {
            root,
            workspace,
            outside,
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").expect("write check.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink("../outside", fixture.workspace.join("link"))
            .expect("link to the outside directory");
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "a failing check"]);
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn outside(&self) -> &Path {
        &self.outside
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn status(&self) -> String {
        self.git(&["status", "--porcelain", "--untracked-files=all"])
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.workspace.join(file)).expect("read a workspace file")
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
