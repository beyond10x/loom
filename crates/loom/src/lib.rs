#![forbid(unsafe_code)]

//! Loom: the native executor of a Commission run.
//!
//! The run model (Session, Turn, ActionCatalogue, Selection, ArgumentRequest and their ids) is
//! generated from `ess/` into `generated/rust/loom/` and re-exported here as [`model`]. It is never
//! written by hand.
//!
//! Loom implements Commission's [`AgentExecutor`] over Commission's generated `Frontier`. The
//! selector and argument generator below are a minimal seam until `story:action-selector` and
//! `story:argument-generator` replace them.
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

use b10x_commission::admission::admit;
use b10x_commission::model::json::Value;
use b10x_commission::model::responsibility::{
    ActionStatus, Admission, Commission, ExecutorOutcome, ExecutorOutcomeProposedAction,
    ExecutorOutcomeSuspended, Frontier, FrontierAction, ProposedActionArguments, SuspensionReason,
    Unit, commission_state, frontier_state,
};
use b10x_commission::ports::executor::AgentExecutor;

/// Why a selector picked no action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    /// The frontier offers nothing to select. Loom answers it with `NoUsefulAction`.
    NothingAdmissible,
    /// The selector could not answer, for the reason given. Loom answers it with
    /// `Suspended(ExternalAvailability)` carrying the reason.
    Unavailable(String),
}

/// Picks the action Loom proposes next.
pub trait ActionSelector {
    /// The `action` of one of `frontier`'s actions.
    fn select(
        &self,
        frontier: &Frontier<frontier_state::Issued>,
        prompt: &str,
    ) -> Result<String, SelectorError>;
}

/// Generates the arguments of one selected action.
pub trait ArgumentGenerator {
    /// The arguments for `action`: the frontier entry that decided the action's admission.
    fn generate(
        &self,
        action: &FrontierAction,
        prompt: &str,
    ) -> Result<ProposedActionArguments, String>;
}

/// Deterministic bootstrap selector used only for tests/examples: the first `Admissible` action.
#[derive(Debug, Default)]
pub struct FirstAdmissibleSelector;

impl ActionSelector for FirstAdmissibleSelector {
    fn select(
        &self,
        frontier: &Frontier<frontier_state::Issued>,
        _prompt: &str,
    ) -> Result<String, SelectorError> {
        frontier
            .data()
            .actions
            .iter()
            .find(|listed| listed.status == ActionStatus::Admissible)
            .map(|listed| listed.action.clone())
            .ok_or(SelectorError::NothingAdmissible)
    }
}

/// Bootstrap argument generator: always the empty object.
#[derive(Debug, Default)]
pub struct EmptyObjectArguments;

impl ArgumentGenerator for EmptyObjectArguments {
    fn generate(
        &self,
        _action: &FrontierAction,
        _prompt: &str,
    ) -> Result<ProposedActionArguments, String> {
        Ok(ProposedActionArguments(Value::Object(Vec::new())))
    }
}

pub struct Loom<S, G> {
    selector: S,
    arguments: G,
    prompt: String,
}

impl<S, G> Loom<S, G> {
    pub fn new(selector: S, arguments: G, prompt: impl Into<String>) -> Self {
        Self {
            selector,
            arguments,
            prompt: prompt.into(),
        }
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

/// The entry of `action` that decided `admission`, whatever the order of the frontier's entries:
/// an `Admissible` entry when admitted, an `ApprovalRequired` entry naming the capability when it
/// needs authority. Among several such entries, the least by capability and then reasons.
fn deciding_entry<'a>(
    frontier: &'a Frontier<frontier_state::Issued>,
    action: &str,
    admission: &Admission,
) -> Option<&'a FrontierAction> {
    frontier
        .data()
        .actions
        .iter()
        .filter(|listed| listed.action == action)
        .filter(|listed| match admission {
            Admission::Admissible(_) => listed.status == ActionStatus::Admissible,
            Admission::NeedsAuthority(needs) => {
                listed.status == ActionStatus::ApprovalRequired
                    && listed.capability.as_deref() == Some(needs.capability.as_str())
            }
            Admission::Refused(_) => false,
        })
        .min_by(|a, b| (&a.capability, &a.reasons).cmp(&(&b.capability, &b.reasons)))
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
    /// frontier that admits nothing, a selector that finds nothing admissible, and a selection
    /// Commission refuses are `NoUsefulAction`. A selector that is unavailable, or a failing
    /// argument generator, is `Suspended` with `ExternalAvailability` carrying its message.
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        if frontier.data().case_id != commission.data().case_id || admits_nothing(frontier) {
            return no_useful_action();
        }
        let selected = match self.selector.select(frontier, &self.prompt) {
            Ok(selected) => selected,
            Err(SelectorError::NothingAdmissible) => return no_useful_action(),
            Err(SelectorError::Unavailable(error)) => return outage(error),
        };

        // Safety invariant: only what Commission admits, or admits once authorized, is proposed.
        // An action outside the frontier is refused there too.
        let admission = admit(frontier, &selected);
        if matches!(admission, Admission::Refused(_)) {
            return no_useful_action();
        }
        let Some(entry) = deciding_entry(frontier, &selected, &admission) else {
            return no_useful_action();
        };

        match self.arguments.generate(entry, &self.prompt) {
            Ok(arguments) => ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: selected,
                arguments,
            }),
            Err(error) => outage(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_commission::model::primitives::Uuid;
    use b10x_commission::model::responsibility::{
        AgentRevisionId, AuthorityContext, CaseId, CommissionData, CommissionId, FrontierData,
        FrontierId, PrincipalId,
    };

    /// A selector that names one action, or fails with one error.
    struct Scripted(Result<&'static str, SelectorError>);

    impl ActionSelector for Scripted {
        fn select(
            &self,
            _frontier: &Frontier<frontier_state::Issued>,
            _prompt: &str,
        ) -> Result<String, SelectorError> {
            self.0.clone().map(str::to_owned)
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
}
