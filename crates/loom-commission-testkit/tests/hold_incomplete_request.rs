//! `story:ess-096-upgrade`, decision 2a: a hold with some but not all of its four request members
//! (`action_request_id`, `expected_case_revision`, `action`, `arguments`) is refused
//! `incomplete-request`, naming the Suspension, whatever its reason, and this check runs before
//! `already-held`: a mixed hold for a commission that already holds a Suspension answers
//! `incomplete-request` and the held Suspension stays as it was. Over the in-memory store and over
//! `FileSuspensionStore`.
//!
//! A hold of a reason other than `Decision` with none of the four is held and reads back equal; a
//! `Decision` hold with all four is refused `storage-failed` and nothing is stored. What a take, a
//! release and the two resumes do with such a held Suspension, which holds no request, is
//! asserted on both stores.

use std::path::PathBuf;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, BudgetLimit, CaseId, Commission,
    CommissionData, CommissionId, DecidedHold, DecidedRecord, DecisionHold, DecisionUnblocks,
    EffectOutcome, HoldSuspension, HoldSuspensionOutcome, HostStop, HumanDecisionRequest,
    ObservationId, PrincipalId, ProposedActionArguments, ReleaseSuspension,
    ReleaseSuspensionOutcome, RunBudgetBreach, RunId, SuspensionKey, SuspensionReason,
    SuspensionRequestIncomplete, SuspensionState, TakeSuspension, TakeSuspensionOutcome, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, ResumeError, ResumeRefusal, resume_after_decision, resume_after_grant,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> CommissionId {
    CommissionId(uuid(0x91))
}

fn key() -> SuspensionKey {
    SuspensionKey {
        commission_id: commission(),
    }
}

/// A hold for the commission with `reason`, carrying the members whose bit is set in `mask`
/// (1 request, 2 expected case revision, 4 action, 8 arguments).
fn hold(reason: SuspensionReason, mask: u8) -> HoldSuspension {
    HoldSuspension {
        suspension: key(),
        commission_id: commission(),
        run_id: RunId(uuid(0x101)),
        run_case_revision: 1,
        action_request_id: (mask & 1 != 0).then(|| ActionRequestId(uuid(0x301))),
        case_id: CaseId("case-incomplete".to_owned()),
        expected_case_revision: (mask & 2 != 0).then_some(1),
        action: (mask & 4 != 0).then(|| "repository.merge".to_owned()),
        arguments: (mask & 8 != 0).then(|| ProposedActionArguments(Value::Object(Vec::new()))),
        reason,
    }
}

fn reasons() -> [SuspensionReason; 2] {
    [
        SuspensionReason::Authority(Value::Null),
        SuspensionReason::Decision(DecisionHold {
            request_id: "DR-1".to_owned(),
            unblocks: DecisionUnblocks::Question,
        }),
    ]
}

