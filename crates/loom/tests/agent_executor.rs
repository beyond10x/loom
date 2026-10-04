//! Acceptance for `story:agent-executor`: Loom implements the Commission `AgentExecutor` on model
//! types synthesized from `ess/`, stops at an approval-gated action with `Suspended` instead of
//! proposing it, and imports nothing from Canon.
//!
//! The frontier the scripted fake governor serves is transcribed from ELS
//! `docs/examples/software-change.md` (case CHG-1842): ELS `software.change/1` has no machine
//! fixture yet (els `story:software-change-protocol` is a draft), so the two frontiers below are
//! that example's "Initial" and "After `tests.run` on R2" states, typed as Commission's generated
//! `Frontier` items. The frontier is Commission's own generated entity, the one its executor port
//! takes; Canon's `Frontier` is not used.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time:
//! a build directory shared between worktrees reuses binaries across them.

use std::path::PathBuf;

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended,
    Frontier, FrontierAction, FrontierClaim, PrincipalId, SuspensionReason, Truth,
    commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{CommissionRunId, Session, SessionData, SessionId, SessionState};
use b10x_loom::{ActionSelector, EmptyObjectArguments, FirstAdmissibleSelector, Loom};

/// The action the approval stop is about.
const MERGE: &str = "repository.merge";

/// The capability the frontier names for the approval-gated merge.
const MERGE_CAPABILITY: &str = "repository.write";

/// More invocations than the scripted case needs to reach the approval stop.
const MAX_INVOCATIONS: usize = 8;

fn action(
    name: &str,
    status: ActionStatus,
    capability: Option<&str>,
    reasons: &[&str],
) -> FrontierAction {
    FrontierAction {
        action: name.to_owned(),
        status,
        capability: capability.map(str::to_owned),
        reasons: reasons.iter().map(|reason| (*reason).to_owned()).collect(),
    }
}

/// ELS `docs/examples/software-change.md`, "Initial": `tests.pass` is unknown, and merge is
/// blocked while inspect, edit and `tests.run` are admissible.
fn initial() -> Answer {
    Answer::at(1).with_items(
        vec![FrontierClaim {
            claim: "tests.pass".to_owned(),
            value: Truth::Unknown,
        }],
        Vec::new(),
        vec![
            action("repository.inspect", ActionStatus::Admissible, None, &[]),
            action("repository.edit", ActionStatus::Admissible, None, &[]),
            action("tests.run", ActionStatus::Admissible, None, &[]),
            action(
                MERGE,
                ActionStatus::Blocked,
                None,
                &["tests.pass is Unknown, required True"],
            ),
        ],
    )
}

/// ELS `docs/examples/software-change.md`, "After `tests.run` on R2": `tests.pass` is true and
/// merge needs approval. The other actions stay admissible.
fn after_tests_on_r2() -> Answer {
    Answer::at(2).with_items(
        vec![FrontierClaim {
            claim: "tests.pass".to_owned(),
            value: Truth::True,
        }],
        Vec::new(),
        vec![
            action("repository.inspect", ActionStatus::Admissible, None, &[]),
            action("repository.edit", ActionStatus::Admissible, None, &[]),
            action("tests.run", ActionStatus::Admissible, None, &[]),
            action(
                MERGE,
                ActionStatus::ApprovalRequired,
                Some(MERGE_CAPABILITY),
                &[],
            ),
        ],
    )
}

/// A selector that wants to merge, as a model asked to land a change would: it picks
/// `repository.merge` whenever the frontier lists it as anything but `Blocked`, and otherwise
/// whatever `FirstAdmissibleSelector` picks.
struct MergeSeeking;

