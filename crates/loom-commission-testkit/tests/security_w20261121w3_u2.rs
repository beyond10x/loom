//! Security review, wave 2026-11-21-w3, unit U2 (`story:decision-suspension-reason`): integrity
//! conformance checks of the decision hold against the invariants of the review brief.
//!
//! * A run held for a decision resumes only with that decision: not after another resume consumed
//!   the hold and the run was held again for another request between this resume's read and its
//!   take, not for another case, not over a run store holding the Run for another commission.
//! * `resume_after_grant` refuses a `Resuming` decision hold `HeldForDecision`, reading nothing
//!   from the governor.
//! * A decision hold carrying request members (a crafted Suspension) never reaches the code that
//!   performs a request.
//! * A Suspension file written before the story decodes as an approval hold with its four
//!   members; a file whose reason and members disagree is refused by every operation and the
//!   refusal changes nothing on disk; a hold the file would not read back is never written.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use b10x_loom_commission::model::behaviour::{RunStorage, SuspensionStorage};
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AnyRun, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, DecidedHold, DecidedRecord, DecisionHold, DecisionUnblocks, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, ExecutorOutcomeSuspended, HoldSuspension,
    HoldSuspensionOutcome, ObservationId, PrincipalId, ProposedActionArguments, ReleaseSuspension,
    ReleaseSuspensionOutcome, Run, RunData, RunId, SuspensionData, SuspensionKey, SuspensionReason,
    SuspensionRequestIncomplete, SuspensionSnapshot, SuspensionState, TakeSuspension,
    TakeSuspensionOutcome, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStore, SuspensionStoreError,
};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, ResumeError, ResumeRefusal, resume_after_decision, resume_after_grant,
    run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

const RUN: u64 = 0x101;
const MERGE: &str = "repository.merge";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn case(name: &str) -> CaseId {
    CaseId(format!("case-sec-{name}"))
}

fn commission_of(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: case.clone(),
        principal: PrincipalId("principal-sec".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn governor_at(case: &CaseId, revision: i64) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [Answer::at(revision)]);
    governor
}

fn decision(request: &str) -> SuspensionReason {
    SuspensionReason::Decision(DecisionHold {
        request_id: request.to_owned(),
        unblocks: DecisionUnblocks::Question,
    })
}

fn decided(request: &str) -> DecidedHold {
    DecidedHold {
        request_id: request.to_owned(),
        record: DecidedRecord::NoRecord(Unit(true)),
    }
}

/// An effect port that performs every action and records each action it was handed.
#[derive(Default)]
struct Effects {
    invoked: Mutex<Vec<String>>,
}

