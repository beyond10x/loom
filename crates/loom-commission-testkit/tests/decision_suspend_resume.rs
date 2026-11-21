//! Acceptance for `story:decision-suspension-reason`: Commission holds a run for a decision request
//! and resumes it only once the decision is taken and, where the decided option produces a record,
//! recorded.
//!
//! An executor that returns `Suspended` with a `Decision` reason suspends the Run and, under a
//! suspension port, holds the commission's Suspension `Pending` with that reason and no action
//! request. `resume_after_decision`, given the decided hold, reads the Suspension, refuses one held
//! for authority (`HeldForAuthority`) or for another request (`OtherDecision`), takes it (a
//! `Resuming` one is not taken again), resumes the Run, releases the Suspension and continues the
//! loop from the case's current revision, performing no held request. `resume_after_grant` refuses
//! a Decision hold (`HeldForDecision`), and a commission holding one starts no Run.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use b10x_loom_commission::model::behaviour::{RunStorage, SuspensionStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, DecidedHold, DecidedRecord, DecisionHold, DecisionUnblocks, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeSuspended, HoldSuspension,
    HoldSuspensionOutcome, ObservationId, PrincipalId, ProposedActionArguments, RunId, RunOutcome,
    RunOutcomeSuspended, RunState, SuspensionData, SuspensionKey, SuspensionReason,
    SuspensionSnapshot, SuspensionState, SuspensionStateConflict, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, LoopFailure, ResumeError, ResumeRefusal, resume_after_decision,
    resume_after_grant, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

/// The decision request the run is held for.
const HELD: &str = "DR-1";
/// The run the loop starts, and the one every hold names.
const RUN: u64 = 0x101;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case(name: &str) -> CaseId {
    CaseId(format!("case-decision-{name}"))
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-d".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// A governor holding `case` open at `revision`, its frontier listing no action, on every call.
fn governor_at(case: &CaseId, revision: i64) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [Answer::at(revision)]);
    governor
}

/// The hold of `request` for what it `unblocks`.
fn decision(request: &str, unblocks: DecisionUnblocks) -> SuspensionReason {
    SuspensionReason::Decision(DecisionHold {
        request_id: request.to_owned(),
        unblocks,
    })
}

fn suspended(reason: SuspensionReason) -> ExecutorOutcome {
    ExecutorOutcome::Suspended(ExecutorOutcomeSuspended { reason })
}

fn recorded(request: &str) -> DecidedHold {
    DecidedHold {
        request_id: request.to_owned(),
        record: DecidedRecord::Recorded(Value::Object(vec![(
            "record_id".to_owned(),
            Value::Text("rec-answer-1".to_owned()),
        )])),
    }
}

fn no_record(request: &str) -> DecidedHold {
    DecidedHold {
        request_id: request.to_owned(),
        record: DecidedRecord::NoRecord(Unit(true)),
    }
}

/// An effect port that performs every action and counts its invocations.
#[derive(Default)]
struct Effects {
    invoked: AtomicUsize,
}

