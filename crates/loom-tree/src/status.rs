//! `tree status`: the tree as one document for a live view, of format [`STATUS_FORMAT`].

use std::collections::BTreeSet;
use std::path::Path;

use b10x_loom_bindings::{COPY_SOURCE, PROGRAM_SOURCE};
use b10x_loom_commission::model::responsibility::{CaseId, ObservationData};
use b10x_loom_commission::ports::governor::Governor as _;
use b10x_loom_runlog::log_file_name;
use loom::bindings::Executor;
use loom::governor::CaseLink;
use loom::runlog::{RunLogEvent, RunLogLine};
use loom::tree::{
    ApprovalDecision, CaseStatus, StatusApproval, StatusCopy, StatusEvidence, StatusRun,
    TreeRefusalReason, TreeStatus,
};
use loom_governor::{CanonGovernor, FileCaseStore, case_file_stem};
use loom_protocols::ProtocolCatalog;
use serde_json::Value;

use crate::run::{Held, plain_commission, tree_cases};
use crate::store::{existing_store, read_manifest, read_regular};
use crate::{Refused, unreadable};

/// The format a printed tree status names, which [`crate::read_status`] reads.
pub const STATUS_FORMAT: &str = "loom.tree-status/1";

/// The file of a tree opened with `--protocols` that names the protocol blobs it keeps.
pub const PROTOCOLS_FILE: &str = "tree-protocols.json";

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

/// The catalog a tree opened with `--protocols` keeps in its store; `None` for a tree opened
/// without it, whose commands use the host's catalog.
///
/// # Errors
/// `ProtocolRefused` when the store names kept protocols: this build of `tree open` keeps none,
/// so such a store was written by another one.
pub fn stored_catalog(store: &Path) -> Result<Option<ProtocolCatalog>, Refused> {
    let store = existing_store(store)?;
    match read_regular(&store.join(PROTOCOLS_FILE))? {
        None => Ok(None),
        Some(_) => Err(Refused::new(
            TreeRefusalReason::ProtocolRefused,
            format!(
                "the store names kept protocols in {PROTOCOLS_FILE}, which this build of the tree commands does not read"
            ),
        )),
    }
}

/// The status of the tree in the store `store`, read with `catalog`. The store is read, never
/// held, so it answers while a `tree run` holds the store.
///
/// The document has `format` [`STATUS_FORMAT`] and `version` the version of this crate; per case
/// its id, its link name, its parent, its protocol (with its id and revision), its case revision,
/// the outcome it ended through, the stop its last run recorded with the decisions it waits on,
/// its open obligations, the paths of its case file and its run log, its records in the order
/// received, the runs of its bound programs, its approvals in the order recorded and the records
/// copied into it.
///
/// # Errors
/// `NotATree` when the store holds no tree; `StoreUnreadable` when it cannot be read, the governor
/// cannot decide a case, or a record is not a `canon-evidence/1` record.
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
    let held = tree_cases(&governor, &manifest.root, &manifest.bindings, catalog)?;
    let observations = governor
        .try_observations()
        .map_err(|error| unreadable("the store's observations", format!("{error:?}")))?;
    let mut statuses = Vec::new();
    for member in members(&governor, &manifest.root)? {
        let case = member.case.as_str();
        let id = CaseId(member.case.clone());
        let decide = |error: String| unreadable(format!("case {case}"), error);
        let protocol = governor
            .protocol(&id)
            .map_err(|error| decide(format!("{error:?}")))?;
        let frontier = governor
            .frontier(&id)
            .map_err(|error| decide(format!("{error:?}")))?;
        let frontier = frontier.data();
        let open: Vec<String> = frontier
            .obligations
            .iter()
            .filter(|obligation| obligation.open)
            .map(|obligation| obligation.obligation.clone())
            .collect();
        let state = governor
            .case_state(&id)
            .map_err(|error| decide(format!("{error:?}")))?;
        let log = store.join(log_file_name(case));
        let lines = log_lines(&log)?;
        let (stop, stop_next) = last_stop(&lines);
        let (protocol_id, protocol_revision) = match protocol.rsplit_once('@') {
            Some((id, revision)) => (id.to_owned(), revision.to_owned()),
            None => (protocol.clone(), String::new()),
        };
        let bindings = held.iter().find(|held| held.case == member.case);
        statuses.push(CaseStatus {
            case: member.case.clone(),
            parent: member.parent,
            protocol,
            revision: frontier.case_revision,
            outcome: state
                .termination
                .as_ref()
                .map(|termination| termination.outcome.clone()),
            stop,
            open_obligations: open,
            case_file: text(&store.join(format!("{}.json", case_file_stem(case)))),
            run_log: text(&log),
            name: member.link.map(|link| link.name),
            protocol_id,
            protocol_revision,
            stop_next,
            evidence: evidence(case, &state.evidence, &lines)?,
            runs: runs(case, &observations),
            approvals: approvals(&lines, bindings),
            copies: copies(case, &observations, &state.evidence),
        });
    }
    Ok(TreeStatus {
        format: STATUS_FORMAT.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        store: text(&store),
        workspace: manifest.workspace,
        root: manifest.root,
        cases: statuses,
        protocols: Vec::new(),
    })
}

/// The lines of the log at `log`; none when it is missing or does not verify (`tree verify`
/// names that).
fn log_lines(log: &Path) -> Result<Vec<RunLogLine>, Refused> {
    let bytes = read_regular(log)?.unwrap_or_default();
    Ok(b10x_loom_runlog::verify(&bytes).unwrap_or_default())
}