impl ActionSelector for MergeSeeking {
    fn select(
        &self,
        frontier: &Frontier<frontier_state::Issued>,
        prompt: &str,
    ) -> Result<String, String> {
        let merge_open = frontier
            .data()
            .actions
            .iter()
            .any(|listed| listed.action == MERGE && listed.status != ActionStatus::Blocked);
        if merge_open {
            Ok(MERGE.to_owned())
        } else {
            FirstAdmissibleSelector.select(frontier, prompt)
        }
    }
}

fn commission(case: &CaseId) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000001".to_owned(),
        )),
        agent_revision_id: AgentRevisionId(CommissionUuid(
            "00000000-0000-4000-8000-000000000002".to_owned(),
        )),
        case_id: case.clone(),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// Acceptance 2: Loom, as the `AgentExecutor`, is invoked on each frontier the scripted governor
/// issues for CHG-1842 until it returns `Suspended`. It suspends naming the merge capability, and
/// no invocation proposes `repository.merge`.
#[test]
fn executor_suspends_at_merge_approval() {
    let case = CaseId("CHG-1842".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [initial(), after_tests_on_r2()]);
    let commission = commission(&case);
    let loom = Loom::new(MergeSeeking, EmptyObjectArguments, "land the change");

    let mut outcomes = Vec::new();
    let mut suspended = None;
    for _ in 0..MAX_INVOCATIONS {
        let frontier = governor
            .frontier(&case)
            .unwrap_or_else(|error| panic!("frontier for {} failed: {error:?}", case.0));
        let outcome = loom.run(&commission, &frontier);
        outcomes.push(outcome.clone());
        if let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended { reason }) = outcome {
            suspended = Some(reason);
            break;
        }
    }

    let proposed_merge: Vec<&ExecutorOutcome> = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction { action, .. })
                    if action == MERGE
            )
        })
        .collect();
    assert!(
        proposed_merge.is_empty(),
        "Loom proposed {MERGE}, which needs approval: {proposed_merge:?} (all outcomes: {outcomes:?})"
    );

    assert!(
        outcomes.len() > 1,
        "the approval stop came on the first invocation, while merge was still Blocked: {outcomes:?}"
    );
    assert!(
        matches!(
            &outcomes[0],
            ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction { action, .. })
                if action == "repository.inspect"
        ),
        "on the initial frontier Loom should propose the first admissible action: {:?}",
        outcomes[0]
    );

    let reason = suspended.unwrap_or_else(|| {
        panic!("Loom never returned Suspended in {MAX_INVOCATIONS} invocations: {outcomes:?}")
    });
    match &reason {
        SuspensionReason::Authority(detail) => assert_eq!(
            detail.member("capability"),
            Some(&Value::Text(MERGE_CAPABILITY.to_owned())),
            "the authority suspension does not name the merge capability: {detail:?}"
        ),
        other => panic!("Loom suspended for a reason other than authority: {other:?}"),
    }
}

/// Acceptance 3: a generated `Session`, built through `b10x-loom`'s re-export, carries the
/// `CommissionRunId` it was built with.
#[test]
fn session_carries_commission_run_id() {
    let run = CommissionRunId(Uuid("00000000-0000-4000-8000-0000000000a1".to_owned()));
    let session = Session::new(SessionData {
        session_id: SessionId(Uuid("00000000-0000-4000-8000-0000000000b2".to_owned())),
        commission_run: run.clone(),
    });

    assert_eq!(session.state(), SessionState::Active);
    assert_eq!(session.data().commission_run, run);
    assert_eq!(session.into_data().commission_run, run);
}

/// Acceptance 6: `crates/loom/src/lib.rs` imports no `b10x_canon` item.
#[test]
fn lib_imports_no_canon() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let path = PathBuf::from(manifest).join("src/lib.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));

    let canon_lines: Vec<String> = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("b10x_canon"))
        .map(|(index, line)| format!("{}:{}: {}", path.display(), index + 1, line.trim()))
        .collect();
    assert!(
        canon_lines.is_empty(),
        "lib.rs names b10x_canon:\n{}",
        canon_lines.join("\n")
    );
}
