//! Security review, wave 2026-11-21-w5, unit U1 (`story:ess-096-upgrade`), pass 1: integrity
//! conformance checks of `HoldSuspension` after decision 2a and `correction-0`.
//!
//! * Every reason crossed with every one of the 16 member sets, on the in-memory store and on
//!   `FileSuspensionStore`, from an empty store: both stores answer the same outcome, store the
//!   same Suspension, and what a refused hold leaves is nothing (invariants 1 to 3).
//! * A mixed hold, and a hold with no request, for a commission that already holds a Suspension
//!   leave its file byte for byte as it was (invariant 1).
//! * Suspension files written by hand: what the decoder reads for every split of the three
//!   request members it tests for `null` (invariant 3).
//! * The named case: a held Suspension of a reason other than `Decision` with no request gives a
//!   port caller nothing a plain `Authority` hold with its request does not: the take and release
//!   that clear it clear a plain hold too, with the request id `get` hands out; it starts no Run,
//!   and neither resume performs anything, `Pending` or `Resuming`.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, BudgetLimit, CaseId, Commission,
    CommissionData, CommissionId, DecidedHold, DecidedRecord, DecisionHold, DecisionUnblocks,
    EffectOutcome, HoldSuspension, HoldSuspensionOutcome, HostStop, HumanDecisionRequest,
    ObservationId, PrincipalId, ProposedActionArguments, ReleaseSuspension,
    ReleaseSuspensionOutcome, RunBudgetBreach, RunId, SuspensionData, SuspensionKey,
    SuspensionReason, SuspensionRequestIncomplete, SuspensionSnapshot, SuspensionState,
    SuspensionStateConflict, TakeSuspension, TakeSuspensionOutcome, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::{SuspensionPort, SuspensionStore};
use b10x_loom_commission::runtime::{
    LoopContext, LoopError, LoopFailure, ResumeError, ResumeRefusal, resume_after_decision,
    resume_after_grant, run_with_suspension,
};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;

const COMMISSION: &str = "00000000-0000-4000-8000-0000000005a1";
const REQUEST: &str = "00000000-0000-4000-8000-0000000005a3";
const CASE: &str = "case-sec-w5-u1";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> CommissionId {
    CommissionId(Uuid(COMMISSION.to_owned()))
}

fn key() -> SuspensionKey {
    SuspensionKey {
        commission_id: commission(),
    }
}

fn incomplete() -> HoldSuspensionOutcome {
    HoldSuspensionOutcome::IncompleteRequest {
        error: SuspensionRequestIncomplete { suspension: key() },
    }
}

/// A hold for the commission with `reason`, carrying the members whose bit is set in `mask`
/// (1 request, 2 expected case revision, 4 action, 8 arguments).
fn hold(reason: SuspensionReason, mask: u8, run: u64) -> HoldSuspension {
    HoldSuspension {
        suspension: key(),
        commission_id: commission(),
        run_id: RunId(uuid(run)),
        run_case_revision: 1,
        action_request_id: (mask & 1 != 0).then(|| ActionRequestId(Uuid(REQUEST.to_owned()))),
        case_id: CaseId(CASE.to_owned()),
        expected_case_revision: (mask & 2 != 0).then_some(1),
        action: (mask & 4 != 0).then(|| "repository.merge".to_owned()),
        arguments: (mask & 8 != 0).then(|| ProposedActionArguments(Value::Object(Vec::new()))),
        reason,
    }
}

/// The Suspension `decide_hold` stores for `input`, `Pending`.
fn stored_for(input: &HoldSuspension) -> SuspensionSnapshot {
    SuspensionSnapshot {
        state: SuspensionState::Pending,
        data: SuspensionData {
            suspension: input.suspension.clone(),
            commission_id: input.commission_id.clone(),
            run_id: input.run_id.clone(),
            run_case_revision: input.run_case_revision,
            action_request_id: input.action_request_id.clone(),
            case_id: input.case_id.clone(),
            expected_case_revision: input.expected_case_revision,
            action: input.action.clone(),
            arguments: input.arguments.clone(),
            reason: input.reason.clone(),
        },
    }
}

fn authority() -> SuspensionReason {
    SuspensionReason::Authority(Value::Object(vec![(
        "capability".to_owned(),
        Value::Text("repository.merge".to_owned()),
    )]))
}

fn decision() -> SuspensionReason {
    SuspensionReason::Decision(DecisionHold {
        request_id: "DR-1".to_owned(),
        unblocks: DecisionUnblocks::Approval,
    })
}

