//! `tree approve`: a principal's decision on one capability for one case of the tree, recorded at
//! the case's current revision.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::primitives::Timestamp as CommissionTimestamp;
use b10x_loom_commission::model::responsibility::{ActionStatus, CaseId};
use b10x_loom_commission::ports::governor::Governor as _;
use loom::bindings::{BindingsDocument, Executor, ResultSource};
use loom::runlog::{AuthorityRecorded, RunLogEvent};
use loom::tree::{ApprovalDecision, CaseApproval, TreeManifest, TreeRefusalReason};
use loom_governor::{CanonGovernor, FallibleCaseStore, FileCaseStore};
use loom_protocols::ProtocolCatalog;

use crate::approvals::record_approval;
use crate::log::Logs;
use crate::person::{PERFORMED, PersonEffects};
use crate::run::{case_bindings, one_step};
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
    /// The capability decided; `None` only with a person action whose protocol action needs no
    /// capability, which then records no authority.
    pub capability: Option<&'a str>,
    /// The principal who decided.
    pub principal: &'a str,
    /// The decision.
    pub decision: ApprovalDecision,
    /// The `person` action the principal then takes, with `--action`.
    pub action: Option<&'a str>,
    /// The result the person action ends with, with `--result`: a value every `from: effect` entry
    /// of the action's binding lists in its `results`; `performed` when `None`.
    pub result: Option<&'a str>,
    /// The person's reasons for a named result, kept as a blob of the store.
    pub reasons: Option<Reasons<'a>>,
    /// Where the command reads its instants.
    pub time: TreeTime,
    /// The tree's catalog.
    pub catalog: &'a ProtocolCatalog,
}

