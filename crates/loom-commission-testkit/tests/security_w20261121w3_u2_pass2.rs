//! Security review pass 2, wave 2026-11-21-w3, unit U2 (`story:decision-suspension-reason`):
//! integrity conformance checks of correction 1 (a decision hold is taken and released only by
//! naming its decision request; `decide_hold` refuses a hold whose reason and members disagree).
//!
//! * A stale resume for `DR-1` never continues a run held since for `DR-2`: not through its take
//!   (`DR-2` `Pending`), not through the release-first path (`DR-2` `Resuming`, the take answering
//!   `wrong-state`), not when it read the `DR-1` hold `Resuming` and meets `DR-2`; over the
//!   in-memory store and over `FileSuspensionStore`. The `DR-2` hold stays as it was.
//! * A take or release naming an action request, as the approval path does, never consumes a
//!   decision hold.
//! * The in-memory store holds no Suspension whose reason and members disagree.

use std::cell::Cell;
use std::path::PathBuf;

use b10x_loom_commission::model::behaviour::SuspensionStorage;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, DecidedHold, DecidedRecord, DecisionHold, DecisionUnblocks, EffectOutcome,
    EffectOutcomePerformed, ExecutorOutcome, HoldSuspension, HoldSuspensionOutcome, ObservationId,
    PrincipalId, ProposedActionArguments, ReleaseSuspension, ReleaseSuspensionOutcome, RunId,
    SuspensionData, SuspensionKey, SuspensionReason, SuspensionRequestIncomplete,
    SuspensionSnapshot, SuspensionState, TakeSuspension, TakeSuspensionOutcome, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{
    SuspensionPort, SuspensionStore, SuspensionStoreError,
};
use b10x_loom_commission::runtime::{
    LoopContext, LoopEnd, ResumeError, ResumeRefusal, resume_after_decision,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

const RUN: u64 = 0x101;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0x81)),
        agent_revision_id: AgentRevisionId(uuid(0x82)),
        case_id: CaseId("case-sec2".to_owned()),
        principal: PrincipalId("principal-sec2".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

fn decision(request: &str) -> SuspensionReason {
    SuspensionReason::Decision(DecisionHold {
        request_id: request.to_owned(),
        unblocks: DecisionUnblocks::Question,
    })
}

fn data(reason: SuspensionReason) -> SuspensionData {
    let commission_id = CommissionId(uuid(0x81));
    SuspensionData {
        suspension: SuspensionKey {
            commission_id: commission_id.clone(),
        },
        commission_id,
        run_id: RunId(uuid(RUN)),
        run_case_revision: 1,
        action_request_id: None,
        case_id: CaseId("case-sec2".to_owned()),
        expected_case_revision: None,
        action: None,
        arguments: None,
        reason,
    }
}

fn approval() -> SuspensionData {
    let mut held = data(SuspensionReason::Authority(Value::Null));
    held.action_request_id = Some(ActionRequestId(uuid(0x301)));
    held.expected_case_revision = Some(1);
    held.action = Some("repository.merge".to_owned());
    held.arguments = Some(ProposedActionArguments(Value::Null));
    held
}

fn hold_of(data: &SuspensionData) -> HoldSuspension {
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

fn key() -> SuspensionKey {
    SuspensionKey {
        commission_id: CommissionId(uuid(0x81)),
    }
}

fn take(port: &dyn SuspensionPort, decision: Option<&str>) -> TakeSuspensionOutcome {
    port.take(TakeSuspension {
        suspension: key(),
        action_request_id: None,
        decision_request_id: decision.map(str::to_owned),
    })
}

fn release(port: &dyn SuspensionPort, decision: Option<&str>) -> ReleaseSuspensionOutcome {
    port.release(ReleaseSuspension {
        suspension: key(),
        action_request_id: None,
        decision_request_id: decision.map(str::to_owned),
    })
}

fn held(port: &dyn SuspensionPort) -> Option<SuspensionSnapshot> {
    port.get(&CommissionId(uuid(0x81)))
        .expect("the store reads")
}

#[derive(Default)]
struct Effects {
    invoked: Cell<usize>,
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
        self.invoked.set(self.invoked.get() + 1);
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
        Timestamp("2026-11-21T10:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

struct Resumed {
    result: Result<LoopEnd, ResumeError>,
    executor_calls: usize,
    invoked: usize,
}

/// `resume_after_decision` for `request` over `port`, its executor answering once.
fn resume(port: &dyn SuspensionPort, request: &str) -> Resumed {
    let commission = commission();
    let governor = FakeGovernor::new();
    governor.script(commission.data().case_id.clone(), [Answer::at(2)]);
    let executor = ScriptedExecutor::new([ExecutorOutcome::NoUsefulAction(Unit(true))]);
    let effects = Effects::default();
    let result = resume_after_decision(
        &governor,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission,
        &mut RunStore::new(|| RunId(uuid(RUN))),
        port,
        &DecidedHold {
            request_id: request.to_owned(),
            record: DecidedRecord::NoRecord(Unit(true)),
        },
        &mut Context::default(),
    );
    Resumed {
        result,
        executor_calls: executor.calls().len(),
        invoked: effects.invoked.get(),
    }
}

/// What other resumes do to the store between the stale resume's read and its first write.
#[derive(Clone, Copy, Debug)]
enum Meanwhile {
    /// The `DR-1` hold is consumed and the run held again for `DR-2`, `Pending`.
    HeldAgain,
    /// As `HeldAgain`, and a resume for `DR-2` took it and has not released it yet.
    HeldAgainAndTaken,
}

/// A port over `inner` that, before the first take or release it is asked for, lets the other
/// resumes do what `meanwhile` says, through `inner`'s own commands.
struct Interleaved<'s> {
    inner: &'s dyn SuspensionPort,
    meanwhile: Meanwhile,
    fired: Cell<bool>,
}

impl Interleaved<'_> {
    fn others(&self) {
        if self.fired.replace(true) {
            return;
        }
        let state = held(self.inner).expect("the DR-1 hold is stored").state;
        if state == SuspensionState::Pending {
            assert!(matches!(
                take(self.inner, Some("DR-1")),
                TakeSuspensionOutcome::Taken { .. }
            ));
        }
        assert!(matches!(
            release(self.inner, Some("DR-1")),
            ReleaseSuspensionOutcome::Released { .. }
        ));
        assert!(matches!(
            self.inner.hold(hold_of(&data(decision("DR-2")))),
            HoldSuspensionOutcome::Held { .. }
        ));
        if let Meanwhile::HeldAgainAndTaken = self.meanwhile {
            assert!(matches!(
                take(self.inner, Some("DR-2")),
                TakeSuspensionOutcome::Taken { .. }
            ));
        }
    }
}

impl SuspensionPort for Interleaved<'_> {
    fn hold(&self, input: HoldSuspension) -> HoldSuspensionOutcome {
        self.inner.hold(input)
    }

    fn take(&self, input: TakeSuspension) -> TakeSuspensionOutcome {
        self.others();
        self.inner.take(input)
    }

    fn release(&self, input: ReleaseSuspension) -> ReleaseSuspensionOutcome {
        self.others();
        self.inner.release(input)
    }

    fn get(
        &self,
        commission: &CommissionId,
    ) -> Result<Option<SuspensionSnapshot>, SuspensionStoreError> {
        self.inner.get(commission)
    }
}

fn file_store(name: &str) -> FileSuspensionStore {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w20261121w3-u2-pass2")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    FileSuspensionStore::open(dir).expect("the store opens")
}

/// For each store, each state the stale resume read `DR-1` in, and each thing the other resumes
/// did meanwhile: the stale `DR-1` resume continues nothing, performs nothing, and the `DR-2` hold
/// is left exactly as the other resumes left it.
#[test]
fn security_pass2_a_stale_decision_resume_never_continues_a_run_held_for_another_request() {
    let mut failures = Vec::new();
    for store in ["memory", "file"] {
        for read_as in [SuspensionState::Pending, SuspensionState::Resuming] {
            for meanwhile in [Meanwhile::HeldAgain, Meanwhile::HeldAgainAndTaken] {
                let label = format!("{store} {read_as:?} {meanwhile:?}");
                let memory = SuspensionStore::new();
                let file = file_store(&format!("{read_as:?}-{meanwhile:?}"));
                let inner: &dyn SuspensionPort = if store == "memory" { &memory } else { &file };
                assert!(matches!(
                    inner.hold(hold_of(&data(decision("DR-1")))),
                    HoldSuspensionOutcome::Held { .. }
                ));
                if read_as == SuspensionState::Resuming {
                    // A resume for DR-1 took it and stopped before its release.
                    assert!(matches!(
                        take(inner, Some("DR-1")),
                        TakeSuspensionOutcome::Taken { .. }
                    ));
                }
                let port = Interleaved {
                    inner,
                    meanwhile,
                    fired: Cell::new(false),
                };

                let resumed = resume(&port, "DR-1");

                let expected_state = match meanwhile {
                    Meanwhile::HeldAgain => SuspensionState::Pending,
                    Meanwhile::HeldAgainAndTaken => SuspensionState::Resuming,
                };
                let left = held(inner);
                let kept = left.as_ref().is_some_and(|left| {
                    left.state == expected_state && left.data == data(decision("DR-2"))
                });
                let continued = resumed.result.is_ok() || resumed.executor_calls > 0;
                if !kept || continued || resumed.invoked > 0 {
                    failures.push(format!(
                        "{label}: resume ended {:?}, executor ran {}, invoked {}, DR-2 hold now \
                         {left:?}",
                        resumed.result.as_ref().map(|end| &end.outcome),
                        resumed.executor_calls,
                        resumed.invoked
                    ));
                }
                if let Err(ResumeError::Refused(refusal)) = &resumed.result {
                    let named = ResumeRefusal::OtherDecision {
                        request_id: "DR-1".to_owned(),
                        held_request_id: "DR-2".to_owned(),
                    };
                    if refusal != &named {
                        failures.push(format!("{label}: refused {refusal:?}, not {named:?}"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A take or release naming an action request, as `resume_after_grant` and the approval path
/// name one, never consumes a decision hold, `Pending` or `Resuming`, in either store.
#[test]
fn security_pass2_a_take_or_release_naming_an_action_request_never_consumes_a_decision_hold() {
    let memory = SuspensionStore::new();
    let file = file_store("action-request");
    for (name, port) in [
        ("memory", &memory as &dyn SuspensionPort),
        ("file", &file as &dyn SuspensionPort),
    ] {
        assert!(matches!(
            port.hold(hold_of(&data(decision("DR-1")))),
            HoldSuspensionOutcome::Held { .. }
        ));
        for decision in [None, Some("DR-1")] {
            let request = Some(ActionRequestId(uuid(0x301)));
            let pending = held(port);
            let taken = port.take(TakeSuspension {
                suspension: key(),
                action_request_id: request.clone(),
                decision_request_id: decision.map(str::to_owned),
            });
            assert!(
                matches!(taken, TakeSuspensionOutcome::OtherRequest { .. }),
                "{name} {decision:?}: take answered {taken:?}"
            );
            assert_eq!(held(port), pending, "{name}: the take changed the hold");
        }
        assert!(matches!(
            take(port, Some("DR-1")),
            TakeSuspensionOutcome::Taken { .. }
        ));
        for decision in [None, Some("DR-1")] {
            let resuming = held(port);
            let released = port.release(ReleaseSuspension {
                suspension: key(),
                action_request_id: Some(ActionRequestId(uuid(0x301))),
                decision_request_id: decision.map(str::to_owned),
            });
            assert!(
                matches!(released, ReleaseSuspensionOutcome::OtherRequest { .. }),
                "{name} {decision:?}: release answered {released:?}"
            );
            assert_eq!(held(port), resuming, "{name}: the release changed the hold");
        }
    }
}

/// An approval hold is taken and released by its action request id as before, and a take or
/// release that also names a decision request is refused and changes nothing.
#[test]
fn security_pass2_an_approval_hold_is_taken_as_before_and_never_by_a_decision() {
    let memory = SuspensionStore::new();
    let file = file_store("approval");
    for (name, port) in [
        ("memory", &memory as &dyn SuspensionPort),
        ("file", &file as &dyn SuspensionPort),
    ] {
        assert!(matches!(
            port.hold(hold_of(&approval())),
            HoldSuspensionOutcome::Held { .. }
        ));
        let request = approval().action_request_id;
        let before = held(port);
        let misnamed = port.take(TakeSuspension {
            suspension: key(),
            action_request_id: request.clone(),
            decision_request_id: Some("DR-1".to_owned()),
        });
        assert!(
            matches!(misnamed, TakeSuspensionOutcome::OtherDecision { .. }),
            "{name}: take answered {misnamed:?}"
        );
        assert_eq!(held(port), before, "{name}");
        assert!(matches!(
            port.take(TakeSuspension {
                suspension: key(),
                action_request_id: request.clone(),
                decision_request_id: None,
            }),
            TakeSuspensionOutcome::Taken { .. }
        ));
        let resuming = held(port);
        let misnamed = port.release(ReleaseSuspension {
            suspension: key(),
            action_request_id: request.clone(),
            decision_request_id: Some("DR-1".to_owned()),
        });
        assert!(
            matches!(misnamed, ReleaseSuspensionOutcome::OtherDecision { .. }),
            "{name}: release answered {misnamed:?}"
        );
        assert_eq!(held(port), resuming, "{name}");
        assert!(matches!(
            port.release(ReleaseSuspension {
                suspension: key(),
                action_request_id: request,
                decision_request_id: None,
            }),
            ReleaseSuspensionOutcome::Released { .. }
        ));
        assert_eq!(held(port), None, "{name}");
    }
}

/// The in-memory store (the one the conformance target holds through) refuses every hold whose
/// reason and four request members disagree, and stores nothing: one giving some but not all four
/// `IncompleteRequest`, naming its Suspension (`story:ess-096-upgrade`), a `Decision` hold with all
/// four `StorageFailed`; the two agreeing holds are held, and so is a hold of another reason with
/// none of the four, stored with that reason and no request member.
#[test]
fn security_pass2_the_memory_store_holds_no_suspension_whose_reason_and_members_disagree() {
    let full = approval();
    let mut forbidden = Vec::new();
    for mask in 0u8..16 {
        for reason in [decision("DR-1"), SuspensionReason::Authority(Value::Null)] {
            let mut held = data(reason.clone());
            if mask & 1 != 0 {
                held.action_request_id = full.action_request_id.clone();
            }
            if mask & 2 != 0 {
                held.expected_case_revision = full.expected_case_revision;
            }
            if mask & 4 != 0 {
                held.action = full.action.clone();
            }
            if mask & 8 != 0 {
                held.arguments = full.arguments.clone();
            }
            let decision_hold = matches!(reason, SuspensionReason::Decision(_));
            let agrees = mask == 0 || (!decision_hold && mask == 15);
            let mixed = mask != 0 && mask != 15;
            let store = SuspensionStore::new();
            let outcome = store.hold(hold_of(&held));
            let stored = SuspensionPort::get(&store, &CommissionId(uuid(0x81))).unwrap();
            let refused = if mixed {
                outcome
                    == HoldSuspensionOutcome::IncompleteRequest {
                        error: SuspensionRequestIncomplete { suspension: key() },
                    }
            } else {
                matches!(outcome, HoldSuspensionOutcome::StorageFailed { .. })
            };
            if agrees {
                assert!(
                    matches!(outcome, HoldSuspensionOutcome::Held { .. }),
                    "{mask:04b} {reason:?}: {outcome:?}"
                );
                if !decision_hold && mask == 0 {
                    let stored = stored.expect("the hold is stored");
                    assert_eq!(stored.data.reason, reason, "{mask:04b}");
                    assert_eq!(
                        (
                            stored.data.action_request_id,
                            stored.data.expected_case_revision,
                            stored.data.action,
                            stored.data.arguments
                        ),
                        (None, None, None, None),
                        "{mask:04b} {reason:?}"
                    );
                }
            } else if !refused || stored.is_some() {
                forbidden.push(format!(
                    "{mask:04b} {reason:?}: {outcome:?}, stored {stored:?}"
                ));
            }
        }
    }
    // The conformance codec's reading of `arguments: null` on a Decision hold.
    let mut null_arguments = data(decision("DR-1"));
    null_arguments.arguments = Some(ProposedActionArguments(Value::Null));
    let store = SuspensionStore::new();
    let outcome = store.hold(hold_of(&null_arguments));
    if outcome
        != (HoldSuspensionOutcome::IncompleteRequest {
            error: SuspensionRequestIncomplete { suspension: key() },
        })
        || !SuspensionStorage::list(&store).is_empty()
    {
        forbidden.push(format!("null arguments on a Decision hold: {outcome:?}"));
    }
    assert!(forbidden.is_empty(), "{forbidden:#?}");
}