/// Every variant of `SuspensionReason`, `Decision` last.
fn every_reason() -> Vec<SuspensionReason> {
    let payload = || Value::Object(vec![("why".to_owned(), Value::Text("probe".to_owned()))]);
    vec![
        authority(),
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
        decision(),
    ]
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w20261121w5-u1")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every Suspension file in `dir`.
fn suspension_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("suspension-")
        })
        .map(|entry| entry.path())
        .collect();
    files.sort();
    files
}

fn file_of(dir: &Path) -> PathBuf {
    let hex: String = COMMISSION
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    dir.join(format!("suspension-{hex}.json"))
}

/// What the specification and `correction-0` decide for a hold of `reason` with the members of
/// `mask`, from an empty store: `incomplete-request` for a mixed one, `storage-failed` for a
/// `Decision` one with all four, `held` otherwise.
fn decided(reason: &SuspensionReason, mask: u8) -> &'static str {
    match (mask, reason) {
        (1..=14, _) => "incomplete-request",
        (15, SuspensionReason::Decision(_)) => "storage-failed",
        _ => "held",
    }
}

fn kind(outcome: &HoldSuspensionOutcome) -> &'static str {
    match outcome {
        HoldSuspensionOutcome::Settled { .. } => "settled",
        HoldSuspensionOutcome::IncompleteRequest { .. } => "incomplete-request",
        HoldSuspensionOutcome::Held { .. } => "held",
        HoldSuspensionOutcome::AlreadyHeld { .. } => "already-held",
        HoldSuspensionOutcome::StorageFailed { .. } => "storage-failed",
    }
}

/// Invariants 1 to 3: every reason, both stores, from an empty store, over [`CHECK_MASKS`]: no
/// member, each one alone, all but each one and all four. The slow lane's twin takes every member
/// set, the six pairs too.
#[test]
fn security_w5_u1_both_stores_answer_and_keep_the_same_for_every_reason_and_member_set() {
    both_stores_answer_and_keep_the_same("matrix", &CHECK_MASKS);
}

/// [`security_w5_u1_both_stores_answer_and_keep_the_same_for_every_reason_and_member_set`] over
/// all 16 member sets.
#[test]
fn slow_security_w5_u1_both_stores_answer_and_keep_the_same_for_every_reason_and_member_set() {
    let every: Vec<u8> = (0u8..16).collect();
    both_stores_answer_and_keep_the_same("slow-matrix", &every);
}

/// The member sets of `task check`'s matrix: each mask's bits name the request, the expected
/// revision, the action and the arguments ([`hold`]).
const CHECK_MASKS: [u8; 10] = [0, 1, 2, 4, 8, 7, 11, 13, 14, 15];