/// Where a person's reasons come from.
#[derive(Debug, Clone, Copy)]
pub enum Reasons<'a> {
    /// The text itself (`--reasons`).
    Text(&'a str),
    /// An absolute path of a regular file inside the tree's workspace (`--reasons-file`), opened
    /// without following a symbolic link ([`b10x_loom_bindings::read_reasons_file`]).
    File(&'a Path),
}

/// Records the decision `request` names (see the module documentation) and returns it; `None`
/// for a person action taken with no capability, which records no authority.
///
/// # Errors
/// `StoreInUse` when another writer holds the store; `NotATree`; `CaseNotInTree` when the case is
/// not a case of the tree; `NotAPersonAction` for a principal or capability that is empty, longer
/// than [`MAX_NAME_BYTES`] or holds a control character, for no capability without a person
/// action to allow, and for a person action whose protocol action needs a capability none was
/// given for; for a result without a person action to allow or that an entry of the action does
/// not list, reasons without a result, and reasons that are empty, over 65536 bytes or 60 lines,
/// or a file not read as [`b10x_loom_bindings::read_reasons_file`] reads it; `LogRefused` when
/// the case's run log does not verify; `StoreUnreadable`. Nothing is written on a refusal.
pub fn approve(request: &ApproveRequest<'_>) -> Result<Option<CaseApproval>, Refused> {
    let allows_action = request.action.is_some() && request.decision == ApprovalDecision::Allow;
    if request.capability.is_none() && !allows_action {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "a decision names a capability; only a person action that needs none is taken without one",
        ));
    }
    if request.result.is_some() && !allows_action {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "a result is named only for a person action taken",
        ));
    }
    if request.reasons.is_some() && request.result.is_none() {
        return Err(Refused::new(
            TreeRefusalReason::NotAPersonAction,
            "reasons are given only with a named result",
        ));
    }
    for (what, name) in [
        ("principal", Some(request.principal)),
        ("capability", request.capability),
        ("result", request.result),
    ] {
        let Some(name) = name else { continue };
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
    if let Some(action) = request.action {
        person_action(&governor, &manifest, request, action)?;
    }
    // The reasons are read and bounded before anything is written.
    let reasons = match request.reasons {
        None => None,
        Some(Reasons::Text(text)) => {
            b10x_loom_bindings::reasons_fit(text)
                .map_err(|detail| reasons_refused(request, detail))?;
            Some(text.to_owned())
        }
        Some(Reasons::File(path)) => Some(
            b10x_loom_bindings::read_reasons_file(Path::new(&manifest.workspace), path)
                .map_err(|detail| reasons_refused(request, detail))?,
        ),
    };
    let approval = match request.capability {
        Some(capability) => {
            let approval = CaseApproval {
                case: request.case.to_owned(),
                capability: capability.to_owned(),
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
            Some(approval)
        }
        None => None,
    };
    if let Some(action) = request.action
        && request.decision == ApprovalDecision::Allow
    {
        let reasons_blob = reasons
            .map(|text| {
                b10x_loom_bindings::keep_reasons(&store, &text).map_err(|error| {
                    unreadable(
                        format!("case {}", request.case),
                        format!("the reasons: {error}"),
                    )
                })
            })
            .transpose()?;
        take_person_action(
            &governor,
            &store,
            &manifest,
            request,
            action,
            &logs,
            reasons_blob,
        )?;
    }
    Ok(approval)
}

/// The refusal of `request`'s reasons, saying `detail`.
fn reasons_refused(request: &ApproveRequest<'_>, detail: String) -> Refused {
    let refused = Refused::new(TreeRefusalReason::NotAPersonAction, detail).case(request.case);
    match request.action {
        Some(action) => refused.action(crate::time::cut(action).to_owned()),
        None => refused,
    }
}

/// The bindings document of `request`'s case, and why `action` cannot be taken as its principal
/// now, if it cannot: it must be bound to `person` in the case, its principal the one named,
/// admissible, and given a capability when its protocol action needs one.
fn person_action<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    manifest: &TreeManifest,
    request: &ApproveRequest<'_>,
    action: &str,
) -> Result<BindingsDocument, Refused> {
    let case = CaseId(request.case.to_owned());
    let bindings = case_bindings(governor, manifest, request.catalog, request.case)?;
    let refused = |detail: String| {
        Refused::new(TreeRefusalReason::NotAPersonAction, detail)
            .case(request.case.to_owned())
            .action(crate::time::cut(action).to_owned())
    };
    let Some(bound) = bindings
        .actions
        .iter()
        .find(|bound| bound.action == action && bound.executor.kind == Executor::Person)
    else {
        return Err(refused("the case binds no such action to a person".into()));
    };
    if bound.executor.principal.as_deref() != Some(request.principal) {
        return Err(refused(format!(
            "the action is bound to the person {}, not to the principal named",
            bound.executor.principal.as_deref().unwrap_or("nobody")
        )));
    }
    let frontier = governor
        .frontier(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    let Some(listed) = frontier
        .data()
        .actions
        .iter()
        .find(|listed| listed.action == action && listed.status != ActionStatus::Blocked)
    else {
        return Err(refused(
            "the action is not admissible in the case now".into(),
        ));
    };
    // Only an action that needs no capability is taken without one: authority is never skipped.
    if let (Some(needed), None) = (&listed.capability, request.capability) {
        return Err(refused(format!(
            "the action needs the capability {needed}; name it with --capability"
        )));
    }
    // A named result is one every `from: effect` entry lists: checked over all of them before the
    // first record, so there is never a half write.
    if let Some(result) = request.result
        && let Some(entry) = bound.evidence.iter().find(|entry| {
            entry.from == ResultSource::Effect
                && !entry.results.iter().any(|rule| rule.value == result)
        })
    {
        let listed: Vec<&str> = entry
            .results
            .iter()
            .map(|rule| rule.value.as_str())
            .collect();
        return Err(refused(format!(
            "the {} entry lists no result {result}; it lists {}",
            entry.kind,
            listed.join(", ")
        )));
    }
    Ok(bindings)
}

/// Takes the `person` action `action` of `request`'s case as its principal, through Commission's
/// loop: the approval just recorded is the authority it is revalidated against.
fn take_person_action<S: FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    store: &Path,
    manifest: &TreeManifest,
    request: &ApproveRequest<'_>,
    action: &str,
    logs: &Logs,
    reasons_blob: Option<String>,
) -> Result<(), Refused> {
    let bindings = person_action(governor, manifest, request, action)?;
    let case = CaseId(request.case.to_owned());
    let at = request.time.now()?;
    let revision = governor
        .current_revision(&case)
        .map_err(|error| unreadable(format!("case {}", request.case), format!("{error:?}")))?;
    // The log's length too: a person action taken with no approval is taken again at one revision
    // with records of their own.
    let seed = format!(
        "{}-{revision}-{}",
        crate::approvals::read_approvals(store, request.case)?.len(),
        crate::run::read_lines(store, request.case)?.len()
    );
    let effects = PersonEffects {
        governor,
        case: case.clone(),
        bindings: &bindings,
        workspace: PathBuf::from(&manifest.workspace),
        principal: request.principal.to_owned(),
        at: CommissionTimestamp(at.0.clone()),
        seed: seed.clone(),
        result: request.result.unwrap_or(PERFORMED).to_owned(),
        reasons_blob,
    };
    let ended = one_step(governor, store, &case, action, &effects, &request.time)?;
    logs.append(request.case, RunLogEvent::ActionFinished(ended))?;
    Ok(())
}
