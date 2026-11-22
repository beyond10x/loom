//! The approvals of a tree: a principal's decision on one capability for one case at one case
//! revision (`loom.tree.CaseApproval`), kept beside the case file as `approvals-<stem>.json`, and
//! the authority provider Commission's loop asks over them.
//!
//! An approval allows its capability for its case while the case is at the revision it was
//! recorded at, and nothing else: another case, another capability or a moved case reads no
//! approval, and the provider answers `ApprovalRequired`. A `Deny` wins over an `Allow` recorded for
//! the same case, capability and revision.

use std::collections::BTreeMap;
use std::path::Path;

use b10x_loom_commission::model::responsibility::{
    ActionRequestData, AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny,
    CommissionData, Unit,
};
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom::primitives::Timestamp;
use loom::tree::{ApprovalDecision, CaseApproval};
use loom_governor::case_file_stem;
use serde_json::{Value, json};

use crate::store::{document, integer, read_regular, text, write_file};
use crate::{Refused, unreadable};

/// The name of the file that holds the approvals of `case`.
pub fn approvals_file_name(case: &str) -> String {
    format!("approvals-{}.json", case_file_stem(case))
}

fn approval_json(approval: &CaseApproval) -> Value {
    json!({
        "case": approval.case,
        "capability": approval.capability,
        "decision": match approval.decision {
            ApprovalDecision::Allow => "Allow",
            ApprovalDecision::Deny => "Deny",
        },
        "principal": approval.principal,
        "case_revision": approval.case_revision,
        "at": approval.at.0,
    })
}

/// The approvals recorded for `case` in the store `dir`, in the order recorded.
///
/// # Errors
/// `StoreUnreadable` when the file cannot be read or holds anything but a list of approvals of
/// `case`.
pub fn read_approvals(dir: &Path, case: &str) -> Result<Vec<CaseApproval>, Refused> {
    let path = dir.join(approvals_file_name(case));
    let Some(bytes) = read_regular(&path)? else {
        return Ok(Vec::new());
    };
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| unreadable(path.display(), error))?;
    let Value::Array(items) = value else {
        return Err(unreadable(path.display(), "is not a list of approvals"));
    };
    let mut approvals = Vec::with_capacity(items.len());
    for item in items {
        let Value::Object(object) = item else {
            return Err(unreadable(
                path.display(),
                "holds an entry that is not an object",
            ));
        };
        let named = text(&object, "case", &path)?;
        if named != case {
            return Err(unreadable(
                path.display(),
                format!("holds an approval of case {named:?}, not of {case:?}"),
            ));
        }
        let decision = match text(&object, "decision", &path)? {
            "Allow" => ApprovalDecision::Allow,
            "Deny" => ApprovalDecision::Deny,
            other => {
                return Err(unreadable(
                    path.display(),
                    format!("holds the decision {other:?}, neither Allow nor Deny"),
                ));
            }
        };
        approvals.push(CaseApproval {
            case: case.to_owned(),
            capability: text(&object, "capability", &path)?.to_owned(),
            decision,
            principal: text(&object, "principal", &path)?.to_owned(),
            case_revision: integer(&object, "case_revision", &path)?,
            at: Timestamp(text(&object, "at", &path)?.to_owned()),
        });
    }
    Ok(approvals)
}

/// Records `approval` for its case in the store `dir`. One already recorded for the same
/// capability, principal and case revision is replaced, so recording the same decision twice adds
/// nothing. Called holding the store.
///
/// # Errors
/// `StoreUnreadable` when the file cannot be read or written.
pub(crate) fn record_approval(dir: &Path, approval: &CaseApproval) -> Result<(), Refused> {
    let mut approvals = read_approvals(dir, &approval.case)?;
    approvals.retain(|held| {
        !(held.capability == approval.capability
            && held.principal == approval.principal
            && held.case_revision == approval.case_revision)
    });
    approvals.push(approval.clone());
    let list: Vec<Value> = approvals.iter().map(approval_json).collect();
    write_file(
        dir,
        &approvals_file_name(&approval.case),
        &document(&Value::Array(list)),
    )
}

/// The authority provider of a tree run: it answers from the approvals the store held when the
/// run began (only `tree approve`, another writer, records one).
#[derive(Debug, Default)]
pub struct TreeAuthority {
    approvals: BTreeMap<String, Vec<CaseApproval>>,
}

impl TreeAuthority {
    /// The provider over `approvals`, by case.
    pub fn new(approvals: BTreeMap<String, Vec<CaseApproval>>) -> Self {
        Self { approvals }
    }

    /// The decision recorded for `capability` on `case` at `case_revision`: `Deny` when any
    /// principal denied it, else `Allow` when one allowed it, else none.
    pub fn decision(
        &self,
        case: &str,
        capability: &str,
        case_revision: i64,
    ) -> Option<ApprovalDecision> {
        let matching: Vec<&CaseApproval> = self
            .approvals
            .get(case)
            .into_iter()
            .flatten()
            .filter(|approval| {
                approval.capability == capability && approval.case_revision == case_revision
            })
            .collect();
        if matching
            .iter()
            .any(|approval| approval.decision == ApprovalDecision::Deny)
        {
            Some(ApprovalDecision::Deny)
        } else if matching.is_empty() {
            None
        } else {
            Some(ApprovalDecision::Allow)
        }
    }
}

impl AuthorityProvider for TreeAuthority {
    fn decide(
        &self,
        commission: &CommissionData,
        request: &ActionRequestData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        if commission.case_id != request.case_id {
            return Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!(
                    "the request is for case {}, and the commission for case {}",
                    request.case_id.0, commission.case_id.0
                ),
            }));
        }
        let case = request.case_id.0.as_str();
        let revision = request.expected_case_revision;
        Ok(match self.decision(case, capability, revision) {
            Some(ApprovalDecision::Allow) => AuthorityVerdict::Allow(Unit(true)),
            Some(ApprovalDecision::Deny) => AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!("{capability} is denied for case {case} at revision {revision}"),
            }),
            None => AuthorityVerdict::ApprovalRequired(AuthorityVerdictApprovalRequired {
                request: format!(
                    "{capability} for case {case} at revision {revision}, approved by no one"
                ),
            }),
        })
    }
}
