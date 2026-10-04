//! The governor a revalidation is answered against.
//!
//! `RevalidateActionRequest`'s refusals are `external:` in the specification: the governor's
//! current revision and frontier decide them, not the input. A scenario forces one by
//! `configure_external_outcome`, and the target answers by putting the governor in the state that
//! external condition names. `b10x_commission::action_request::revalidate` then decides the outcome
//! from that governor; nothing here picks it.

use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    ActionStatus, CaseId, CompletionDetermination, Frontier, FrontierAction, FrontierData,
    FrontierId, GovernorError, Unit, frontier_state,
};
use b10x_commission::ports::governor::Governor;

/// The capability the governor names for an action that needs approval.
pub const CAPABILITY: &str = "commission-conformance.approval";

/// The reason the governor gives for an action it blocks.
pub const BLOCKED: &str = "the conformance governor blocks this action";

/// The external condition a scenario forced, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// The governor agrees with the request: its revision is current and the action admissible.
    Agrees,
    /// `stale`: the governor's current revision of the case is not the request's expected one.
    MovedOn,
    /// `not-admitted`: the current frontier refuses the action.
    Refuses,
    /// `needs-authority`: the current frontier lists the action as `ApprovalRequired`, naming one
    /// capability.
    NeedsApproval,
}

impl Condition {
    /// The condition the `external:` outcome `outcome` names.
    pub fn forcing(outcome: &str) -> Option<Self> {
        match outcome {
            "stale" => Some(Self::MovedOn),
            "not-admitted" => Some(Self::Refuses),
            "needs-authority" => Some(Self::NeedsApproval),
            _ => None,
        }
    }
}

/// A governor holding one case at one revision, with one frontier.
#[derive(Debug, Clone)]
pub struct ScenarioGovernor {
    case: CaseId,
    revision: i64,
    action: String,
    condition: Condition,
}

impl ScenarioGovernor {
    /// The governor of `case`, which a request expects at `expected`, about `action`, under
    /// `condition`.
    pub fn new(case: CaseId, expected: i64, action: String, condition: Condition) -> Self {
        let revision = match condition {
            // Any revision but the expected one; at `i64::MAX` the case cannot have moved on, so
            // it is one behind instead.
            Condition::MovedOn => expected.checked_add(1).unwrap_or_else(|| expected - 1),
            _ => expected,
        };
        Self {
            case,
            revision,
            action,
            condition,
        }
    }

    fn known(&self, case: &CaseId) -> Result<(), GovernorError> {
        if *case == self.case {
            Ok(())
        } else {
            Err(GovernorError::UnknownCase)
        }
    }
}

impl Governor for ScenarioGovernor {
    fn current_revision(&self, case: &CaseId) -> Result<i64, GovernorError> {
        self.known(case)?;
        Ok(self.revision)
    }

    fn frontier(&self, case: &CaseId) -> Result<Frontier<frontier_state::Issued>, GovernorError> {
        self.known(case)?;
        let (status, capability, reasons) = match self.condition {
            Condition::Agrees | Condition::MovedOn => (ActionStatus::Admissible, None, Vec::new()),
            Condition::Refuses => (ActionStatus::Blocked, None, vec![BLOCKED.to_owned()]),
            Condition::NeedsApproval => (
                ActionStatus::ApprovalRequired,
                Some(CAPABILITY.to_owned()),
                Vec::new(),
            ),
        };
        Ok(Frontier::new(FrontierData {
            frontier_id: FrontierId(Uuid("00000000-0000-4000-a000-000000000001".to_owned())),
            case_id: self.case.clone(),
            case_revision: self.revision,
            claims: Vec::new(),
            obligations: Vec::new(),
            actions: vec![FrontierAction {
                action: self.action.clone(),
                status,
                capability,
                reasons,
            }],
        }))
    }

    fn completion(&self, case: &CaseId) -> Result<CompletionDetermination, GovernorError> {
        self.known(case)?;
        Ok(CompletionDetermination::Open(Unit(true)))
    }
}
