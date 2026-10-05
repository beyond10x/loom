#![forbid(unsafe_code)]

//! Loom: the native executor of a Commission run.
//!
//! The run model (Session, Turn, ActionCatalogue, Selection, ArgumentRequest and their ids) is
//! generated from `ess/` into `generated/rust/loom/` and re-exported here as [`model`]. It is never
//! written by hand.
//!
//! Loom implements Commission's [`AgentExecutor`] over Commission's generated `Frontier`. Its
//! selector chooses from the catalogue projected from that frontier ([`selection`]), and its
//! argument generator is handed the one catalogue entry the selection names ([`arguments`]).
//!
//! Loom keeps no admission rule of its own: whether a selected action may be proposed is
//! Commission's [`admit`]. An action that needs authority is proposed, and Commission rechecks it
//! and asks its authority provider (Atlas ADR 0082); Loom never suspends for authority.

/// The run model, synthesized from the ESS specification.
pub use loom as model;

pub mod arguments;
pub mod compaction;
pub mod harness;
pub mod projection;
pub mod recovery;
pub mod revalidation;
pub mod selection;
pub mod session;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use sha2::{Digest, Sha256};

use b10x_loom_commission::admission::admit;
use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, Admission, Commission, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, Frontier, ProposedActionArguments, SuspensionReason, Unit,
    commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;

pub use arguments::{ArgumentContext, ArgumentGenerator, EmptyObjectArguments};
pub use selection::{ActionSelector, FirstAdmissibleSelector, SelectorError};

use arguments::RequestRecord;
use model::behaviour::SelectionStorage;
use model::run::obligations::RequestArgumentsBehavior;
use model::run::{
    ActionCatalogue, AnySelection, ArgumentRequestId, ArgumentRequestSnapshot, CatalogueId,
    RequestArguments, RequestArgumentsOutcome, Selection, SelectionId, SelectionSnapshot, TurnId,
    action_catalogue_state, selection_state,
};
use selection::{SelectionContext, SelectionRefusal};

/// Loom as Commission's agent executor: a selector, an argument generator and the run's prompt,
/// with the record of every selection it has made and every argument request that serves one.
///
/// Each call of [`AgentExecutor::run`] is one run, numbered from 0 per Loom. A run's catalogue is
/// identified by its frontier's id; its selection and its argument request get ids of their own,
/// derived from the frontier's id and the run's number. Two runs on one frontier
/// therefore keep two selections and two requests apart.
pub struct Loom<S, G> {
    selector: S,
    arguments: G,
    prompt: String,
    record: Mutex<RequestRecord>,
    runs: AtomicU64,
}

impl<S, G> Loom<S, G> {
    /// A Loom that selects with `selector`, generates arguments with `arguments` and works on
    /// `prompt`, with an empty record and no runs yet.
    pub fn new(selector: S, arguments: G, prompt: impl Into<String>) -> Self {
        Self {
            selector,
            arguments,
            prompt: prompt.into(),
            record: Mutex::default(),
            runs: AtomicU64::new(0),
        }
    }

    /// Every selection this Loom has made, one per run that selected, in the order it made them.
    pub fn selections(&self) -> Vec<SelectionSnapshot> {
        self.record().selections().to_vec()
    }

    /// Every argument request this Loom has recorded, in the order it recorded them, each naming
    /// the selection of its own run.
    pub fn argument_requests(&self) -> Vec<ArgumentRequestSnapshot> {
        self.record().argument_requests().to_vec()
    }

    /// The record, whatever a panicking holder left: each write to it is one whole snapshot.
    fn record(&self) -> MutexGuard<'_, RequestRecord> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<S: ActionSelector, G> Loom<S, G> {
    /// The selector's choice from `catalogue`, as the selection `selection_id`. An action the
    /// catalogue does not list is refused and named, whatever the selector's confidence.
    pub fn select(
        &self,
        catalogue: &ActionCatalogue<action_catalogue_state::Projected>,
        selection_id: SelectionId,
    ) -> Result<Selection<selection_state::Selected>, SelectionRefusal> {
        let context = SelectionContext {
            prompt: self.prompt.clone(),
        };
        selection::select(&self.selector, &context, catalogue, selection_id)
    }
}

/// Whether Commission would refuse every action `frontier` lists: then there is nothing to propose.
fn admits_nothing(frontier: &Frontier<frontier_state::Issued>) -> bool {
    frontier
        .data()
        .actions
        .iter()
        .all(|listed| matches!(admit(frontier, &listed.action), Admission::Refused(_)))
}

/// Whether `frontier` lists an entry of `action` that decides `admission`, whatever the order of
/// its entries: an `Admissible` entry when admitted, an `ApprovalRequired` entry naming the
/// capability when it needs authority.
fn has_deciding_entry(
    frontier: &Frontier<frontier_state::Issued>,
    action: &str,
    admission: &Admission,
) -> bool {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.action == action)
        .any(|listed| match admission {
            Admission::Admissible(_) => listed.status == ActionStatus::Admissible,
            Admission::NeedsAuthority(needs) => {
                listed.status == ActionStatus::ApprovalRequired
                    && listed.capability.as_deref() == Some(needs.capability.as_str())
            }
            Admission::Refused(_) => false,
        })
}

