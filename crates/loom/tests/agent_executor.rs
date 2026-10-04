//! Acceptance for `story:agent-executor`: Loom implements the Commission `AgentExecutor` on model
//! types synthesized from `ess/`, proposes an approval-gated action for Commission to authorize
//! (Atlas ADR 0082) rather than suspending for authority itself, and imports nothing from Canon.
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

use b10x_commission::admission::admit;
use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid as CommissionUuid;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, AdmissionNeedsAuthority, AgentRevisionId, AuthorityContext, CaseId,
    Commission, CommissionData, CommissionId, ExecutorOutcome, ExecutorOutcomeProposedAction,
    Frontier, FrontierAction, FrontierClaim, PrincipalId, Truth, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;
use b10x_commission::ports::governor::Governor;
use b10x_commission_testkit::fake_governor::{Answer, FakeGovernor};
use b10x_loom::model::primitives::Uuid;
use b10x_loom::model::run::{CommissionRunId, Session, SessionData, SessionId, SessionState};
use b10x_loom::{
    ActionSelector, EmptyObjectArguments, FirstAdmissibleSelector, Loom, SelectorError,
};

/// The approval-gated action.
const MERGE: &str = "repository.merge";

/// The capability the frontier names for the approval-gated merge.
const MERGE_CAPABILITY: &str = "repository.write";

/// More invocations than the scripted case needs to reach the merge proposal.
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
    ) -> Result<String, SelectorError> {
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

/// Acceptance 2, as decided by Atlas ADR 0082 (coordinator decision F6 on adversary pass 1,
/// wave 2026-10-04-w8): the executor proposes and Commission rechecks the proposal and asks its
/// authority provider; Loom does not suspend for authority. Loom, as the `AgentExecutor`, is
/// invoked on each frontier the scripted governor issues for CHG-1842 until it proposes
/// `repository.merge`. It never proposes merge while merge is `Blocked`, it proposes merge on the
/// frontier where merge is `ApprovalRequired`, and Commission's admission of that proposal is
/// `NeedsAuthority` naming the merge capability.
#[test]
fn executor_proposes_merge_and_commission_asks_authority() {
    let case = CaseId("CHG-1842".to_owned());
    let governor = FakeGovernor::new();
    governor.script(case.clone(), [initial(), after_tests_on_r2()]);
    let commission = commission(&case);
    let loom = Loom::new(MergeSeeking, EmptyObjectArguments, "land the change");

    let mut outcomes = Vec::new();
    let mut merge = None;
    for _ in 0..MAX_INVOCATIONS {
        let frontier = governor
            .frontier(&case)
            .unwrap_or_else(|error| panic!("frontier for {} failed: {error:?}", case.0));
        let outcome = loom.run(&commission, &frontier);
        outcomes.push(outcome.clone());
        if proposes_merge(&outcome) {
            merge = Some(frontier);
            break;
        }
    }

    assert!(
        matches!(
            &outcomes[0],
            ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction { action, .. })
                if action == "repository.inspect"
        ),
        "on the initial frontier, where merge is Blocked, Loom should propose the first admissible \
         action: {outcomes:?}"
    );

    let frontier = merge.unwrap_or_else(|| {
        panic!("Loom never proposed {MERGE} in {MAX_INVOCATIONS} invocations: {outcomes:?}")
    });
    let listed: Vec<ActionStatus> = frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.action == MERGE)
        .map(|listed| listed.status)
        .collect();
    assert_eq!(
        listed,
        vec![ActionStatus::ApprovalRequired],
        "Loom proposed {MERGE} on a frontier where it is not ApprovalRequired: {outcomes:?}"
    );
    assert_eq!(
        admit(&frontier, MERGE),
        Admission::NeedsAuthority(AdmissionNeedsAuthority {
            capability: MERGE_CAPABILITY.to_owned(),
        }),
        "Commission does not ask for the merge capability on Loom's proposal"
    );
}

fn proposes_merge(outcome: &ExecutorOutcome) -> bool {
    matches!(
        outcome,
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction { action, .. })
            if action == MERGE
    )
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