fn both_stores_answer_and_keep_the_same(lane: &str, masks: &[u8]) {
    let mut wrong = Vec::new();
    for (index, reason) in every_reason().into_iter().enumerate() {
        for mask in masks.iter().copied() {
            let label = format!("{mask:04b} {reason:?}");
            let input = hold(reason.clone(), mask, 0x5b1);
            let memory = SuspensionStore::new();
            let dir = store_dir(&format!("{lane}-{index}-{mask}"));
            let file = FileSuspensionStore::open(&dir).expect("the store opens");

            let in_memory = memory.hold(input.clone());
            let on_file = file.hold(input.clone());
            if in_memory != on_file {
                wrong.push(format!(
                    "{label}: memory answered {in_memory:?}, file {on_file:?}"
                ));
            }
            let expected = decided(&reason, mask);
            for (store, outcome) in [("memory", &in_memory), ("file", &on_file)] {
                if kind(outcome) != expected {
                    wrong.push(format!(
                        "{label}: {store} answered {outcome:?}, not {expected}"
                    ));
                }
                if expected == "incomplete-request" && *outcome != incomplete() {
                    wrong.push(format!(
                        "{label}: {store} named another Suspension: {outcome:?}"
                    ));
                }
            }

            let kept_in_memory = memory.get(&commission()).expect("memory reads");
            let kept_on_file = file.get(&commission()).expect("the file store reads");
            if kept_in_memory != kept_on_file {
                wrong.push(format!(
                    "{label}: memory keeps {kept_in_memory:?}, file {kept_on_file:?}"
                ));
            }
            let files = suspension_files(&dir);
            if expected == "held" {
                if kept_on_file != Some(stored_for(&input)) {
                    wrong.push(format!(
                        "{label}: the file store reads back {kept_on_file:?}"
                    ));
                }
                if files.len() != 1 {
                    wrong.push(format!("{label}: {} Suspension files", files.len()));
                }
            } else {
                if kept_in_memory.is_some() || kept_on_file.is_some() {
                    wrong.push(format!(
                        "{label}: refused, yet memory keeps {kept_in_memory:?}, file {kept_on_file:?}"
                    ));
                }
                if !files.is_empty() {
                    wrong.push(format!("{label}: refused, yet wrote {files:?}"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Invariant 1: for a commission that already holds a Suspension (a full `Authority` hold, a
/// `Budget` hold with no request, a `Decision` hold), every mixed hold of every reason answers
/// `incomplete-request` and every hold with no request `already-held`, and the held Suspension's
/// file is byte for byte as it was, the only one.
#[test]
fn security_w5_u1_a_hold_after_a_held_one_leaves_its_file_byte_for_byte() {
    let mut wrong = Vec::new();
    let firsts = [
        ("full-authority", hold(authority(), 15, 0x5c1)),
        (
            "budget-without-request",
            hold(SuspensionReason::Budget(Value::Null), 0, 0x5c2),
        ),
        ("decision", hold(decision(), 0, 0x5c3)),
    ];
    for (name, first) in firsts {
        let dir = store_dir(&format!("after-held-{name}"));
        let file = FileSuspensionStore::open(&dir).expect("the store opens");
        let memory = SuspensionStore::new();
        for (store, port) in [("file", &file as &dyn SuspensionPort), ("memory", &memory)] {
            let held = port.hold(first.clone());
            assert!(
                matches!(held, HoldSuspensionOutcome::Held { .. }),
                "{name}: {store} answered {held:?}"
            );
        }
        let path = file_of(&dir);
        let bytes = std::fs::read(&path).expect("the held file is there");
        for reason in every_reason() {
            for mask in 0u8..15 {
                let label = format!("{name} then {mask:04b} {reason:?}");
                let expected = if mask == 0 {
                    "already-held"
                } else {
                    "incomplete-request"
                };
                for (store, port) in [("file", &file as &dyn SuspensionPort), ("memory", &memory)] {
                    let outcome = port.hold(hold(reason.clone(), mask, 0x5d1));
                    if kind(&outcome) != expected {
                        wrong.push(format!("{label}: {store} answered {outcome:?}"));
                    }
                    let now = port.get(&commission()).expect("the store reads");
                    if now != Some(stored_for(&first)) {
                        wrong.push(format!("{label}: {store} now keeps {now:?}"));
                    }
                }
                if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                    wrong.push(format!("{label}: the held file changed"));
                }
                if suspension_files(&dir) != vec![path.clone()] {
                    wrong.push(format!("{label}: files are {:?}", suspension_files(&dir)));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A Suspension file as the store writes it, its members as given.
fn file_text(state: &str, members: [&str; 4], reason: &str) -> String {
    let [action_request_id, expected, action, arguments] = members;
    format!(
        "{{\"state\":\"{state}\",\"commission_id\":\"{COMMISSION}\",\
         \"run_id\":\"00000000-0000-4000-8000-0000000005b1\",\"run_case_revision\":1,\
         \"action_request_id\":{action_request_id},\"case_id\":\"{CASE}\",\
         \"expected_case_revision\":{expected},\"action\":{action},\"arguments\":{arguments},\
         \"reason\":{reason}}}"
    )
}

const AUTHORITY: &str =
    "{\"kind\":\"Authority\",\"payload\":{\"capability\":\"repository.merge\"}}";
const BUDGET: &str = "{\"kind\":\"Budget\",\"payload\":null}";
const DECISION: &str =
    "{\"kind\":\"Decision\",\"payload\":{\"request_id\":\"DR-1\",\"unblocks\":\"Approval\"}}";

/// Invariant 3, hand-written files: a file of a reason other than `Decision` reads as holding no
/// request only when all three of `action_request_id`, `expected_case_revision` and `action` are
/// `null` and `arguments` is `null` too. Every other split of the three, with `arguments` `null`
/// or not, is refused by `get`, `take`, `release` and `hold`, and the file stays as written. A
/// `Decision` file with all four `null` reads as before, and a full file with `arguments: null`
/// keeps them as a present `null`.
#[test]
fn security_w5_u1_a_hand_written_file_reads_as_no_request_only_when_all_four_are_null() {
    let request = format!("\"{REQUEST}\"");
    let present = [request.as_str(), "1", "\"repository.merge\""];
    let mut wrong = Vec::new();
    for reason in [AUTHORITY, BUDGET] {
        for split in 0u8..8 {
            for arguments in ["null", "{}"] {
                let member = |bit: u8, index: usize| {
                    if split & bit != 0 {
                        present[index]
                    } else {
                        "null"
                    }
                };
                let members = [member(1, 0), member(2, 1), member(4, 2), arguments];
                let label = format!("{reason} split {split:03b} arguments {arguments}");
                let dir = store_dir(&format!(
                    "hand-{}-{split}-{}",
                    reason.len(),
                    arguments.len()
                ));
                let store = FileSuspensionStore::open(&dir).expect("the store opens");
                let text = file_text("Pending", members, reason);
                std::fs::write(file_of(&dir), &text).unwrap();
                let read = store.get(&commission());
                match (split, arguments) {
                    (0, "null") => match &read {
                        Ok(Some(held))
                            if held.data.action_request_id.is_none()
                                && held.data.expected_case_revision.is_none()
                                && held.data.action.is_none()
                                && held.data.arguments.is_none() => {}
                        other => wrong.push(format!("{label}: read {other:?}")),
                    },
                    (7, _) => match &read {
                        Ok(Some(held))
                            if held.data.action_request_id
                                == Some(ActionRequestId(Uuid(REQUEST.to_owned())))
                                && held.data.arguments.is_some() => {}
                        other => wrong.push(format!("{label}: read {other:?}")),
                    },
                    _ => {
                        if read.is_ok() {
                            wrong.push(format!("{label}: read {read:?}, not refused"));
                        }
                        let taken = store.take(TakeSuspension {
                            suspension: key(),
                            action_request_id: None,
                            decision_request_id: None,
                        });
                        if !matches!(taken, TakeSuspensionOutcome::StorageFailed { .. }) {
                            wrong.push(format!("{label}: take answered {taken:?}"));
                        }
                        let released = store.release(ReleaseSuspension {
                            suspension: key(),
                            action_request_id: None,
                            decision_request_id: None,
                        });
                        if !matches!(released, ReleaseSuspensionOutcome::StorageFailed { .. }) {
                            wrong.push(format!("{label}: release answered {released:?}"));
                        }
                        let held = store.hold(hold(authority(), 0, 0x5e1));
                        if !matches!(held, HoldSuspensionOutcome::StorageFailed { .. }) {
                            wrong.push(format!("{label}: hold answered {held:?}"));
                        }
                        if std::fs::read_to_string(file_of(&dir)).ok().as_deref()
                            != Some(text.as_str())
                        {
                            wrong.push(format!("{label}: the refused file changed"));
                        }
                    }
                }
            }
        }
    }

    let dir = store_dir("hand-decision-none");
    let store = FileSuspensionStore::open(&dir).expect("the store opens");
    std::fs::write(
        file_of(&dir),
        file_text("Resuming", ["null", "null", "null", "null"], DECISION),
    )
    .unwrap();
    match store.get(&commission()) {
        Ok(Some(held))
            if held.state == SuspensionState::Resuming
                && held.data.reason == decision()
                && held.data.action_request_id.is_none()
                && held.data.arguments.is_none() => {}
        other => wrong.push(format!("Decision, all four null: read {other:?}")),
    }

    // Written through the store: a full hold whose arguments are `null` reads back as written.
    let dir = store_dir("round-trip-null-arguments");
    let store = FileSuspensionStore::open(&dir).expect("the store opens");
    let mut full = hold(authority(), 15, 0x5e2);
    full.arguments = Some(ProposedActionArguments(Value::Null));
    let held = store.hold(full.clone());
    if !matches!(held, HoldSuspensionOutcome::Held { .. }) {
        wrong.push(format!("full hold, null arguments: answered {held:?}"));
    }
    let back = store.get(&commission()).expect("the store reads");
    if back != Some(stored_for(&full)) {
        wrong.push(format!("full hold, null arguments: reads back {back:?}"));
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The named case, the comparison: a plain `Authority` hold with its request is refused a take
/// naming neither request, and is taken and released by naming the request id any port caller
/// reads from `get`, which leaves the store empty, as the take and release naming neither leave a
/// hold with no request (`hold_incomplete_request.rs`).
fn a_plain_hold_is_cleared_with_the_id_get_hands_out(port: &dyn SuspensionPort) {
    assert!(matches!(
        port.hold(hold(authority(), 15, 0x5f1)),
        HoldSuspensionOutcome::Held { .. }
    ));
    let neither = port.take(TakeSuspension {
        suspension: key(),
        action_request_id: None,
        decision_request_id: None,
    });
    assert!(
        matches!(neither, TakeSuspensionOutcome::OtherRequest { .. }),
        "a take naming neither answered {neither:?}"
    );
    let read = port
        .get(&commission())
        .expect("the store reads")
        .expect("the hold is stored")
        .data
        .action_request_id;
    let taken = port.take(TakeSuspension {
        suspension: key(),
        action_request_id: read.clone(),
        decision_request_id: None,
    });
    assert!(
        matches!(taken, TakeSuspensionOutcome::Taken { .. }),
        "{taken:?}"
    );
    let released = port.release(ReleaseSuspension {
        suspension: key(),
        action_request_id: read,
        decision_request_id: None,
    });
    assert!(
        matches!(released, ReleaseSuspensionOutcome::Released { .. }),
        "{released:?}"
    );
    assert_eq!(port.get(&commission()).expect("the store reads"), None);
}

#[test]
fn security_w5_u1_a_plain_hold_is_cleared_with_the_id_get_hands_out_on_both_stores() {
    a_plain_hold_is_cleared_with_the_id_get_hands_out(&SuspensionStore::new());
    let store = FileSuspensionStore::open(store_dir("plain-cleared")).expect("the store opens");
    a_plain_hold_is_cleared_with_the_id_get_hands_out(&store);
}

fn assigned() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: commission(),
        agent_revision_id: AgentRevisionId(uuid(0x5a2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-sec-w5".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// An effect port that performs every action and must never be handed one here.
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
        panic!("no request may be admitted here, yet {request:?} was invoked")
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x700 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x800 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-11-21T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The named case, the runtime: for every reason other than `Decision`, a commission holding a
/// Suspension with no request starts no Run (`CommissionSuspended`, as a plain hold), and once a
/// take naming neither made it `Resuming`, `resume_after_grant` still answers the missing request
/// (`ResumeError::Store`) without asking the governor, and `resume_after_decision` refuses it
/// `HeldForAuthority`; nothing is invoked and the Suspension stays as it is.
fn a_hold_without_a_request_moves_no_run(port: &dyn SuspensionPort, reason: SuspensionReason) {
    let label = format!("{reason:?}");
    let assigned = assigned();
    let input = hold(reason, 0, 0x5b1);
    assert!(
        matches!(port.hold(input.clone()), HoldSuspensionOutcome::Held { .. }),
        "{label}"
    );
    let governor = FakeGovernor::new();
    governor.script(assigned.data().case_id.clone(), [Answer::at(1)]);

    let started = run_with_suspension(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &Effects,
        &assigned,
        &mut RunStore::new(|| RunId(uuid(0x5b2))),
        port,
        &mut Context::default(),
    );
    match &started {
        Err(LoopError {
            failure:
                LoopFailure::CommissionSuspended(SuspensionStateConflict {
                    state: SuspensionState::Pending,
                    action_request_id: None,
                }),
            ..
        }) => {}
        other => panic!("{label}: run_with_suspension answered {other:?}"),
    }
    assert_eq!(
        port.get(&commission()).expect("the store reads"),
        Some(stored_for(&input)),
        "{label}"
    );

    let taken = port.take(TakeSuspension {
        suspension: key(),
        action_request_id: None,
        decision_request_id: None,
    });
    assert!(
        matches!(taken, TakeSuspensionOutcome::Taken { .. }),
        "{label}: {taken:?}"
    );
    let resuming = port.get(&commission()).expect("the store reads");
    assert_eq!(
        resuming.as_ref().map(|held| held.state),
        Some(SuspensionState::Resuming),
        "{label}"
    );

    let granted = resume_after_grant(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &Effects,
        &assigned,
        &mut RunStore::new(|| RunId(uuid(0x5b1))),
        port,
        &mut Context::default(),
    );
    match &granted {
        Err(ResumeError::Store(error)) if error.message.contains("holds no action request") => {}
        other => panic!("{label}: resume_after_grant answered {other:?}"),
    }
    let decided = resume_after_decision(
        &governor,
        &ScriptedExecutor::new([]),
        &StaticAuthorityProvider::new(),
        &Effects,
        &assigned,
        &mut RunStore::new(|| RunId(uuid(0x5b1))),
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
        "{label}: resume_after_decision answered {decided:?}"
    );
    assert!(
        governor.calls().is_empty(),
        "{label}: {:?}",
        governor.calls()
    );
    assert_eq!(
        port.get(&commission()).expect("the store reads"),
        resuming,
        "{label}"
    );
}

#[test]
fn security_w5_u1_a_hold_without_a_request_moves_no_run_on_both_stores() {
    let reasons = every_reason();
    for (index, reason) in reasons.iter().take(reasons.len() - 1).enumerate() {
        a_hold_without_a_request_moves_no_run(&SuspensionStore::new(), reason.clone());
        let store =
            FileSuspensionStore::open(store_dir(&format!("moves-no-run-{index}"))).expect("open");
        a_hold_without_a_request_moves_no_run(&store, reason.clone());
    }
}
