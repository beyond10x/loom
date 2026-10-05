//! Action requests and their revalidation against the current case revision.
//!
//! A proposed action becomes an [`ActionRequest`] bound to the case revision of the frontier it was
//! chosen from ([`request`]). The expected revision always comes from that frontier, never from
//! executor output: a proposal carries no revision (`AGENTS.md` § Rules).
//!
//! Immediately before anything is done with a request, [`revalidate`] checks it again. It sends
//! the request, identity included, as the input of the specification's `RevalidateActionRequest`
//! command ([`command_input`]) and answers with that command's outcome, in this order:
//!
//! 1. stale, naming both revisions, when the governor's current revision of the case is not the
//!    request's expected revision. This is decided from the revision alone, before any frontier is
//!    read, and holds even when the action would be admissible now: a selection made on stale
//!    state is not permission;
//! 2. not admitted, with a reason naming both cases, when the frontier the governor then issues is
//!    for another case, whatever revision it carries;
//! 3. stale, naming both revisions, when that frontier is for another revision of this case;
//! 4. not admitted, naming the action and the frontier's reasons, when frontier admission
//!    ([`admit`]) refuses the action on the current frontier;
//! 5. needs authority, naming the capability, when the frontier lists the action as
//!    `ApprovalRequired`: such an action is never admitted here without an authority decision;
//! 6. otherwise admitted.
//!
//! Revalidation changes nothing. A governor that cannot answer fails it with its typed error.
#![forbid(unsafe_code)]

use crate::admission::admit;
use crate::model::responsibility::{
    ActionNeedsAuthority, ActionNotAdmitted, ActionRequest, ActionRequestData, ActionRequestId,
    ActionRequestStale, Admission, ExecutorOutcomeProposedAction, Frontier, GovernorError,
    RevalidateActionRequest, RevalidateActionRequestOutcome, RunId, action_request_state,
    frontier_state,
};
use crate::ports::governor::Governor;

/// The request for `proposal`, made by run `run_id` on `frontier`: bound to the frontier's case and
/// to the revision the frontier was issued for.
pub fn request<S: frontier_state::Marker>(
    action_request_id: ActionRequestId,
    run_id: RunId,
    frontier: &Frontier<S>,
    proposal: ExecutorOutcomeProposedAction,
) -> ActionRequest<action_request_state::Requested> {
    let frontier = frontier.data();
    ActionRequest::new(ActionRequestData {
        action_request_id,
        run_id,
        case_id: frontier.case_id.clone(),
        expected_case_revision: frontier.case_revision,
        action: proposal.action,
        arguments: proposal.arguments,
    })
}

/// The input of the `RevalidateActionRequest` command for `request`: the whole request, its
/// identity included, so an outcome answers for exactly this request.
pub fn command_input(
    request: &ActionRequest<action_request_state::Requested>,
) -> RevalidateActionRequest {
    let request = request.data();
    RevalidateActionRequest {
        action_request_id: request.action_request_id.clone(),
        run_id: request.run_id.clone(),
        case_id: request.case_id.clone(),
        expected_case_revision: request.expected_case_revision,
        action: request.action.clone(),
        arguments: request.arguments.clone(),
    }
}

/// Revalidates `request` against the case's current revision and frontier, read from `governor`
/// now, in the order the module documents.
pub fn revalidate<G: Governor + ?Sized>(
    governor: &G,
    request: &ActionRequest<action_request_state::Requested>,
) -> Result<RevalidateActionRequestOutcome, GovernorError> {
    let input = command_input(request);
    let expected = input.expected_case_revision;

    let current = governor.current_revision(&input.case_id)?;
    if current != expected {
        return Ok(stale(expected, current));
    }

    let frontier = governor.frontier(&input.case_id)?;
    let issued = frontier.data();
    if issued.case_id != input.case_id {
        return Ok(RevalidateActionRequestOutcome::NotAdmitted {
            error: ActionNotAdmitted {
                action: input.action,
                reasons: vec![format!(
                    "the governor issued a frontier for case `{}`, not `{}`, \
                     to action request `{}`",
                    issued.case_id.0, input.case_id.0, input.action_request_id.0.0
                )],
            },
        });
    }
    if issued.case_revision != expected {
        return Ok(stale(expected, issued.case_revision));
    }

    Ok(match admit(&frontier, &input.action) {
        Admission::Admissible(_) => RevalidateActionRequestOutcome::Admitted,
        Admission::NeedsAuthority(needs) => RevalidateActionRequestOutcome::NeedsAuthority {
            error: ActionNeedsAuthority {
                action: input.action,
                capability: needs.capability,
            },
        },
        Admission::Refused(refused) => RevalidateActionRequestOutcome::NotAdmitted {
            error: ActionNotAdmitted {
                action: refused.action,
                reasons: refused.reasons,
            },
        },
    })
}

fn stale(expected: i64, current: i64) -> RevalidateActionRequestOutcome {
    RevalidateActionRequestOutcome::Stale {
        error: ActionRequestStale {
            expected_case_revision: expected,
            current_case_revision: current,
        },
    }
}