/// The stop and the decisions the last `stopped` line of `lines` names.
fn last_stop(lines: &[RunLogLine]) -> (Option<String>, Vec<String>) {
    lines
        .iter()
        .rev()
        .find_map(|line| match &line.event {
            RunLogEvent::Stopped(stopped) => {
                Some((Some(stopped.stop.clone()), stopped.next.clone()))
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// The records of `case`, in the order received, each with the action of the step whose `action
/// finished` line names it.
fn evidence(
    case: &str,
    held: &[loom_governor::HeldEvidence],
    lines: &[RunLogLine],
) -> Result<Vec<StatusEvidence>, Refused> {
    let text_of = |facts: &Value, name: &str| facts[name].as_str().map(str::to_owned);
    held.iter()
        .enumerate()
        .map(|(index, record)| {
            let facts = plain_commission(&record.data.facts);
            let id = record.data.evidence_id.0.0.clone();
            let refused = || {
                unreadable(
                    format!("case {case}"),
                    format!("the record at index {index} is not a canon-evidence/1 record"),
                )
            };
            if facts["format"] != "canon-evidence/1" {
                return Err(refused());
            }
            let action = lines.iter().find_map(|line| match &line.event {
                RunLogEvent::ActionFinished(finished) if finished.evidence.contains(&id) => {
                    Some(finished.action.clone())
                }
                _ => None,
            });
            let upstream = facts
                .get("upstream_revisions")
                .cloned()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            Ok(StatusEvidence {
                index: i64::try_from(index).unwrap_or(i64::MAX),
                evidence_id: id,
                kind: text_of(&facts, "kind").ok_or_else(refused)?,
                result: text_of(&facts, "result"),
                subject: text_of(&facts, "subject").ok_or_else(refused)?,
                subject_revision: text_of(&facts, "subject_revision").ok_or_else(refused)?,
                upstream_revisions: crate::encode::loom_json(&upstream),
                producer: record.data.producer.clone(),
                action,
            })
        })
        .collect()
}

/// The runs of the bound programs of `case`, in the order the store keeps their observations.
fn runs(case: &str, observations: &[ObservationData]) -> Vec<StatusRun> {
    let payloads: Vec<Value> = observations
        .iter()
        .filter(|observation| observation.source == PROGRAM_SOURCE)
        .map(|observation| plain_commission(&observation.payload))
        .filter(|payload| payload["case"] == case)
        .collect();
    let mut runs: Vec<StatusRun> = Vec::new();
    for payload in &payloads {
        let action = payload["action"].as_str().unwrap_or_default().to_owned();
        let retry_of: Vec<String> = payload["retry_of"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let before = runs.iter_mut().rev().find(|run| run.action == action);
        let attempt = match (retry_of.is_empty(), before) {
            (true, before) => {
                if let Some(before) = before {
                    before.retried = false;
                }
                1
            }
            (false, Some(before)) => {
                before.retried = true;
                before.attempt + 1
            }
            (false, None) => 1,
        };
        runs.push(StatusRun {
            action,
            exit: payload["exit"].as_i64(),
            attempt,
            retried: false,
            retry_of,
        });
    }
    runs
}

/// Each `tree approve` on the case, from its `authority recorded` lines in the order recorded.
/// The action `--action` named is the one of the `action finished` line that follows the
/// approval at once, when the case binds that action to a person: a run never takes a person's
/// action, so only `tree approve --action` writes that line there.
fn approvals(lines: &[RunLogLine], held: Option<&Held>) -> Vec<StatusApproval> {
    let person: BTreeSet<&str> = held
        .map(|held| {
            held.bindings
                .actions
                .iter()
                .filter(|bound| bound.executor.kind == Executor::Person)
                .map(|bound| bound.action.as_str())
                .collect()
        })
        .unwrap_or_default();
    lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| {
            let RunLogEvent::AuthorityRecorded(recorded) = &line.event else {
                return None;
            };
            let answer = if recorded.decision == "Deny" {
                ApprovalDecision::Deny
            } else {
                ApprovalDecision::Allow
            };
            let action = lines.get(at + 1).and_then(|next| match &next.event {
                RunLogEvent::ActionFinished(finished)
                    if answer == ApprovalDecision::Allow
                        && person.contains(finished.action.as_str()) =>
                {
                    Some(finished.action.clone())
                }
                _ => None,
            });
            Some(StatusApproval {
                capability: recorded.capability.clone(),
                action,
                answer,
                actor: recorded.principal.clone(),
                at: line.at.0.clone(),
            })
        })
        .collect()
}

/// The records a `copy` binding copied into `case`, in the order the store keeps the copy's
/// observations, each with the kind of the copy the case holds.
fn copies(
    case: &str,
    observations: &[ObservationData],
    held: &[loom_governor::HeldEvidence],
) -> Vec<StatusCopy> {
    observations
        .iter()
        .filter(|observation| observation.source == COPY_SOURCE)
        .filter_map(|observation| {
            let payload = plain_commission(&observation.payload);
            if payload["case"] != case {
                return None;
            }
            let kind = held.iter().find_map(|record| {
                record
                    .data
                    .observation_ids
                    .contains(&observation.observation_id)
                    .then(|| record.data.kind.clone())
            })?;
            Some(StatusCopy {
                source_case: payload["source_case"].as_str()?.to_owned(),
                target_case: case.to_owned(),
                record_kind: kind,
            })
        })
        .collect()
}

fn text(path: &Path) -> String {
    path.display().to_string()
}
