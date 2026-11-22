//! `tree close`: terminates one case of the tree through an outcome a person names, when more than
//! one outcome is legitimate for it and `tree run` therefore waits.

use std::path::Path;

use b10x_loom_commission::model::responsibility::CaseId;
use loom::primitives::Timestamp;
use loom::tree::TreeRefusalReason;
use loom_governor::{CanonGovernor, FileCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::log::Logs;
use crate::run::{closed_lines, legitimate_outcomes, termination};
use crate::status::members;
use crate::store::{existing_store, read_manifest};
use crate::{Refused, StoreLock, TreeTime, unreadable};

/// What `tree close` is given.
#[derive(Debug)]
pub struct CloseRequest<'a> {
    /// The store directory `tree open` made.
    pub store: &'a Path,
    /// The case of the tree to close.
    pub case: &'a str,
    /// The outcome the case ends through: one Canon reports legitimate for it now.
    pub outcome: &'a str,
    /// Where the command reads its instant.
    pub time: TreeTime,
    /// The tree's catalog.
    pub catalog: &'a ProtocolCatalog,
}

/// A case `tree close` terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseClosed {
    /// The case.
    pub case: String,
    /// The outcome it ended through.
    pub outcome: String,
    /// The instant the termination records.
    pub at: Timestamp,
}

/// Terminates the case `request` names through its outcome, writes the case's `terminated` line
/// and, for a child, the parent's `child closed` line, and returns what it closed.
///
/// # Errors
/// `StoreInUse` when another writer holds the store; `NotATree`; `CaseNotInTree`; `CaseTerminated`
/// when the case is terminated already; `OutcomeNotLegitimate` when the outcome is not one Canon
/// reports legitimate for the case now, naming those it does; `LogRefused` when a run log to be
/// appended to does not verify; `StoreUnreadable`. Nothing is written on a refusal.
pub fn close(request: &CloseRequest<'_>) -> Result<CaseClosed, Refused> {
    let store = existing_store(request.store)?;
    read_manifest(&store)?;
    let _held = StoreLock::acquire(&store)?;
    let manifest = read_manifest(&store)?;
    let at = request.time.now()?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(request.catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let named = crate::time::cut(request.case).to_owned();
    let Some(member) = members(&governor, &manifest.root)?
        .into_iter()
        .find(|member| member.case == request.case)
    else {
        return Err(Refused::new(
            TreeRefusalReason::CaseNotInTree,
            format!("the tree rooted at {} holds no such case", manifest.root),
        )
        .case(named));
    };
    if let Some(outcome) = termination(&governor, request.case)? {
        return Err(Refused::new(
            TreeRefusalReason::CaseTerminated,
            format!("the case is terminated through {outcome} already"),
        )
        .case(named));
    }
    let case = CaseId(request.case.to_owned());
    let legitimate = legitimate_outcomes(&governor, &case)?;
    if !legitimate.iter().any(|outcome| outcome == request.outcome) {
        return Err(Refused::new(
            TreeRefusalReason::OutcomeNotLegitimate,
            format!(
                "the outcome {} is not legitimate for the case; legitimate now: [{}]",
                crate::time::cut(request.outcome),
                legitimate.join(", ")
            ),
        )
        .case(named));
    }
    let logs = Logs::open(&store, request.time.clone())?;
    logs.reconcile(request.case)?;
    if let Some(parent) = &member.parent {
        logs.reconcile(parent)?;
    }
    governor
        .terminate(&case, request.outcome, &at.0)
        .map_err(|error| {
            Refused::new(TreeRefusalReason::StoreUnreadable, error.to_string()).case(named.clone())
        })?;
    closed_lines(
        &logs,
        request.case,
        member.parent.as_deref(),
        request.outcome,
        &mut |_, _| {},
    )?;
    Ok(CaseClosed {
        case: request.case.to_owned(),
        outcome: request.outcome.to_owned(),
        at,
    })
}