impl Effects {
    fn invoked(&self) -> Vec<String> {
        self.invoked.lock().unwrap().clone()
    }
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.lock().unwrap().push(format!("{request:?}"));
        Ok(EffectOutcome::Performed(EffectOutcomePerformed {
            report: Value::Text("performed".to_owned()),
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

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

fn stored(
    suspensions: &dyn SuspensionPort,
    commission: &Commission<commission_state::Assigned>,
) -> Option<SuspensionSnapshot> {
    suspensions
        .get(&commission.data().commission_id)
        .expect("the store reads")
}

/// The Suspension a decision hold of `commission` stores, with request members as given.
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

fn hold_input(data: &SuspensionData) -> HoldSuspension {
    HoldSuspension {
        suspension: data.suspension.clone(),
        commission_id: data.commission_id.clone(),
        run_id: data.run_id.clone(),
        run_case_revision: data.run_case_revision,
        action_request_id: data.action_request_id.clone(),
        case_id: data.case_id.clone(),
        expected_case_revision: data.expected_case_revision,
        action: data.action.clone(),
        arguments: data.arguments.clone(),
        reason: data.reason.clone(),
    }
}

/// A run of `commission` whose executor suspends it for `reason`, the case at revision 1.
fn held_by_the_run(
    commission: &Commission<commission_state::Assigned>,
    suspensions: &dyn SuspensionPort,
    reason: SuspensionReason,
) {
    let governor = governor_at(&commission.data().case_id, 1);
    let executor = ScriptedExecutor::new([ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason,
    })]);
    run_with_suspension(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &Effects::default(),
        commission,
        &mut run_store(),
        suspensions,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error}"));
}

struct Resumed {
    result: Result<LoopEnd, ResumeError>,
    executor: ScriptedExecutor,
    governor: FakeGovernor,
    invoked: Vec<String>,
}

fn resume_decided_with(
    commission: &Commission<commission_state::Assigned>,
    suspensions: &dyn SuspensionPort,
    runs: &mut RunStore,
    decided: &DecidedHold,
) -> Resumed {
    let governor = governor_at(&commission.data().case_id, 2);
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::default();
    let result = resume_after_decision(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        commission,
        runs,
        suspensions,
        decided,
        &mut Context::default(),
    );
    Resumed {
        result,
        executor,
        governor,
        invoked: effects.invoked(),
    }
}

fn resume_decided(
    commission: &Commission<commission_state::Assigned>,
    suspensions: &dyn SuspensionPort,
    decided: &DecidedHold,
) -> Resumed {
    resume_decided_with(commission, suspensions, &mut run_store(), decided)
}

fn refusal(result: &Result<LoopEnd, ResumeError>) -> Option<&ResumeRefusal> {
    match result {
        Err(ResumeError::Refused(refusal)) => Some(refusal),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Invariant 1: only the decided request resumes the run it holds.
// ---------------------------------------------------------------------------------------------

/// A suspension port over memory on which, between a resume's read and its take, another resume
/// consumed the decision hold `DR-1` and the run it continued was held again for `DR-2` (take,
/// release, hold: the three port calls that resume and that run make).
struct HeldAgainBeforeTheTake<'s> {
    inner: &'s SuspensionStore,
    again: SuspensionData,
    fired: Cell<bool>,
}

impl SuspensionPort for HeldAgainBeforeTheTake<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.inner.hold(input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        if !self.fired.replace(true) {
            let key = self.again.suspension.clone();
            assert!(matches!(
                self.inner.take(TakeSuspension {
                    suspension: key.clone(),
                    action_request_id: None,
                    decision_request_id: Some("DR-1".to_owned()),
                }),
                TakeSuspensionOutcome::Taken { .. }
            ));
            assert!(matches!(
                self.inner.release(ReleaseSuspension {
                    suspension: key,
                    action_request_id: None,
                    decision_request_id: Some("DR-1".to_owned()),
                }),
                ReleaseSuspensionOutcome::Released { .. }
            ));
            assert!(matches!(
                self.inner.hold(hold_input(&self.again)),
                HoldSuspensionOutcome::Held { .. }
            ));
        }
        self.inner.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.inner.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        SuspensionPort::get(self.inner, commission)
    }
}

/// A resume given `DR-1` that read the `DR-1` hold must not consume a hold for `DR-2` placed
/// between its read and its take: the take names no decision request, so any decision hold
/// matches it. Either the resume is refused or the `DR-2` hold stays.
#[test]
fn security_a_stale_decision_resume_does_not_consume_a_later_hold_for_another_request() {
    let commission = commission_of(&case("stale"));
    let inner = SuspensionStore::new();
    held_by_the_run(&commission, &inner, decision("DR-1"));
    let again = decision_data(&commission, decision("DR-2"));
    let port = HeldAgainBeforeTheTake {
        inner: &inner,
        again: again.clone(),
        fired: Cell::new(false),
    };

    let resumed = resume_decided(&commission, &port, &decided("DR-1"));

    let left = stored(&inner, &commission);
    let still_held = left.as_ref().is_some_and(|left| {
        left.state == SuspensionState::Pending && left.data.reason == decision("DR-2")
    });
    assert!(
        refusal(&resumed.result).is_some() && still_held,
        "the DR-1 decision resumed the run held for DR-2: the resume ended {:?}, the executor \
         ran {} times, and the DR-2 hold is now {left:?}",
        resumed.result.as_ref().map(|end| &end.outcome),
        resumed.executor.calls().len()
    );
}

/// A resume of the commission over another case is refused `OtherCommission` and changes
/// nothing.
#[test]
fn security_a_decision_resume_for_another_case_is_refused_and_changes_nothing() {
    let commission = commission_of(&case("held"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(&commission, &suspensions, decision("DR-1"));
    let before = stored(&suspensions, &commission);

    let elsewhere = commission_of(&case("elsewhere"));
    let resumed = resume_decided(&elsewhere, &suspensions, &decided("DR-1"));

    assert_eq!(
        refusal(&resumed.result),
        Some(&ResumeRefusal::OtherCommission)
    );
    assert_eq!(stored(&suspensions, &commission), before);
    assert!(resumed.executor.calls().is_empty());
    assert!(resumed.invoked.is_empty());
}

/// A run store that holds the Run for another commission refuses the resume before the take.
#[test]
fn security_a_decision_resume_over_a_run_held_for_another_commission_is_refused() {
    let commission = commission_of(&case("run-other"));
    let suspensions = SuspensionStore::new();
    held_by_the_run(&commission, &suspensions, decision("DR-1"));
    let before = stored(&suspensions, &commission);
    let mut runs = run_store();
    let foreign = Run::new(RunData {
        run_id: RunId(uuid(RUN)),
        commission_id: CommissionId(uuid(0x99)),
        case_revision: 1,
    });
    RunStorage::put(&mut runs, AnyRun::Suspended(foreign.suspend()).snapshot());

    let resumed = resume_decided_with(&commission, &suspensions, &mut runs, &decided("DR-1"));

    assert_eq!(
        refusal(&resumed.result),
        Some(&ResumeRefusal::OtherCommission)
    );
    assert_eq!(stored(&suspensions, &commission), before);
    assert!(resumed.executor.calls().is_empty());
}

/// `resume_after_grant` refuses a `Resuming` decision hold `HeldForDecision` (not `InDoubt`),
/// reads nothing from the governor and changes nothing.
#[test]
fn security_resume_after_grant_refuses_a_resuming_decision_hold_before_the_governor() {
    let commission = commission_of(&case("grant-resuming"));
    let mut suspensions = SuspensionStore::new();
    SuspensionStorage::put(
        &mut suspensions,
        SuspensionSnapshot {
            state: SuspensionState::Resuming,
            data: decision_data(&commission, decision("DR-1")),
        },
    );
    let before = stored(&suspensions, &commission);
    let governor = governor_at(&commission.data().case_id, 1);
    let effects = Effects::default();

    let resumed = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut run_store(),
        &suspensions,
        &mut Context::default(),
    );

    assert_eq!(refusal(&resumed), Some(&ResumeRefusal::HeldForDecision));
    assert!(governor.calls().is_empty());
    assert_eq!(stored(&suspensions, &commission), before);
    assert!(effects.invoked().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Invariant 6: a decision hold carrying request members never reaches the performing code.
// ---------------------------------------------------------------------------------------------

/// The four request members of a merge, smuggled into a decision hold.
fn smuggled(data: &mut SuspensionData, with_request: bool) {
    data.action_request_id = with_request.then(|| ActionRequestId(uuid(0x301)));
    data.expected_case_revision = Some(1);
    data.action = Some(MERGE.to_owned());
    data.arguments = Some(ProposedActionArguments(Value::Object(Vec::new())));
}

#[test]
fn security_a_decision_hold_with_a_smuggled_request_is_never_performed() {
    let commission = commission_of(&case("smuggled"));

    // resume_after_grant: refused before the governor, nothing performed.
    let mut suspensions = SuspensionStore::new();
    let mut data = decision_data(&commission, decision("DR-1"));
    smuggled(&mut data, true);
    SuspensionStorage::put(
        &mut suspensions,
        SuspensionSnapshot {
            state: SuspensionState::Pending,
            data: data.clone(),
        },
    );
    let governor = governor_at(&commission.data().case_id, 1);
    let effects = Effects::default();
    let granted = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut run_store(),
        &suspensions,
        &mut Context::default(),
    );
    assert_eq!(refusal(&granted), Some(&ResumeRefusal::HeldForDecision));
    assert!(governor.calls().is_empty());
    assert!(effects.invoked().is_empty());

    // resume_after_decision on one naming a request: a store error, the Suspension kept.
    let decided_one = resume_decided(&commission, &suspensions, &decided("DR-1"));
    assert!(
        matches!(decided_one.result, Err(ResumeError::Store(_))),
        "{:?}",
        decided_one.result.as_ref().map(|end| &end.outcome)
    );
    assert!(decided_one.invoked.is_empty());
    assert!(decided_one.governor.calls().is_empty());
    assert_eq!(
        stored(&suspensions, &commission).map(|held| held.data),
        Some(data)
    );

    // resume_after_decision on one with action and arguments but no request: the loop goes on and
    // performs nothing it holds.
    let mut suspensions = SuspensionStore::new();
    let mut data = decision_data(&commission, decision("DR-1"));
    smuggled(&mut data, false);
    SuspensionStorage::put(
        &mut suspensions,
        SuspensionSnapshot {
            state: SuspensionState::Pending,
            data,
        },
    );
    let resumed = resume_decided(&commission, &suspensions, &decided("DR-1"));
    assert!(resumed.result.is_ok(), "{:?}", resumed.result.err());
    assert!(
        resumed.invoked.is_empty(),
        "the smuggled merge was performed: {:?}",
        resumed.invoked
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: older files decode; disagreeing files are refused and left as they are.
// ---------------------------------------------------------------------------------------------

const COMMISSION: &str = "00000000-0000-4000-8000-000000000081";
const REQUEST: &str = "00000000-0000-4000-8000-000000000301";

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w20261121w3-u2")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn file_of(dir: &Path) -> PathBuf {
    let hex: String = COMMISSION
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    dir.join(format!("suspension-{hex}.json"))
}

/// A Suspension file as the store at `39283f1b` wrote it, its members as given.
fn file_text(
    action_request_id: &str,
    expected: &str,
    action: &str,
    arguments: &str,
    reason: &str,
) -> String {
    format!(
        "{{\"state\":\"Pending\",\"commission_id\":\"{COMMISSION}\",\
         \"run_id\":\"00000000-0000-4000-8000-000000000101\",\"run_case_revision\":1,\
         \"action_request_id\":{action_request_id},\"case_id\":\"case-sec-file\",\
         \"expected_case_revision\":{expected},\"action\":{action},\"arguments\":{arguments},\
         \"reason\":{reason}}}"
    )
}

const AUTHORITY: &str =
    "{\"kind\":\"Authority\",\"payload\":{\"capability\":\"repository.merge\"}}";
const DECISION: &str =
    "{\"kind\":\"Decision\",\"payload\":{\"request_id\":\"DR-1\",\"unblocks\":\"Question\"}}";

#[test]
fn security_a_suspension_file_written_before_the_story_decodes_as_an_approval_hold() {
    for (name, arguments, expected) in [
        ("old-null-arguments", "null", Value::Null),
        ("old-object-arguments", "{}", Value::Object(Vec::new())),
    ] {
        let dir = store_dir(name);
        let store = FileSuspensionStore::open(&dir).expect("the store opens");
        let text = file_text(
            &format!("\"{REQUEST}\""),
            "3",
            "\"repository.merge\"",
            arguments,
            AUTHORITY,
        );
        std::fs::write(file_of(&dir), &text).unwrap();

        let held = store
            .get(&CommissionId(Uuid(COMMISSION.to_owned())))
            .unwrap_or_else(|error| panic!("{name}: the old file is refused: {error}"))
            .expect("the old file is read");
        assert_eq!(held.state, SuspensionState::Pending, "{name}");
        assert_eq!(
            held.data.action_request_id,
            Some(ActionRequestId(Uuid(REQUEST.to_owned()))),
            "{name}"
        );
        assert_eq!(held.data.expected_case_revision, Some(3), "{name}");
        assert_eq!(held.data.action.as_deref(), Some(MERGE), "{name}");
        assert_eq!(
            held.data.arguments,
            Some(ProposedActionArguments(expected)),
            "{name}"
        );
        assert!(
            matches!(held.data.reason, SuspensionReason::Authority(_)),
            "{name}"
        );
    }
}

/// Every file whose reason and request members disagree is refused by `get`, `hold`, `take` and
/// `release`, and the file is byte-identical afterwards, the only file of its kind.
#[test]
fn security_a_suspension_file_whose_reason_and_members_disagree_is_refused_and_kept() {
    let request = format!("\"{REQUEST}\"");
    let cases = [
        (
            "decision-with-request",
            file_text(&request, "null", "null", "null", DECISION),
        ),
        (
            "decision-with-revision",
            file_text("null", "3", "null", "null", DECISION),
        ),
        (
            "decision-with-action",
            file_text("null", "null", "\"repository.merge\"", "null", DECISION),
        ),
        (
            "decision-with-arguments",
            file_text("null", "null", "null", "{}", DECISION),
        ),
        (
            "decision-with-all-four",
            file_text(&request, "3", "\"repository.merge\"", "{}", DECISION),
        ),
        (
            "authority-without-request",
            file_text("null", "3", "\"repository.merge\"", "{}", AUTHORITY),
        ),
        (
            "authority-without-action",
            file_text(&request, "3", "null", "{}", AUTHORITY),
        ),
        (
            "decision-unblocking-nothing-known",
            file_text(
                "null",
                "null",
                "null",
                "null",
                "{\"kind\":\"Decision\",\"payload\":{\"request_id\":\"DR-1\",\"unblocks\":\"Merge\"}}",
            ),
        ),
    ];
    let commission = CommissionId(Uuid(COMMISSION.to_owned()));
    let key = SuspensionKey {
        commission_id: commission.clone(),
    };
    for (name, text) in cases {
        let dir = store_dir(name);
        let store = FileSuspensionStore::open(&dir).expect("the store opens");
        let file = file_of(&dir);
        std::fs::write(&file, &text).unwrap();

        assert!(store.get(&commission).is_err(), "{name}: get read it");
        for request in [None, Some(ActionRequestId(Uuid(REQUEST.to_owned())))] {
            let taken = store.take(TakeSuspension {
                suspension: key.clone(),
                action_request_id: request.clone(),
                decision_request_id: None,
            });
            assert!(
                matches!(taken, TakeSuspensionOutcome::StorageFailed { .. }),
                "{name}: take {request:?} answered {taken:?}"
            );
            let released = store.release(ReleaseSuspension {
                suspension: key.clone(),
                action_request_id: request.clone(),
                decision_request_id: None,
            });
            assert!(
                matches!(released, ReleaseSuspensionOutcome::StorageFailed { .. }),
                "{name}: release {request:?} answered {released:?}"
            );
        }
        let probe = commission_of(&CaseId("case-sec-file".to_owned()));
        let held = store.hold(hold_input(&decision_data(&probe, decision("DR-9"))));
        assert!(
            matches!(held, HoldSuspensionOutcome::StorageFailed { .. }),
            "{name}: hold answered {held:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            text,
            "{name}: the refused file changed"
        );
        let suspensions = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("suspension-")
            })
            .count();
        assert_eq!(suspensions, 1, "{name}: another Suspension file appeared");
    }

    // story:ess-096-upgrade: a hold of a reason other than Decision with none of the four is held,
    // so its file, all four `null`, reads back as that Suspension with no request member.
    let dir = store_dir("authority-with-no-members");
    let store = FileSuspensionStore::open(&dir).expect("the store opens");
    std::fs::write(
        file_of(&dir),
        file_text("null", "null", "null", "null", AUTHORITY),
    )
    .unwrap();
    let read = store
        .get(&commission)
        .expect("authority-with-no-members: get reads it")
        .expect("authority-with-no-members: a Suspension is stored");
    assert!(matches!(read.data.reason, SuspensionReason::Authority(_)));
    assert_eq!(
        (
            read.data.action_request_id,
            read.data.expected_case_revision,
            read.data.action,
            read.data.arguments
        ),
        (None, None, None, None),
        "authority-with-no-members"
    );
}

/// A hold whose reason and members disagree is refused by the file store and nothing is written:
/// one giving some but not all four members `IncompleteRequest`, naming its Suspension
/// (`story:ess-096-upgrade`), a `Decision` hold with all four `StorageFailed`. A hold of another
/// reason with none of the four is held and written, and reads back with no request member.
#[test]
fn security_the_file_store_writes_no_hold_whose_reason_and_members_disagree() {
    let commission = commission_of(&CaseId("case-sec-file".to_owned()));
    let mut decision_with_request = decision_data(&commission, decision("DR-1"));
    smuggled(&mut decision_with_request, true);
    let mut decision_with_action = decision_data(&commission, decision("DR-1"));
    smuggled(&mut decision_with_action, false);
    let mut authority_without_members =
        decision_data(&commission, SuspensionReason::Authority(Value::Null));
    authority_without_members.action_request_id = None;
    let mut authority_without_arguments =
        decision_data(&commission, SuspensionReason::Authority(Value::Null));
    smuggled(&mut authority_without_arguments, true);
    authority_without_arguments.arguments = None;
    for (name, data, mixed) in [
        ("hold-decision-with-request", decision_with_request, false),
        ("hold-decision-with-action", decision_with_action, true),
        (
            "hold-authority-without-arguments",
            authority_without_arguments,
            true,
        ),
    ] {
        let dir = store_dir(name);
        let store = FileSuspensionStore::open(&dir).expect("the store opens");
        let key = data.suspension.clone();
        let held = store.hold(hold_input(&data));
        if mixed {
            assert_eq!(
                held,
                HoldSuspensionOutcome::IncompleteRequest {
                    error: SuspensionRequestIncomplete { suspension: key },
                },
                "{name}: hold answered {held:?}"
            );
        } else {
            assert!(
                matches!(held, HoldSuspensionOutcome::StorageFailed { .. }),
                "{name}: hold answered {held:?}"
            );
        }
        assert!(!file_of(&dir).exists(), "{name}: a file was written");
    }

    let name = "hold-authority-without-members";
    let dir = store_dir(name);
    let store = FileSuspensionStore::open(&dir).expect("the store opens");
    let held = store.hold(hold_input(&authority_without_members));
    assert!(
        matches!(held, HoldSuspensionOutcome::Held { .. }),
        "{name}: hold answered {held:?}"
    );
    let stored = store
        .get(&authority_without_members.commission_id)
        .expect("the store reads")
        .expect("the hold is stored");
    assert_eq!(stored.state, SuspensionState::Pending, "{name}");
    assert_eq!(stored.data, authority_without_members, "{name}");
    assert!(matches!(stored.data.reason, SuspensionReason::Authority(_)));
    assert_eq!(
        (
            stored.data.action_request_id,
            stored.data.expected_case_revision,
            stored.data.action,
            stored.data.arguments
        ),
        (None, None, None, None),
        "{name}"
    );
}