/// The `kind` id of run number `run` on the frontier `frontier_id`: a name-based UUID (RFC 9562
/// version 8) over the SHA-256 of the three. Within one `Loom`, two runs, two frontiers or two kinds
/// get different ids, up to a SHA-256 collision; a second `Loom` (or one restarted) numbers its runs
/// from 0 again, so its ids are unique only within itself (story:interruption-recovery).
fn run_id(kind: &str, frontier_id: &str, run: u64) -> model::primitives::Uuid {
    let digest = Sha256::digest(format!("{kind}\n{frontier_id}\n{run}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    model::primitives::Uuid(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn no_useful_action() -> ExecutorOutcome {
    ExecutorOutcome::NoUsefulAction(Unit(true))
}

fn outage(error: String) -> ExecutorOutcome {
    ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
        reason: SuspensionReason::ExternalAvailability(Value::Object(vec![(
            "error".to_owned(),
            Value::Text(error),
        )])),
    })
}

impl<S, G> AgentExecutor for Loom<S, G>
where
    S: ActionSelector,
    G: ArgumentGenerator,
{
    /// The port has no error channel. A frontier for another case than the commission's, a
    /// frontier that admits nothing, a selector that finds nothing admissible, a selection the
    /// projected catalogue does not list, and a selection Commission refuses are `NoUsefulAction`. A selector that is unavailable, or a failing
    /// argument generator, is `Suspended` with `ExternalAvailability` carrying its message.
    ///
    /// Every selection made is recorded; for one Commission does not refuse, the argument request
    /// is recorded against it before the generator is handed the selected catalogue entry.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        let run = self.runs.fetch_add(1, Ordering::Relaxed);
        if frontier.data().case_id != commission.data().case_id || admits_nothing(frontier) {
            return no_useful_action();
        }
        // The selector sees only the catalogue projected from this frontier. Until a run assigns
        // turn identities, one frontier is one turn and one catalogue, identified by the
        // frontier's id; the selection and the argument request are this run's own.
        let identity = || frontier.data().frontier_id.0.0.clone();
        let catalogue = projection::project(
            frontier,
            CatalogueId(model::primitives::Uuid(identity())),
            TurnId(model::primitives::Uuid(identity())),
        );
        let selection = match self.select(
            &catalogue,
            SelectionId(run_id("selection", &identity(), run)),
        ) {
            Ok(selection) => selection,
            Err(SelectionRefusal::NotInCatalogue(_))
            | Err(SelectionRefusal::Selector(SelectorError::NothingAdmissible)) => {
                return no_useful_action();
            }
            Err(SelectionRefusal::Selector(SelectorError::Unavailable(error))) => {
                return outage(error);
            }
        };
        let selection_id = selection.data().selection_id.clone();
        let selected = selection.data().action.clone();
        self.record()
            .put(AnySelection::Selected(selection).snapshot());

        // Safety invariant: only what Commission admits, or admits once authorized, is proposed.
        // An action outside the catalogue was refused above; Commission decides the rest.
        let admission = admit(frontier, &selected);
        if matches!(admission, Admission::Refused(_))
            || !has_deciding_entry(frontier, &selected, &admission)
        {
            return no_useful_action();
        }
        // The generator is handed the one entry the selection names, never the rest of the
        // catalogue (Atlas ADR 0073, step 1); the selection was checked against the catalogue.
        let Some(entry) = catalogue
            .data()
            .entries
            .iter()
            .find(|entry| entry.action == selected)
        else {
            return no_useful_action();
        };
        let requested = self.record().request_arguments(RequestArguments {
            argument_request_id: ArgumentRequestId(run_id("argument-request", &identity(), run)),
            selection_id,
        });
        if !matches!(requested, Ok(RequestArgumentsOutcome::Requested { .. })) {
            return no_useful_action();
        }

        let context = ArgumentContext {
            prompt: self.prompt.clone(),
        };
        match self.arguments.generate(&context, entry) {
            Ok(arguments) => ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: selected,
                arguments: ProposedActionArguments(arguments),
            }),
            Err(error) => outage(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_loom_commission::model::primitives::Uuid;
    use b10x_loom_commission::model::responsibility::{
        AgentRevisionId, AuthorityContext, CaseId, CommissionData, CommissionId, FrontierAction,
        FrontierData, FrontierId, PrincipalId,
    };

    /// A selector that names one action, or fails with one error.
    struct Scripted(Result<&'static str, SelectorError>);

    impl ActionSelector for Scripted {
        fn select(
            &self,
            _context: &SelectionContext,
            _candidates: &[model::run::CatalogueEntry],
        ) -> Result<selection::Choice, SelectorError> {
            self.0.clone().map(|action| selection::Choice {
                action: action.to_owned(),
                confidence: None,
            })
        }

        fn strategy(&self) -> model::run::SelectionStrategy {
            model::run::SelectionStrategy::ReasoningModel
        }
    }

    fn commission() -> Commission<commission_state::Assigned> {
        Commission::new(CommissionData {
            commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000001".into())),
            agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000002".into())),
            case_id: CaseId("CASE-1".into()),
            principal: PrincipalId("principal-a".into()),
            authority_context: AuthorityContext(Value::Null),
        })
    }

    fn frontier_for(case: &str, actions: Vec<FrontierAction>) -> Frontier<frontier_state::Issued> {
        Frontier::new(FrontierData {
            frontier_id: FrontierId(Uuid("00000000-0000-4000-8000-000000000003".into())),
            case_id: CaseId(case.into()),
            case_revision: 1,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions,
        })
    }

    fn listed(action: &str, status: ActionStatus, capability: Option<&str>) -> FrontierAction {
        FrontierAction {
            action: action.into(),
            status,
            capability: capability.map(str::to_owned),
            reasons: Vec::new(),
        }
    }

    fn run(selector: Scripted, actions: Vec<FrontierAction>) -> ExecutorOutcome {
        Loom::new(selector, EmptyObjectArguments, "investigate")
            .run(&commission(), &frontier_for("CASE-1", actions))
    }

    /// `story:action-selector`: the selector is handed the catalogue projected from the frontier,
    /// and an action that catalogue does not list is refused before Commission is asked.
    #[test]
    fn selector_cannot_expand_frontier() {
        let outcome = run(
            Scripted(Ok("forbidden.action")),
            vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// An approval-gated selection is proposed: Commission asks for the authority (ADR 0082).
    #[test]
    fn approval_gated_selection_is_proposed_for_commission_to_authorize() {
        let outcome = run(
            Scripted(Ok("repository.merge")),
            vec![
                listed("repository.merge", ActionStatus::Admissible, None),
                listed(
                    "repository.merge",
                    ActionStatus::ApprovalRequired,
                    Some("repository.write"),
                ),
            ],
        );
        assert!(
            matches!(&outcome, ExecutorOutcome::ProposedAction(proposal) if proposal.action == "repository.merge"),
            "{outcome:?}"
        );
    }

    #[test]
    fn an_unavailable_selector_keeps_its_message() {
        let outcome = run(
            Scripted(Err(SelectorError::Unavailable(
                "model endpoint unreachable".to_owned(),
            ))),
            vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
        );
        let ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
            reason: SuspensionReason::ExternalAvailability(detail),
        }) = &outcome
        else {
            panic!("not an outage: {outcome:?}");
        };
        assert_eq!(
            detail.member("error"),
            Some(&Value::Text("model endpoint unreachable".to_owned()))
        );
    }

    #[test]
    fn nothing_admissible_from_the_selector_is_no_useful_action() {
        let outcome = run(
            Scripted(Err(SelectorError::NothingAdmissible)),
            vec![listed(
                "repository.merge",
                ActionStatus::ApprovalRequired,
                Some("repository.write"),
            )],
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// The commission is for CASE-1; a frontier issued for CASE-2 is not its frontier, and Loom
    /// proposes nothing on it, however admissible its actions.
    #[test]
    fn a_frontier_for_another_case_is_no_useful_action() {
        let outcome = Loom::new(
            Scripted(Ok("metrics.inspect")),
            EmptyObjectArguments,
            "investigate",
        )
        .run(
            &commission(),
            &frontier_for(
                "CASE-2",
                vec![listed("metrics.inspect", ActionStatus::Admissible, None)],
            ),
        );
        assert_eq!(outcome, ExecutorOutcome::NoUsefulAction(Unit(true)));
    }

    /// A run id is a version-8 UUID, the same for the same inputs, and different when the kind,
    /// the frontier or the run number differs.
    #[test]
    fn run_ids_are_uuids_of_their_kind_frontier_and_run() {
        const FRONTIER: &str = "00000000-0000-4000-8000-000000000003";
        let id = run_id("selection", FRONTIER, 0).0;
        let groups: Vec<&str> = id.split('-').collect();
        assert_eq!(
            groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12],
            "{id}"
        );
        assert!(
            id.bytes()
                .all(|byte| byte == b'-' || matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "{id}"
        );
        assert!(groups[2].starts_with('8'), "version 8: {id}");
        assert!(
            matches!(groups[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'),
            "RFC 9562 variant: {id}"
        );
        assert_eq!(run_id("selection", FRONTIER, 0).0, id);
        for other in [
            run_id("argument-request", FRONTIER, 0),
            run_id("selection", FRONTIER, 1),
            run_id("selection", "00000000-0000-4000-8000-000000000004", 0),
            model::primitives::Uuid(FRONTIER.to_owned()),
        ] {
            assert_ne!(other.0, id);
        }
    }
}
