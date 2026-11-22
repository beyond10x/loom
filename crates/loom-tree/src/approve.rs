//! `tree approve`: a principal's decision on one capability for one case of the tree, recorded at
//! the case's current revision.

use std::path::Path;

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_commission::ports::governor::Governor as _;
use loom::runlog::{AuthorityRecorded, RunLogEvent};
use loom::tree::{ApprovalDecision, CaseApproval, TreeRefusalReason};
use loom_governor::{CanonGovernor, FileCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::approvals::record_approval;
use crate::log::Logs;
use crate::status::members;
use crate::store::{existing_store, read_manifest};
use crate::{Refused, StoreLock, TreeTime, unreadable};

/// The most bytes of a principal or a capability `tree approve` records.
pub const MAX_NAME_BYTES: usize = 200;

/// What `tree approve` is given.
#[derive(Debug)]
pub struct ApproveRequest<'a> {
    /// The store directory `tree open` made.
    pub store: &'a Path,
    /// The case of the tree the decision is for.
    pub case: &'a str,
    /// The capability decided.
    pub capability: &'a str,
    /// The principal who decided.
    pub principal: &'a str,
    /// The decision.
    pub decision: ApprovalDecision,
    /// Where the command reads its instants.
    pub time: TreeTime,
    /// The tree's catalog.
    pub catalog: &'a ProtocolCatalog,
}

/// Records the decision `request` names (see the module documentation) and returns it.
///
/// # Errors
/// `StoreInUse` when another writer holds the store; `NotATree`; `CaseNotInTree` when the case is
/// not a case of the tree; `NotAPersonAction` for a principal or capability that is empty, longer
/// than [`MAX_NAME_BYTES`] or holds a control character; `LogRefused` when the case's run log does
/// not verify; `StoreUnreadable`. Nothing is written on a refusal.
pub fn approve(request: &ApproveRequest<'_>) -> Result<CaseApproval, Refused> {
    for (what, name) in [
        ("principal", request.principal),
        ("capability", request.capability),
    ] {
        if name.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(Refused::new(
                TreeRefusalReason::NotAPersonAction,
                format!(
                    "the {what} is empty, longer than {MAX_NAME_BYTES} bytes or holds a control character"
                ),
            ));
        }
    }
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
    if !members(&governor, &manifest.root)?
        .iter()
        .any(|member| member.case == request.case)
    {
        return Err(Refused::new(
            TreeRefusalReason::CaseNotInTree,
            format!("the tree rooted at {} holds no such case", manifest.root),
        )
        .case(crate::time::cut(request.case)));
    }
    let case = CaseId(request.case.to_owned());
    let revision = governor
        .current_revision(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    let logs = Logs::open(&store, request.time.clone())?;
    logs.reconcile(request.case)?;
    let approval = CaseApproval {
        case: request.case.to_owned(),
        capability: request.capability.to_owned(),
        decision: request.decision,
        principal: request.principal.to_owned(),
        case_revision: revision,
        at,
    };
    record_approval(&store, &approval)?;
    logs.append(
        request.case,
        RunLogEvent::AuthorityRecorded(AuthorityRecorded {
            capability: approval.capability.clone(),
            decision: format!("{:?}", approval.decision),
            principal: approval.principal.clone(),
        }),
    )?;
    Ok(approval)
}
