//! Adversary pass 2, wave 2026-10-21-w5, `story:delete-harness-port`: the selection contract as
//! fix `517b423` rewrote it.
//!
//! `docs/contracts/loom-action-selection.md` § Selection telemetry now says: "A selection refused
//! at the execution boundary, after it was made, by Commission's admission or by revalidation
//! against the current frontier, adds no record: the refused selection keeps the record written
//! when it was made, and its state says it was refused (`loom.run.Selections`)."
//!
//! Loom moves a selection to `Refused` only on its own revalidation (`Loom::revalidate`,
//! `loom.run.RevalidateSelection`). The refusal at the execution boundary that is still reachable
//! now the governed loop is gone is Commission's: the runtime revalidates every proposal against
//! the case's current revision and frontier before it hands it to the effect port
//! (`crates/loom-commission/src/action_request.rs`, `revalidate`), and a case somebody else moved
//! after Loom's revalidation is refused `Stale` there and never performed. The case below builds
//! exactly that window and reads the selection's state as the contract promises it.
//!
//! The case reads the contract from the tree it runs in, so a contract that no longer claims a
//! refused state for a refusal by Commission takes it back to green, as does a Loom that records
//! the refusal. No model, network or credential is used.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::{Timestamp, Uuid};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission,
    CommissionData, CommissionId, CompletionDetermination, EffectOutcome, EffectOutcomePerformed,
    ExecutorOutcome, Frontier, FrontierAction, FrontierData, FrontierId, GovernorError,
    Observation, ObservationId, PrincipalId, RevalidateActionRequestOutcome, RunId, Unit,
    commission_state, frontier_state, observation_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::evidence::ObservationPort;
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_commission::ports::governor::Governor;
use b10x_loom_commission::runtime::{LoopContext, run_until_blocked};
use b10x_loom_commission_testkit::fake_authority::StaticAuthorityProvider;
use b10x_loom_executor::model::run::{RevalidateSelectionOutcome, SelectionState};
use b10x_loom_executor::{EmptyObjectArguments, FirstAdmissibleSelector, Loom};

const CASE: &str = "CASE-ADV2-U2";
const SEARCH: &str = "logs.search";
const REVISION: i64 = 7;

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn commission() -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(uuid(1)),
        agent_revision_id: AgentRevisionId(uuid(2)),
        case_id: CaseId(CASE.to_owned()),
        principal: PrincipalId("principal-a".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    })
}

/// The case at revision 7 until somebody else moves it to 8. It stays open and lists one
/// admissible action needing no authority at either revision.
struct Case {
    moved: AtomicBool,
}

impl Case {
    fn revision(&self) -> i64 {
        if self.moved.load(Ordering::SeqCst) {
            REVISION + 1
        } else {
            REVISION
        }
    }
}

impl Governor for Case {
    fn current_revision(&self, _case: &CaseId) -> Result<i64, GovernorError> {
        Ok(self.revision())
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        let revision = self.revision();
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(uuid(0xf00 + u64::try_from(revision).unwrap_or(0))),
            case_id: case.clone(),
            case_revision: revision,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![FrontierAction {
                action: SEARCH.to_owned(),
                status: ActionStatus::Admissible,
                capability: None,
                reasons: Vec::new(),
            }],
        }))
    }

    fn completion(&self, _case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}

impl ObservationPort for Case {
    fn observe(
        &self,
        _observation: Observation<observation_state::Reported>,
    ) -> Result<(), GovernorError> {
        Ok(())
    }
}

/// Runs `loom`, then somebody else moves the case: after Loom revalidated and proposed, before
/// the runtime rechecks the proposal at the execution boundary.
struct MovedAfterLoom<'a, E> {
    loom: &'a E,
    case: &'a Case,
}

impl<E: AgentExecutor> AgentExecutor for MovedAfterLoom<'_, E> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let outcome = self.loom.run(commission, frontier);
        self.case.moved.store(true, Ordering::SeqCst);
        outcome
    }
}

/// Performs every action and counts what it was handed.
#[derive(Default)]
struct Effects {
    invoked: AtomicU64,
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
            report: Value::Null,
            attempt: None,
            audit: None,
            usage: None,
        }))
    }
}

#[derive(Default)]
struct Context {
    ids: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.ids += 1;
        ActionRequestId(uuid(0x300 + self.ids))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.ids += 1;
        ObservationId(uuid(0x400 + self.ids))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp("2026-10-21T12:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        None
    }
}

/// The paragraph of the selection contract on refusals at the execution boundary, read from the
/// tree this test runs in.
fn boundary_paragraph() -> String {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is unset: run this test through cargo test");
    let path = PathBuf::from(manifest).join("../../docs/contracts/loom-action-selection.md");
    let contract = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    contract
        .split("\n\n")
        .find(|paragraph| paragraph.contains("refused at the execution boundary"))
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .expect("precondition: the selection contract has a paragraph on refusals at the execution boundary")
}

#[test]
fn adversary2_w20261021w5_u2_a_selection_commission_refuses_at_the_boundary_reads_as_the_contract_says()
 {
    let case = Case {
        moved: AtomicBool::new(false),
    };
    let loom = Loom::new(
        FirstAdmissibleSelector,
        EmptyObjectArguments,
        "search the logs",
    )
    .with_governor(&case);
    let executor = MovedAfterLoom {
        loom: &loom,
        case: &case,
    };
    let effects = Effects::default();
    let ids = AtomicU64::new(0x500);
    let mut runs = Generated::new(RunStore::new(move || {
        RunId(uuid(ids.fetch_add(1, Ordering::SeqCst)))
    }));
    let end = run_until_blocked(
        &case,
        &executor,
        &StaticAuthorityProvider::new(),
        &effects,
        &commission(),
        &mut runs,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    // Preconditions: Loom admitted its selection on its own revalidation and proposed it; the
    // runtime refused the proposal at the execution boundary, stale, and never performed it.
    let revalidations = loom.revalidations();
    assert!(
        matches!(
            revalidations.as_slice(),
            [RevalidateSelectionOutcome::Admitted { .. }]
        ),
        "precondition: Loom's own revalidation admitted the one selection: {revalidations:?}"
    );
    assert!(
        matches!(
            end.requests.as_slice(),
            [request] if matches!(request.outcome, RevalidateActionRequestOutcome::Stale { .. })
        ),
        "precondition: Commission refused the proposal at the execution boundary as stale: {:?}",
        end.requests
    );
    assert!(
        end.admitted.is_empty() && effects.invoked.load(Ordering::SeqCst) == 0,
        "precondition: the refused request was never handed to the effect port: {:?}",
        end.admitted
    );

    // The contract: the refusal adds no record, and the selection's state says it was refused.
    let selections = loom.selections();
    let records = loom.selection_records();
    assert_eq!(selections.len(), 1, "one selection: {selections:?}");
    assert_eq!(
        records.len(),
        1,
        "the refusal adds no record; the one record is the selection's: {records:?}"
    );

    let paragraph = boundary_paragraph();
    let promises_refused_state =
        paragraph.contains("Commission") && paragraph.contains("says it was refused");
    if promises_refused_state {
        assert_eq!(
            selections[0].state,
            SelectionState::Refused,
            "docs/contracts/loom-action-selection.md promises that a selection refused at the \
             execution boundary by Commission keeps its record and that its state says it was \
             refused (`loom.run.Selections`), but the selection Commission's revalidation refused \
             stale reads `{:?}` in Loom::selections. The paragraph: {paragraph}",
            selections[0].state
        );
    }
}
