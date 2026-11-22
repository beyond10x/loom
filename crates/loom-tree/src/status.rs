//! `tree status`: the tree as one document for a live view.

use std::path::{Path, PathBuf};

use b10x_loom_commission::model::responsibility::CaseId;
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_runlog::log_file_name;
use loom::governor::CaseLink;
use loom::runlog::RunLogEvent;
use loom::tree::{CaseStatus, TreeRefusalReason, TreeStatus};
use loom_governor::{CanonGovernor, FileCaseStore, case_file_stem};
use loom_protocols::ProtocolCatalog;

use crate::store::{existing_store, read_manifest, read_regular};
use crate::{Refused, unreadable};

/// One case of the tree, its parent and the link the two hold.
pub(crate) struct Member {
    pub(crate) case: String,
    pub(crate) parent: Option<String>,
    pub(crate) link: Option<CaseLink>,
}

/// The cases of the tree rooted at `root`, the root first and each case before its children, in
/// the order they were opened: the governor's [`CanonGovernor::tree`].
pub(crate) fn members<S: loom_governor::FallibleCaseStore>(
    governor: &CanonGovernor<S>,
    root: &str,
) -> Result<Vec<Member>, Refused> {
    let cases = governor
        .tree(&CaseId(root.to_owned()))
        .map_err(|error| unreadable(format!("the tree of case {root}"), format!("{error:?}")))?;
    Ok(cases
        .into_iter()
        .map(|held| Member {
            case: held.case.0,
            parent: held.parent.as_ref().map(|link| link.parent.0.clone()),
            link: held.parent,
        })
        .collect())
}

/// The status of the tree in the store `store`, read with `catalog`: per case its id, its parent,
/// its protocol, its case revision, the outcome it ended through or the stop its last run
/// recorded, the obligations its current frontier holds open, and the paths of its case file and
/// its run log. The store is read, never held.
///
/// # Errors
/// `NotATree` when the store holds no tree; `StoreUnreadable` when it cannot be read or the
/// governor cannot decide a case.
pub fn tree_status(store: &Path, catalog: &ProtocolCatalog) -> Result<TreeStatus, Refused> {
    let store = existing_store(store)?;
    let manifest = read_manifest(&store)?;
    let cases =
        FileCaseStore::open(store.clone()).map_err(|error| unreadable("the case store", error))?;
    let governor = CanonGovernor::new(cases)
        .with_catalog(catalog)
        .map_err(|error| {
            Refused::new(
                TreeRefusalReason::UnknownProtocol,
                format!("the tree's catalog is refused: {error}"),
            )
        })?;
    let mut statuses = Vec::new();
    for member in members(&governor, &manifest.root)? {
        let id = CaseId(member.case.clone());
        let protocol = governor
            .protocol(&id)
            .map_err(|error| unreadable(format!("case {}", member.case), format!("{error:?}")))?;
        let frontier = governor
            .frontier(&id)
            .map_err(|error| unreadable(format!("case {}", member.case), format!("{error:?}")))?;
        let frontier = frontier.data();
        let open: Vec<String> = frontier
            .obligations
            .iter()
            .filter(|obligation| obligation.open)
            .map(|obligation| obligation.obligation.clone())
            .collect();
        let log = store.join(log_file_name(&member.case));
        statuses.push(CaseStatus {
            case: member.case.clone(),
            parent: member.parent,
            protocol,
            revision: frontier.case_revision,
            outcome: governor
                .case_state(&id)
                .map_err(|error| unreadable(format!("case {}", member.case), format!("{error:?}")))?
                .termination
                .map(|termination| termination.outcome),
            stop: last_stop(&log)?,
            open_obligations: open,
            case_file: text(&store.join(format!("{}.json", case_file_stem(&member.case)))),
            run_log: text(&log),
        });
    }
    Ok(TreeStatus {
        store: text(&store),
        workspace: manifest.workspace,
        root: manifest.root,
        cases: statuses,
        protocols: Vec::new(),
    })
}

/// The stop the last `stopped` line of the log at `log` names, when it has one.
fn last_stop(log: &Path) -> Result<Option<String>, Refused> {
    let bytes = read_regular(log)?.unwrap_or_default();
    let Ok(lines) = b10x_loom_runlog::verify(&bytes) else {
        return Ok(None);
    };
    Ok(lines.iter().rev().find_map(|line| match &line.event {
        RunLogEvent::Stopped(stopped) => Some(stopped.stop.clone()),
        _ => None,
    }))
}

fn text(path: &PathBuf) -> String {
    path.display().to_string()
}