/// Holds a full `Authority` hold on `port`, then sends every mixed hold (14 member sets per
/// reason, both reasons) for the same commission, and returns every answer that is not
/// `IncompleteRequest` naming the Suspension, or that left the held Suspension changed.
fn mixed_holds_after_a_held_one(port: &dyn SuspensionPort) -> Vec<String> {
    let held = port.hold(hold(SuspensionReason::Authority(Value::Null), 15));
    assert!(
        matches!(held, HoldSuspensionOutcome::Held { .. }),
        "the full hold answered {held:?}"
    );
    let first = port
        .get(&commission())
        .expect("the store reads")
        .expect("the full hold is stored");
    let mut wrong = Vec::new();
    for reason in reasons() {
        for mask in 1u8..15 {
            let outcome = port.hold(hold(reason.clone(), mask));
            let expected = HoldSuspensionOutcome::IncompleteRequest {
                error: SuspensionRequestIncomplete { suspension: key() },
            };
            if outcome != expected {
                wrong.push(format!("{mask:04b} {reason:?}: answered {outcome:?}"));
            }
            let now = port.get(&commission()).expect("the store reads");
            if now.as_ref() != Some(&first) {
                wrong.push(format!(
                    "{mask:04b} {reason:?}: the held one is now {now:?}"
                ));
            }
        }
    }
    wrong
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("hold-incomplete-request")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_mixed_hold_after_a_held_one_is_incomplete_on_the_memory_store() {
    let wrong = mixed_holds_after_a_held_one(&SuspensionStore::new());
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_mixed_hold_after_a_held_one_is_incomplete_on_the_file_store() {
    let store = FileSuspensionStore::open(store_dir("after-held")).expect("the store opens");
    let wrong = mixed_holds_after_a_held_one(&store);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Every reason other than `Decision`.
fn other_reasons() -> Vec<SuspensionReason> {
    let payload = || Value::Object(vec![("why".to_owned(), Value::Text("probe".to_owned()))]);
    vec![
        SuspensionReason::Authority(payload()),
        SuspensionReason::Budget(payload()),
        SuspensionReason::Dependency(vec![CaseId("case-other".to_owned())]),
        SuspensionReason::Evidence(vec!["review".to_owned()]),
        SuspensionReason::ExternalAvailability(payload()),
        SuspensionReason::HostStop(HostStop {
            name: "maintenance".to_owned(),
            detail: payload(),
        }),
        SuspensionReason::Human(HumanDecisionRequest(payload())),
        SuspensionReason::LoopBudget(payload()),
        SuspensionReason::RunBudget(RunBudgetBreach {
            action: None,
            limit: BudgetLimit::MaxSteps,
            max: 3,
            charged: Some(4),
        }),
        SuspensionReason::Time(payload()),
    ]
}

fn decision() -> SuspensionReason {
    SuspensionReason::Decision(DecisionHold {
        request_id: "DR-1".to_owned(),
        unblocks: DecisionUnblocks::Question,
    })
}

/// (a) A hold of each reason other than `Decision` with none of the four is held and reads back
/// equal, `Pending`, with no request member. (b) A `Decision` hold with all four is refused
/// `storage-failed` and nothing is stored.
fn holds_without_a_request(open: &dyn Fn(&str) -> Box<dyn SuspensionPort>) -> Vec<String> {
    let mut wrong = Vec::new();
    for (index, reason) in other_reasons().into_iter().enumerate() {
        let port = open(&format!("without-request-{index}"));
        let input = hold(reason.clone(), 0);
        let outcome = port.hold(input.clone());
        if !matches!(outcome, HoldSuspensionOutcome::Held { .. }) {
            wrong.push(format!("{reason:?}: answered {outcome:?}"));
            continue;
        }
        let stored = port.get(&commission()).expect("the store reads");
        let back = stored.as_ref().map(|stored| {
            let data = &stored.data;
            (
                stored.state,
                &data.reason,
                &data.run_id,
                &data.case_id,
                data.run_case_revision,
                data.action_request_id.is_none()
                    && data.expected_case_revision.is_none()
                    && data.action.is_none()
                    && data.arguments.is_none(),
            )
        });
        let expected = (
            SuspensionState::Pending,
            &input.reason,
            &input.run_id,
            &input.case_id,
            input.run_case_revision,
            true,
        );
        if back != Some(expected) {
            wrong.push(format!("{reason:?}: reads back as {stored:?}"));
        }
    }
    let port = open("decision-with-all-four");
    let outcome = port.hold(hold(decision(), 15));
    if !matches!(outcome, HoldSuspensionOutcome::StorageFailed { .. }) {
        wrong.push(format!("Decision with all four: answered {outcome:?}"));
    }
    let stored = port.get(&commission()).expect("the store reads");
    if stored.is_some() {
        wrong.push(format!("Decision with all four: stored {stored:?}"));
    }
    wrong
}

fn memory(_: &str) -> Box<dyn SuspensionPort> {
    Box::new(SuspensionStore::new())
}

fn file(name: &str) -> Box<dyn SuspensionPort> {
    Box::new(FileSuspensionStore::open(store_dir(name)).expect("the store opens"))
}

#[test]
fn a_hold_without_a_request_is_held_unless_its_reason_is_a_decision_on_the_memory_store() {
    let wrong = holds_without_a_request(&memory);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_hold_without_a_request_is_held_unless_its_reason_is_a_decision_on_the_file_store() {
    let wrong = holds_without_a_request(&file);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// (c) A held `Authority` Suspension with no request: a take naming a request is refused
/// `other-request`, one naming a decision request `other-decision`, and both keep it; a take
/// naming neither takes it, and a release naming neither releases it.
fn take_and_release_without_a_request(port: &dyn SuspensionPort) {
    let reason = SuspensionReason::Authority(Value::Null);
    assert!(matches!(
        port.hold(hold(reason, 0)),
        HoldSuspensionOutcome::Held { .. }
    ));
    let held = port.get(&commission()).expect("the store reads");
    let take = |request: Option<ActionRequestId>, decision: Option<&str>| {
        port.take(TakeSuspension {
            suspension: key(),
            action_request_id: request,
            decision_request_id: decision.map(str::to_owned),
        })
    };
    let named = take(Some(ActionRequestId(uuid(0x301))), None);
    assert!(
        matches!(named, TakeSuspensionOutcome::OtherRequest { .. }),
        "take naming a request answered {named:?}"
    );
    let decided = take(None, Some("DR-1"));
    assert!(
        matches!(decided, TakeSuspensionOutcome::OtherDecision { .. }),
        "take naming a decision answered {decided:?}"
    );
    assert_eq!(port.get(&commission()).expect("the store reads"), held);
    let taken = take(None, None);
    assert!(
        matches!(taken, TakeSuspensionOutcome::Taken { .. }),
        "take naming neither answered {taken:?}"
    );
    assert_eq!(
        port.get(&commission())
            .expect("the store reads")
            .map(|stored| stored.state),
        Some(SuspensionState::Resuming)
    );
    let released = port.release(ReleaseSuspension {
        suspension: key(),
        action_request_id: None,
        decision_request_id: None,
    });
    assert!(
        matches!(released, ReleaseSuspensionOutcome::Released { .. }),
        "release naming neither answered {released:?}"
    );
    assert_eq!(port.get(&commission()).expect("the store reads"), None);
}

#[test]
fn a_held_suspension_without_a_request_is_taken_and_released_naming_none_on_the_memory_store() {
    take_and_release_without_a_request(&SuspensionStore::new());
}

#[test]
fn a_held_suspension_without_a_request_is_taken_and_released_naming_none_on_the_file_store() {
    take_and_release_without_a_request(&*file("take-release"));
}

fn assigned() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: commission(),
        agent_revision_id: AgentRevisionId(uuid(0x92)),
        case_id: CaseId("case-incomplete".to_owned()),
        principal: PrincipalId("principal-incomplete".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// An effect port that performs every action and is never handed one here.
struct Effects;

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        panic!("no request is admitted here, yet {request:?} was invoked")
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x500 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x600 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// (c) The runtime's resumes of a held `Authority` Suspension with no request: `resume_after_grant`
/// answers `ResumeError::Store` naming the missing request before it takes anything or asks the
/// governor, and `resume_after_decision` refuses it `HeldForAuthority`; the Suspension stays
/// `Pending` as held and nothing is invoked.
fn resumes_without_a_request(port: &dyn SuspensionPort) {
    let assigned = assigned();
    assert!(matches!(
        port.hold(hold(SuspensionReason::Authority(Value::Null), 0)),
        HoldSuspensionOutcome::Held { .. }
    ));
    let held = port.get(&commission()).expect("the store reads");
    let governor = FakeGovernor::new();
    governor.script(assigned.data().case_id.clone(), [Answer::at(1)]);

    let granted = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &Effects,
        &assigned,
        &mut RunStore::new(|| RunId(uuid(0x101))),
        port,
        &mut Context::default(),
    );
    match &granted {
        Err(ResumeError::Store(error)) => assert!(
            error.message.contains("holds no action request"),
            "resume_after_grant answered {error}"
        ),
        other => panic!("resume_after_grant answered {other:?}"),
    }
    assert!(governor.calls().is_empty(), "{:?}", governor.calls());
    assert_eq!(port.get(&commission()).expect("the store reads"), held);

    let decided = resume_after_decision(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &Effects,
        &assigned,
        &mut RunStore::new(|| RunId(uuid(0x101))),
        port,
        &DecidedHold {
            request_id: "DR-1".to_owned(),
            record: DecidedRecord::NoRecord(Unit(true)),
        },
        &mut Context::default(),
    );
    assert!(
        matches!(
            decided,
            Err(ResumeError::Refused(ResumeRefusal::HeldForAuthority))
        ),
        "resume_after_decision answered {decided:?}"
    );
    assert!(governor.calls().is_empty(), "{:?}", governor.calls());
    assert_eq!(port.get(&commission()).expect("the store reads"), held);
}

#[test]
fn the_resumes_of_a_suspension_without_a_request_perform_nothing_on_the_memory_store() {
    resumes_without_a_request(&SuspensionStore::new());
}

#[test]
fn the_resumes_of_a_suspension_without_a_request_perform_nothing_on_the_file_store() {
    resumes_without_a_request(&*file("resumes"));
}

/// `hold(reason, mask)` whose Suspension identity names another commission than its
/// `commission_id`.
fn hold_misnamed(reason: SuspensionReason, mask: u8) -> HoldSuspension {
    HoldSuspension {
        suspension: SuspensionKey {
            commission_id: CommissionId(uuid(0x93)),
        },
        ..hold(reason, mask)
    }
}

/// What a store holds under either commission the misnamed hold names.
fn held_under_both(port: &dyn SuspensionPort) -> [Option<String>; 2] {
    [commission(), CommissionId(uuid(0x93))].map(|named| {
        port.get(&named)
            .map(|held| held.map(|held| format!("{held:?}")))
            .unwrap_or_else(|error| Some(format!("unreadable: {error}")))
    })
}

fn suspension_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("suspension-")
        })
        .count()
}

/// The specification decides `incomplete-request` before every other outcome, so a mixed hold
/// whose Suspension identity names another commission than its `commission_id` is refused
/// `incomplete-request` by both stores, for each reason and each of the 14 mixed member sets,
/// and neither store holds anything after it nor writes a file.
#[test]
fn a_mixed_hold_naming_another_commission_is_incomplete_on_both_stores() {
    let mut wrong = Vec::new();
    let mut reasons = other_reasons();
    reasons.push(decision());
    for (index, reason) in reasons.into_iter().enumerate() {
        for mask in 1u8..15 {
            let input = hold_misnamed(reason.clone(), mask);
            let expected = HoldSuspensionOutcome::IncompleteRequest {
                error: SuspensionRequestIncomplete {
                    suspension: input.suspension.clone(),
                },
            };
            let name = format!("misnamed-{index}-{mask}");
            let dir = store_dir(&name);
            let on_file = FileSuspensionStore::open(&dir).expect("the store opens");
            let in_memory = SuspensionStore::new();
            for (store, port) in [
                ("memory", &in_memory as &dyn SuspensionPort),
                ("file", &on_file),
            ] {
                let outcome = port.hold(input.clone());
                if outcome != expected {
                    wrong.push(format!(
                        "{mask:04b} {reason:?}: {store} answered {outcome:?}"
                    ));
                }
                let held = held_under_both(port);
                if held != [None, None] {
                    wrong.push(format!("{mask:04b} {reason:?}: {store} holds {held:?}"));
                }
            }
            if suspension_files(&dir) != 0 {
                wrong.push(format!(
                    "{mask:04b} {reason:?}: the file store wrote a file"
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A misnamed hold that is not mixed, as each store answers it: the in-memory store decides it by
/// the identity alone and holds it under the identity's commission; the file store refuses it
/// `StorageFailed` before deciding and writes nothing. The two stores differ here, as they did at
/// base `29e5d199` for every hold `decide_hold` held; this case states the difference and does
/// not settle it.
#[test]
fn a_misnamed_hold_with_all_four_or_none_is_held_in_memory_and_refused_by_the_file_store() {
    for (reason, mask) in [
        (SuspensionReason::Authority(Value::Null), 15),
        (SuspensionReason::Authority(Value::Null), 0),
        (decision(), 0),
    ] {
        let label = format!("{mask:04b} {reason:?}");
        let in_memory = SuspensionStore::new();
        let outcome = in_memory.hold(hold_misnamed(reason.clone(), mask));
        assert!(
            matches!(outcome, HoldSuspensionOutcome::Held { .. }),
            "{label}: memory answered {outcome:?}"
        );
        let held = held_under_both(&in_memory);
        assert!(
            held[0].is_none() && held[1].is_some(),
            "{label}: memory holds {held:?}"
        );

        let dir = store_dir(&format!("misnamed-whole-{mask}-{}", label.len()));
        let on_file = FileSuspensionStore::open(&dir).expect("the store opens");
        let outcome = on_file.hold(hold_misnamed(reason, mask));
        assert!(
            matches!(outcome, HoldSuspensionOutcome::StorageFailed { .. }),
            "{label}: file answered {outcome:?}"
        );
        assert_eq!(
            suspension_files(&dir),
            0,
            "{label}: the file store wrote a file"
        );
    }
}