impl Effects {
    fn invoked(&self) -> usize {
        self.invoked.load(Ordering::SeqCst)
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("performed".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

/// New ids from counters and a fixed trusted clock; no step budget.
#[derive(Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(uuid(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(uuid(0x400 + self.observations))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

fn run_store() -> RunStore {
    RunStore::new(|| RunId(uuid(RUN)))
}

/// What the suspension port holds for `commission`.
fn stored(
    suspensions: &dyn SuspensionPort,
    commission: &Commission<commission_state::Assigned>,
) -> Option<SuspensionSnapshot> {
    suspensions
        .get(&commission.data().commission_id)
        .expect("the store reads")
}

/// A run of `commission` under `suspensions` whose executor suspends it for `reason`, the case at
/// revision 1.
fn held_by_the_run(
    commission: &Commission<commission_state::Assigned>,
    suspensions: &dyn SuspensionPort,
    reason: SuspensionReason,
) -> (LoopEnd, RunStore, usize) {
    let governor = governor_at(&commission.data().case_id, 1);
    let executor = ScriptedExecutor::new([suspended(reason)]);
    let effects = Effects::default();
    let mut runs = run_store();
    let end = run_with_suspension(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        commission,
        &mut runs,
        suspensions,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
    (end, runs, effects.invoked())
}

/// What one resume after a decision ended with.
struct Resumed {
    result: Result<LoopEnd, ResumeError>,
    executor: ScriptedExecutor,
    invoked: usize,
    runs: RunStore,
}

/// `resume_after_decision` of `commission` with `decided`, the case at `revision`, its executor
/// answering `NoUsefulAction` once.
fn resume_decided(
    commission: &Commission<commission_state::Assigned>,
    suspensions: &dyn SuspensionPort,
    revision: i64,
    decided: &DecidedHold,
) -> Resumed {
    let governor = governor_at(&commission.data().case_id, revision);
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::default();
    let mut runs = run_store();
    let result = resume_after_decision(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        commission,
        &mut runs,
        suspensions,
        decided,
        &mut Context::default(),
    );
    Resumed {
        result,
        executor,
        invoked: effects.invoked(),
        runs,
    }
}

fn refusal(result: &Result<LoopEnd, ResumeError>) -> Option<&ResumeRefusal> {
    match result {
        Err(ResumeError::Refused(refusal)) => Some(refusal),
        _ => None,
    }
}

/// The commission's Suspension holding the Run for `reason`, with no action request, as a Decision
/// hold stores it.
fn decision_data(
    commission: &Commission<commission_state::Assigned>,
    reason: SuspensionReason,
) -> SuspensionData {
    let commission_id = commission.data().commission_id.clone();
    SuspensionData {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(RUN)),
        run_case_revision: 1,
        action_request_id: None,
        case_id: commission.data().case_id.clone(),
        expected_case_revision: None,
        action: None,
        arguments: None,
        reason,
    }
}

#[test]
fn an_executor_suspended_for_a_decision_holds_a_pending_suspension_with_no_action_request() {
    let commission = commission(&case("held"));
    let suspensions = SuspensionStore::new();
    let reason = decision(HELD, DecisionUnblocks::Question);

    let (end, runs, invoked) = held_by_the_run(&commission, &suspensions, reason.clone());

    assert_eq!(
        end.outcome,
        RunOutcome::Suspended(RunOutcomeSuspended {
            reason: reason.clone()
        })
    );
    let held = stored(&suspensions, &commission).expect("the Suspension is held");
    assert_eq!(held.state, SuspensionState::Pending);
    assert_eq!(held.data, decision_data(&commission, reason));
    assert_eq!(
        held.data.run_id, end.run_id,
        "it holds the Run the loop started"
    );
    assert_eq!(invoked, 0, "nothing is performed");
    assert!(end.admitted.is_empty(), "no request was admitted");
    assert_eq!(
        runs.get(&end.run_id).map(|run| run.state),
        Some(RunState::Suspended)
    );
}

#[test]
fn a_recorded_decision_resumes_the_run_at_the_current_revision_and_releases_the_suspension() {
    let commission = commission(&case("recorded"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Question),
    );

    // The answered question moved an artifact: the case is at revision 2 now.
    let resumed = resume_decided(&commission, &suspensions, 2, &recorded(HELD));

    let end = resumed
        .result
        .unwrap_or_else(|error| panic!("the resume failed: {error}"));
    assert_eq!(end.run_id, RunId(uuid(RUN)), "the same Run goes on");
    let handed: Vec<i64> = resumed
        .executor
        .frontiers()
        .iter()
        .map(|frontier| frontier.case_revision)
        .collect();
    assert_eq!(
        handed,
        vec![2],
        "the loop continues at the case's current revision, not the hold's"
    );
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
    assert_eq!(resumed.invoked, 0, "no held request is performed");
    assert!(end.admitted.is_empty());
    assert_eq!(
        stored(&suspensions, &commission),
        None,
        "the Suspension is released"
    );
    assert_eq!(
        resumed.runs.get(&end.run_id).map(|run| run.state),
        Some(RunState::Running),
        "the Run was resumed"
    );
}

#[test]
fn a_second_resume_after_the_decision_finds_nothing_suspended() {
    let commission = commission(&case("twice"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Question),
    );
    let first = resume_decided(&commission, &suspensions, 2, &recorded(HELD));
    assert!(
        first.result.is_ok(),
        "the first resume goes on: {:?}",
        first.result.err()
    );

    let second = resume_decided(&commission, &suspensions, 2, &recorded(HELD));

    assert_eq!(
        refusal(&second.result),
        Some(&ResumeRefusal::NothingSuspended)
    );
    assert!(second.executor.calls().is_empty(), "no executor runs");
    assert_eq!(second.invoked, 0);
}

#[test]
fn a_decision_that_produces_no_record_resumes_the_run_and_it_ends_as_the_frontier_decides() {
    let commission = commission(&case("no-record"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Outcome),
    );

    // An outcome left untaken leaves the case where it was.
    let resumed = resume_decided(&commission, &suspensions, 1, &no_record(HELD));

    let end = resumed
        .result
        .unwrap_or_else(|error| panic!("the resume failed: {error}"));
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
    assert_eq!(resumed.executor.calls().len(), 1, "the loop went on");
    assert_eq!(resumed.invoked, 0, "nothing is performed");
    assert_eq!(
        stored(&suspensions, &commission),
        None,
        "the Suspension is released"
    );
}

#[test]
fn a_resume_for_another_decision_request_is_refused_and_the_suspension_stays() {
    let commission = commission(&case("other"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Question),
    );
    let before = stored(&suspensions, &commission);

    let resumed = resume_decided(&commission, &suspensions, 2, &recorded("DR-2"));

    assert_eq!(
        refusal(&resumed.result),
        Some(&ResumeRefusal::OtherDecision {
            request_id: "DR-2".to_owned(),
            held_request_id: HELD.to_owned(),
        })
    );
    assert_eq!(
        stored(&suspensions, &commission),
        before,
        "the Suspension stays"
    );
    assert!(resumed.executor.calls().is_empty(), "no executor runs");
    assert_eq!(resumed.invoked, 0);
}

#[test]
fn a_resume_after_a_grant_on_a_decision_hold_is_refused_and_the_suspension_stays() {
    let commission = commission(&case("grant"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Approval),
    );
    let before = stored(&suspensions, &commission);
    let governor = governor_at(&commission.data().case_id, 1);
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::default();

    let resumed = resume_after_grant(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut run_store(),
        &suspensions,
        &mut Context::default(),
    );

    assert_eq!(refusal(&resumed), Some(&ResumeRefusal::HeldForDecision));
    assert_eq!(
        stored(&suspensions, &commission),
        before,
        "the Suspension stays"
    );
    assert!(
        governor.calls().is_empty(),
        "nothing is read from the governor"
    );
    assert_eq!(effects.invoked(), 0);
}

#[test]
fn a_resume_after_a_decision_on_an_authority_hold_is_refused_and_the_suspension_stays() {
    let commission = commission(&case("authority"));
    let suspensions = SuspensionStore::new();
    let commission_id = commission.data().commission_id.clone();
    let held = suspensions.hold(HoldSuspension {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(RUN)),
        run_case_revision: 1,
        action_request_id: Some(ActionRequestId(uuid(0x301))),
        case_id: commission.data().case_id.clone(),
        expected_case_revision: Some(1),
        action: Some("repository.merge".to_owned()),
        arguments: Some(ProposedActionArguments(Value::Null)),
        reason: SuspensionReason::Authority(Value::Null),
    });
    assert!(matches!(held, HoldSuspensionOutcome::Held { .. }));
    let before = stored(&suspensions, &commission);

    let resumed = resume_decided(&commission, &suspensions, 1, &recorded(HELD));

    assert_eq!(
        refusal(&resumed.result),
        Some(&ResumeRefusal::HeldForAuthority)
    );
    assert_eq!(
        stored(&suspensions, &commission),
        before,
        "the Suspension stays"
    );
    assert!(resumed.executor.calls().is_empty(), "no executor runs");
    assert_eq!(resumed.invoked, 0);
}

#[test]
fn a_resuming_decision_suspension_is_released_and_the_loop_goes_on_performing_nothing() {
    let commission = commission(&case("resuming"));
    let mut suspensions = SuspensionStore::new();
    // A resume took it and stopped before it released it.
    SuspensionStorage::put(
        &mut suspensions,
        SuspensionSnapshot {
            state: SuspensionState::Resuming,
            data: decision_data(&commission, decision(HELD, DecisionUnblocks::Question)),
        },
    );

    let resumed = resume_decided(&commission, &suspensions, 2, &recorded(HELD));

    let end = resumed
        .result
        .unwrap_or_else(|error| panic!("the resume failed: {error}"));
    assert_eq!(stored(&suspensions, &commission), None, "it is released");
    assert_eq!(resumed.executor.calls().len(), 1, "the loop went on");
    assert_eq!(end.outcome, RunOutcome::NoAdmissibleAction(Unit(true)));
    assert_eq!(resumed.invoked, 0, "nothing is performed");
}

#[test]
fn a_commission_holding_a_decision_suspension_starts_no_run() {
    let commission = commission(&case("no-new-run"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Question),
    );
    let governor = governor_at(&commission.data().case_id, 1);
    let executor = ScriptedExecutor::new([]);
    let effects = Effects::default();
    let mut runs = run_store();

    let refused = run_with_suspension(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::default(),
    )
    .expect_err("a commission holding a Suspension starts no Run");

    assert_eq!(refused.run_id, None, "no Run was started");
    assert_eq!(
        refused.failure,
        LoopFailure::CommissionSuspended(SuspensionStateConflict {
            state: SuspensionState::Pending,
            action_request_id: None,
        })
    );
    assert!(runs.list().is_empty(), "the run store holds no Run");
    assert!(executor.calls().is_empty(), "the executor was not run");
    assert_eq!(effects.invoked(), 0);
}

const ROLE: &str = "LOOM_DECISION_SUSPEND_RESUME_ROLE";
const DIR: &str = "LOOM_DECISION_SUSPEND_RESUME_DIR";
const TEST: &str = "a_file_suspension_store_holds_a_decision_suspension_across_a_process_restart";

/// The case of the restart test, in both lives.
fn restart_case() -> CaseId {
    case("restart")
}

#[test]
fn a_file_suspension_store_holds_a_decision_suspension_across_a_process_restart() {
    if let Ok(role) = std::env::var(ROLE) {
        let dir = PathBuf::from(std::env::var(DIR).expect("the parent names the directory"));
        match role.as_str() {
            "suspend" => suspend_child(&dir),
            "resume" => resume_child(&dir),
            other => panic!("unknown child role `{other}`"),
        }
        return;
    }

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("decision-suspend-resume")
        .join("restart");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    run_child("suspend", &dir);

    let files: Vec<PathBuf> = std::fs::read_dir(dir.join("suspensions"))
        .expect("the store directory reads")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    assert_eq!(files.len(), 1, "one Suspension file: {files:?}");
    let text = std::fs::read_to_string(&files[0]).expect("the Suspension file reads");
    let file: serde_json::Value = serde_json::from_str(&text).expect("the file is JSON");
    for member in [
        "action_request_id",
        "expected_case_revision",
        "action",
        "arguments",
    ] {
        assert_eq!(
            file.get(member),
            Some(&serde_json::Value::Null),
            "`{member}` is written as null: {text}"
        );
    }
    assert_eq!(file["state"], "Pending");
    assert_eq!(file["reason"]["kind"], "Decision");

    run_child("resume", &dir);

    let suspended = read(&dir.join("suspend.txt"));
    let resumed = read(&dir.join("resume.txt"));
    assert_ne!(
        line(&suspended, "pid"),
        line(&resumed, "pid"),
        "the second life is another process"
    );
    assert_eq!(line(&resumed, "held"), "Pending Decision DR-1 Question");
    assert_eq!(line(&resumed, "outcome"), "NoAdmissibleAction");
    assert_eq!(line(&resumed, "invoked"), "0");
    assert_eq!(line(&resumed, "after"), "none");
}

/// The first life: the executor suspends the run for `DR-1`, held in a file store.
fn suspend_child(dir: &Path) {
    let commission = commission(&restart_case());
    let suspensions =
        FileSuspensionStore::open(dir.join("suspensions")).expect("the suspension store opens");
    let (end, _, invoked) = held_by_the_run(
        &commission,
        &suspensions,
        decision(HELD, DecisionUnblocks::Question),
    );
    assert!(matches!(end.outcome, RunOutcome::Suspended(_)));
    assert_eq!(invoked, 0);
    std::fs::write(
        dir.join("suspend.txt"),
        format!("pid={}\n", std::process::id()),
    )
    .unwrap();
}

/// The second life: a new store over the same directory reads the hold back and resumes it.
fn resume_child(dir: &Path) {
    let commission = commission(&restart_case());
    let suspensions =
        FileSuspensionStore::open(dir.join("suspensions")).expect("the suspension store opens");
    let held = stored(&suspensions, &commission).expect("the hold outlived the first life");
    let SuspensionReason::Decision(hold) = &held.data.reason else {
        panic!("the hold is no Decision hold: {:?}", held.data.reason);
    };
    let held_line = format!(
        "{:?} Decision {} {:?}",
        held.state, hold.request_id, hold.unblocks
    );
    let resumed = resume_decided(&commission, &suspensions, 2, &recorded(HELD));
    let end = resumed
        .result
        .unwrap_or_else(|error| panic!("the resume failed: {error}"));
    let outcome = match end.outcome {
        RunOutcome::NoAdmissibleAction(_) => "NoAdmissibleAction".to_owned(),
        other => format!("{other:?}"),
    };
    let after = match stored(&suspensions, &commission) {
        None => "none".to_owned(),
        Some(left) => format!("{:?}", left.state),
    };
    std::fs::write(
        dir.join("resume.txt"),
        format!(
            "pid={}\nheld={held_line}\noutcome={outcome}\ninvoked={}\nafter={after}\n",
            std::process::id(),
            resumed.invoked
        ),
    )
    .unwrap();
}

/// Runs this test's binary as a child in `role` over `dir` and requires it to exit 0.
fn run_child(role: &str, dir: &Path) {
    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(DIR, dir)
        .output()
        .expect("the child starts");
    assert!(
        output.status.success(),
        "the {role} child failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1 passed"),
        "the {role} child ran the one test: {stdout}"
    );
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The value of `key` in a `key=value` record.
fn line<'t>(record: &'t str, key: &str) -> &'t str {
    record
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
        .unwrap_or_else(|| panic!("no `{key}` in {record}"))
}
