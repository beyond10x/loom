//! The grant record (`intake.routing.Grant`, `ess/intake/domains/routing.yaml`): an operator's
//! approval of the one request a commission's Suspension holds, written by `b10x-loom approve`
//! into the directory `run --state` keeps the commission in, and read by `b10x-loom resume`
//! through [`GrantAuthority`].
//!
//! [`GrantStore`] keeps one file per commission, `grant-<hex of the commission id>.json`, holding
//! the generated [`GrantData`] as a JSON object with one member per declared field, under the
//! write discipline of [`crate::state_dir`]. A later approval of the commission's next Suspension
//! replaces it. A document naming a member its declaration does not, missing one, or holding
//! another commission's grant is refused on read.
//!
//! [`approve`] performs nothing: it reads the commission's Suspension and writes a grant for its
//! request, and only when the Suspension is `Pending` and the request needs the capability the
//! operator named, and never for a `repository.merge` that would hand off a `HEAD` the commission
//! already handed off (`intake.routing.HeadAlreadyHandedOff`).
//!
//! [`GrantAuthority`] allows a capability only while the commission's Suspension is held `Pending`
//! for the very request the grant names, for that capability: a grant for another request, or a
//! request proposed again later in the resumed run (whose Suspension the resume has taken and
//! released by then), is not allowed by it. Everything else needs approval, as the slice's own
//! provider answers. A suspension store that cannot be read is a failure to decide, which never
//! allows.
//!
//! With Mandate (`approve --mandate`, `resume --mandate`, `story:mandate-approval-on-resume`), a
//! [`MandateApprovals`] the host built from its Mandate file takes part: [`approve`] records a
//! Mandate approval for the held request and the grant names it (`mandate_approval`), and
//! [`GrantAuthority`] allows the held request only when Mandate clears and consumes the approval
//! the grant names; a grant naming none is refused. Without it [`approve`] writes a grant naming no
//! approval: its document has no `mandate_approval` member, as a grant written before the member
//! was declared has none, and [`GrantAuthority`] allows such a grant as before. Only Mandate clears
//! a grant naming an approval: without it [`GrantAuthority`] denies one, so leaving out Mandate
//! performs nothing and leaves the approval unconsumed.

use std::fmt;
use std::path::{Path, PathBuf};

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::responsibility::{
    AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny,
    CaseId, CommissionData, CommissionId as HeldCommissionId, SuspensionReason, SuspensionState,
    Unit,
};
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use intake_model::primitives::{Timestamp, Uuid};
pub use intake_model::routing::{ActionRequestId, CommissionId, GrantData, MandateApprovalId};
use serde_json::{Map, Value, json};

use crate::handoff::{HeadAlreadyHandedOff, MERGE, MergeHandoffStore};
use crate::resume::ResumeStoreError;
use crate::state_dir::{StateDir, failed, read_text};

/// The prefix of a grant's file name.
const PREFIX: &str = "grant-";

/// The members a grant has, in the order written; [`MANDATE_APPROVAL`] follows them when the grant
/// names a Mandate approval.
const MEMBERS: [&str; 6] = [
    "commission_id",
    "action",
    "capability",
    "granted",
    "action_request_id",
    "granted_at",
];

/// The member naming the grant's Mandate approval, present only when it names one.
const MANDATE_APPROVAL: &str = "mandate_approval";

/// The request a commission's Suspension holds, as `approve --mandate` approves it.
#[derive(Debug, Clone, Copy)]
pub struct HeldApproval<'a> {
    /// The commission holding the Suspension.
    pub commission_id: &'a CommissionId,
    /// The request the Suspension holds.
    pub action_request_id: &'a ActionRequestId,
    /// The case the request is on.
    pub case_id: &'a CaseId,
    /// The capability the request needs, as the operator approved it.
    pub capability: &'a str,
    /// When it is approved.
    pub at: &'a Timestamp,
}

/// Mandate, as the host configured it for `approve --mandate` and `resume --mandate`
/// (`story:mandate-approval-on-resume`). The command line implements it over `loom_mandate` and the
/// state directory's approval event log ([`crate::mandate_log`]).
pub trait MandateApprovals {
    /// Records a Mandate approval of `held`, and answers its identity, which the grant names.
    ///
    /// # Errors
    /// Why no approval was recorded.
    fn record(&self, held: &HeldApproval<'_>) -> Result<MandateApprovalId, String>;

    /// The authority context a resumed commission carries: the host's Mandate verified context.
    fn authority_context(&self) -> AuthorityContext;

    /// Clears `capability` for the held request `grant` names, of `commission`, through Mandate's
    /// `check_with_approval` with the approval the grant names, and keeps the consumption before
    /// it answers.
    ///
    /// # Errors
    /// Why Mandate did not clear it; nothing is consumed then.
    fn clear(
        &self,
        commission: &CommissionData,
        capability: &str,
        grant: &GrantData,
        approval: &MandateApprovalId,
    ) -> Result<(), String>;
}

/// The grants kept in one directory, one file per commission.
#[derive(Debug)]
pub struct GrantStore {
    dir: StateDir,
}

impl GrantStore {
    /// A store over `dir`, created with its lock file when it does not exist yet, and proven to
    /// take a write, as [`crate::resume::ResumeContextStore::open`] opens it.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the directory or its lock file cannot be created, or the directory
    /// does not take a write.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ResumeStoreError> {
        Ok(Self {
            dir: StateDir::open(dir.into(), "grant")?,
        })
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.dir()
    }

    /// Keeps `grant` for its commission, replacing the one kept before, if any.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the grant would not read back as written or the file cannot be
    /// written; the grant kept before stays then.
    pub fn put(&self, grant: &GrantData) -> Result<(), ResumeStoreError> {
        let path = self.path(&grant.commission_id)?;
        let text = encode(grant);
        if decode(&text, &grant.commission_id).as_ref() != Ok(grant) {
            return Err(failed(&path, "the grant would not read back as written"));
        }
        self.dir.locked(|| self.dir.replace(&path, &text))?
    }

    /// The grant kept for `commission`, or `None` when none is.
    ///
    /// # Errors
    /// [`ResumeStoreError`] when the file cannot be read or is not a grant of `commission`.
    pub fn get(&self, commission: &CommissionId) -> Result<Option<GrantData>, ResumeStoreError> {
        let path = self.path(commission)?;
        self.dir.locked(|| {
            let Some(text) = read_text(&path)? else {
                return Ok(None);
            };
            decode(&text, commission)
                .map(Some)
                .map_err(|problem| failed(&path, problem))
        })?
    }

    fn path(&self, commission: &CommissionId) -> Result<PathBuf, ResumeStoreError> {
        self.dir.file(PREFIX, "a commission id", &commission.0.0)
    }
}

/// Why [`approve`] wrote no grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApproveRefusal {
    /// The commission holds no Suspension.
    NothingSuspended,
    /// The commission's Suspension is `Resuming`: a resume took it, and its request may have been
    /// performed. It is not approved again.
    InDoubt {
        /// The request whose performance is in doubt.
        action_request_id: String,
    },
    /// The suspended request needs another capability than the one approved, or the Suspension
    /// names none.
    OtherCapability {
        /// The capability the Suspension names, if any.
        held: Option<String>,
    },
    /// The suspended request is a `repository.merge` that would hand off a `HEAD` the commission
    /// already handed off (`intake.routing.HeadAlreadyHandedOff`): the same work is not handed
    /// off twice.
    AlreadyHandedOff(HeadAlreadyHandedOff),
    /// The suspension store, the grant store, the case or the hand-offs could not be read or
    /// written.
    Store(String),
    /// Mandate recorded no approval for the request (`approve --mandate`).
    Mandate(String),
}

impl fmt::Display for ApproveRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NothingSuspended => {
                write!(f, "NothingSuspended: the commission holds no Suspension")
            }
            Self::InDoubt { action_request_id } => write!(
                f,
                "InDoubt: the Suspension of request {action_request_id} is being resumed, so its \
                 request may have been performed"
            ),
            Self::OtherCapability { held: Some(held) } => write!(
                f,
                "OtherCapability: the suspended request needs `{held}`, not the capability approved"
            ),
            Self::OtherCapability { held: None } => write!(
                f,
                "OtherCapability: the Suspension names no capability its request needs"
            ),
            Self::AlreadyHandedOff(error) => write!(
                f,
                "AlreadyHandedOff: commission {} already handed off HEAD {}; the same work is not \
                 handed off twice",
                error.commission_id.0.0, error.head
            ),
            Self::Store(why) => write!(f, "the state directory cannot be used: {why}"),
            Self::Mandate(why) => write!(f, "Mandate recorded no approval: {why}"),
        }
    }
}

impl std::error::Error for ApproveRefusal {}

/// The machine's clock now, as a grant records when it was granted.
pub fn now() -> Timestamp {
    Timestamp(crate::executor::now().0)
}

/// A fresh identity of the kind `kind` names (a Mandate approval, a Mandate decision), never
/// issued before by this process or, by its clock, by another.
pub fn fresh_id(kind: &str, seed: &str) -> Uuid {
    Uuid(crate::executor::fresh_uuid(kind, seed).0)
}

/// The instant `seconds` after `at`, in UTC to the second, as a Mandate approval's expiry is
/// written.
///
/// # Errors
/// Why not: `at` is no RFC 3339 instant, or the instant cannot be represented.
pub fn after(at: &Timestamp, seconds: i64) -> Result<Timestamp, String> {
    let start = chrono::DateTime::parse_from_rfc3339(&at.0)
        .map_err(|error| format!("`{}` is no RFC 3339 instant: {error}", at.0))?;
    let end = chrono::TimeDelta::try_seconds(seconds)
        .and_then(|delta| start.checked_add_signed(delta))
        .ok_or_else(|| format!("{seconds} seconds after {} cannot be represented", at.0))?;
    Ok(Timestamp(
        end.with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
    ))
}

/// Approves `capability` for the request `commission`'s Suspension holds, at `at`: writes the
/// grant and performs nothing. A suspended `repository.merge` hands off the case's
/// implementation revision, which `head_of` reads from the governor for the Suspension's case;
/// when the commission already handed that `HEAD` off (`handoffs`), nothing is approved. With
/// `mandate`, a Mandate approval of the request is recorded first and the grant names it.
///
/// # Errors
/// [`ApproveRefusal`] when nothing is suspended for the commission, its Suspension is
/// `Resuming`, its request needs another capability, it would hand off a `HEAD` the commission
/// already handed off, Mandate recorded no approval, or a store cannot be used; no grant is
/// written then.
#[allow(clippy::too_many_arguments)]
pub fn approve(
    suspensions: &dyn SuspensionPort,
    grants: &GrantStore,
    handoffs: &MergeHandoffStore,
    head_of: &dyn Fn(&CaseId) -> Result<String, String>,
    commission: &str,
    capability: &str,
    at: Timestamp,
    mandate: Option<&dyn MandateApprovals>,
) -> Result<GrantData, ApproveRefusal> {
    let held = suspensions
        .get(&HeldCommissionId(
            b10x_loom_commission::model::primitives::Uuid(commission.to_owned()),
        ))
        .map_err(|error| ApproveRefusal::Store(error.to_string()))?
        .ok_or(ApproveRefusal::NothingSuspended)?;
    if held.state != SuspensionState::Pending {
        return Err(ApproveRefusal::InDoubt {
            action_request_id: held.data.action_request_id.0.0,
        });
    }
    let needed = needed_capability(&held.data.reason);
    if needed.as_deref() != Some(capability) {
        return Err(ApproveRefusal::OtherCapability { held: needed });
    }
    if held.data.action == MERGE {
        let head = head_of(&held.data.case_id).map_err(ApproveRefusal::Store)?;
        let commission_id = CommissionId(Uuid(commission.to_owned()));
        if handoffs
            .handed_off(&commission_id, &head)
            .map_err(|error| ApproveRefusal::Store(error.to_string()))?
            .is_some()
        {
            return Err(ApproveRefusal::AlreadyHandedOff(HeadAlreadyHandedOff {
                commission_id,
                head,
            }));
        }
    }
    let commission_id = CommissionId(Uuid(commission.to_owned()));
    let action_request_id = ActionRequestId(Uuid(held.data.action_request_id.0.0));
    let mandate_approval = match mandate {
        Some(mandate) => Some(
            mandate
                .record(&HeldApproval {
                    commission_id: &commission_id,
                    action_request_id: &action_request_id,
                    case_id: &held.data.case_id,
                    capability,
                    at: &at,
                })
                .map_err(ApproveRefusal::Mandate)?,
        ),
        None => None,
    };
    let grant = GrantData {
        commission_id,
        action: held.data.action,
        capability: capability.to_owned(),
        granted: true,
        action_request_id,
        granted_at: at,
        mandate_approval,
    };
    grants
        .put(&grant)
        .map_err(|error| ApproveRefusal::Store(error.to_string()))?;
    Ok(grant)
}

/// The capability a Suspension held on a withheld approval names (`SuspensionReason::Authority`,
/// member `capability`), if any.
pub fn needed_capability(reason: &SuspensionReason) -> Option<String> {
    let SuspensionReason::Authority(json::Value::Object(members)) = reason else {
        return None;
    };
    members.iter().find_map(|(name, value)| match value {
        json::Value::Text(capability) if name == "capability" => Some(capability.clone()),
        _ => None,
    })
}

/// The authority provider `resume` reads the grant through (see the module documentation).
pub struct GrantAuthority<'a> {
    grant: Option<GrantData>,
    suspensions: &'a dyn SuspensionPort,
    mandate: Option<&'a dyn MandateApprovals>,
}

impl<'a> GrantAuthority<'a> {
    /// A provider over `grant`, the one kept for the commission if any, that checks it against
    /// the commission's Suspension in `suspensions` at every call. Without
    /// [`GrantAuthority::with_mandate`] it denies a grant naming a Mandate approval, which only
    /// Mandate clears.
    pub fn new(grant: Option<GrantData>, suspensions: &'a dyn SuspensionPort) -> Self {
        Self {
            grant,
            suspensions,
            mandate: None,
        }
    }

    /// The same provider, which allows the held request only once `mandate` cleared and consumed
    /// the Mandate approval the grant names; a grant naming none allows nothing and is denied.
    #[must_use]
    pub fn with_mandate(mut self, mandate: &'a dyn MandateApprovals) -> Self {
        self.mandate = Some(mandate);
        self
    }

    fn allows(&self, commission: &CommissionData, capability: &str) -> Result<bool, String> {
        let Some(grant) = &self.grant else {
            return Ok(false);
        };
        if !grant.granted
            || grant.commission_id.0.0 != commission.commission_id.0.0
            || grant.capability != capability
        {
            return Ok(false);
        }
        let held = self
            .suspensions
            .get(&commission.commission_id)
            .map_err(|error| error.to_string())?;
        Ok(held.is_some_and(|held| {
            held.state == SuspensionState::Pending
                && held.data.commission_id.0.0 == grant.commission_id.0.0
                && held.data.action_request_id.0.0 == grant.action_request_id.0.0
                && held.data.action == grant.action
                && needed_capability(&held.data.reason).as_deref() == Some(capability)
        }))
    }
}

impl AuthorityProvider for GrantAuthority<'_> {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        if let Some(grant) = &self.grant
            && self
                .allows(commission, capability)
                .map_err(AuthorityProviderError::new)?
        {
            // The held request, granted. Only Mandate clears a grant naming a Mandate approval,
            // consuming it; a grant naming none is allowed only without Mandate.
            let deny = |reason: String| AuthorityVerdict::Deny(AuthorityVerdictDeny { reason });
            return Ok(match (self.mandate, &grant.mandate_approval) {
                (None, None) => AuthorityVerdict::Allow(Unit(true)),
                (None, Some(approval)) => deny(format!(
                    "the grant for `{capability}` names Mandate approval {}, which only Mandate \
                     clears; resume it with --mandate",
                    approval.0.0
                )),
                (Some(_), None) => deny(format!(
                    "the grant for `{capability}` names no Mandate approval; approve it with --mandate"
                )),
                (Some(mandate), Some(approval)) => {
                    match mandate.clear(commission, capability, grant, approval) {
                        Ok(()) => AuthorityVerdict::Allow(Unit(true)),
                        Err(reason) => deny(reason),
                    }
                }
            });
        }
        Ok(AuthorityVerdict::ApprovalRequired(
            AuthorityVerdictApprovalRequired {
                request: format!("approval for `{capability}`"),
            },
        ))
    }
}

/// `grant` as the JSON document the store writes.
fn encode(grant: &GrantData) -> String {
    let mut value = json!({
        "commission_id": grant.commission_id.0.0,
        "action": grant.action,
        "capability": grant.capability,
        "granted": grant.granted,
        "action_request_id": grant.action_request_id.0.0,
        "granted_at": grant.granted_at.0,
    });
    if let Some(approval) = &grant.mandate_approval {
        value[MANDATE_APPROVAL] = json!(approval.0.0);
    }
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}

/// The grant `text` holds, refused unless it is `commission`'s and names exactly its members.
fn decode(text: &str, commission: &CommissionId) -> Result<GrantData, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = if value.get(MANDATE_APPROVAL).is_some() {
        let mut members = MEMBERS.to_vec();
        members.push(MANDATE_APPROVAL);
        exactly(&value, &members)?
    } else {
        exactly(&value, &MEMBERS)?
    };
    let grant = GrantData {
        commission_id: CommissionId(Uuid(text_of(object, "commission_id")?)),
        action: text_of(object, "action")?,
        capability: text_of(object, "capability")?,
        granted: object["granted"]
            .as_bool()
            .ok_or("`granted` is not a boolean")?,
        action_request_id: ActionRequestId(Uuid(text_of(object, "action_request_id")?)),
        granted_at: Timestamp(text_of(object, "granted_at")?),
        mandate_approval: match object.get(MANDATE_APPROVAL) {
            Some(_) => Some(MandateApprovalId(Uuid(text_of(object, MANDATE_APPROVAL)?))),
            None => None,
        },
    };
    if &grant.commission_id != commission {
        return Err(format!(
            "holds the grant of commission {}, not {}",
            grant.commission_id.0.0, commission.0.0
        ));
    }
    Ok(grant)
}

/// `value` as an object naming exactly `members`.
pub(crate) fn exactly<'v>(
    value: &'v Value,
    members: &[&str],
) -> Result<&'v Map<String, Value>, String> {
    let object = value.as_object().ok_or("the document is not an object")?;
    if let Some(unknown) = object.keys().find(|key| !members.contains(&key.as_str())) {
        return Err(format!(
            "the document names `{unknown}`, which it does not declare"
        ));
    }
    if let Some(missing) = members.iter().find(|member| !object.contains_key(**member)) {
        return Err(format!("the document has no `{missing}`"));
    }
    Ok(object)
}

pub(crate) fn text_of(object: &Map<String, Value>, member: &str) -> Result<String, String> {
    object[member]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("`{member}` is not text"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(commission: &str) -> GrantData {
        GrantData {
            commission_id: CommissionId(Uuid(commission.to_owned())),
            action: "repository.merge".to_owned(),
            capability: "repository.merge".to_owned(),
            granted: true,
            action_request_id: ActionRequestId(Uuid(
                "00000000-0000-4000-8000-000000000009".to_owned(),
            )),
            granted_at: Timestamp("2026-10-13T08:00:00Z".to_owned()),
            mandate_approval: None,
        }
    }

    #[test]
    fn a_grant_reads_back_as_kept_is_replaced_by_the_next_and_a_foreign_one_is_refused() {
        let dir = tempfile::Builder::new()
            .prefix("grant-store-")
            .tempdir()
            .unwrap();
        let store = GrantStore::open(dir.path()).unwrap();
        let first = grant("00000000-0000-4000-8000-000000000001");
        assert_eq!(store.get(&first.commission_id).unwrap(), None);
        store.put(&first).unwrap();
        assert_eq!(
            store.get(&first.commission_id).unwrap(),
            Some(first.clone())
        );
        let mut next = first.clone();
        next.action_request_id = ActionRequestId(Uuid("another".to_owned()));
        store.put(&next).unwrap();
        assert_eq!(store.get(&first.commission_id).unwrap(), Some(next));

        // A file named for one commission that holds another's grant is refused.
        let other = CommissionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned()));
        let path = store.path(&other).unwrap();
        std::fs::write(&path, encode(&first)).unwrap();
        assert!(store.get(&other).is_err());
        // So is one naming a member the declaration does not.
        let mut extended: Value = serde_json::from_str(&encode(&first)).unwrap();
        extended["note"] = json!("x");
        std::fs::write(
            store.path(&first.commission_id).unwrap(),
            extended.to_string(),
        )
        .unwrap();
        assert!(store.get(&first.commission_id).is_err());
    }

    /// A grant naming its Mandate approval keeps it as the member `mandate_approval`; a grant naming
    /// none has no such member, as a grant written before it was declared, and reads back so.
    #[test]
    fn a_grant_names_its_mandate_approval_only_when_it_carries_one() {
        let dir = tempfile::Builder::new()
            .prefix("grant-mandate-")
            .tempdir()
            .unwrap();
        let store = GrantStore::open(dir.path()).unwrap();
        let plain = grant("00000000-0000-4000-8000-000000000001");
        let text: Value = serde_json::from_str(&encode(&plain)).unwrap();
        assert_eq!(text.get(MANDATE_APPROVAL), None, "{text}");
        store.put(&plain).unwrap();
        assert_eq!(
            store.get(&plain.commission_id).unwrap(),
            Some(plain.clone())
        );

        let mut carried = plain.clone();
        carried.mandate_approval = Some(MandateApprovalId(Uuid(
            "00000000-0000-4000-8000-0000000000ab".to_owned(),
        )));
        let text: Value = serde_json::from_str(&encode(&carried)).unwrap();
        assert_eq!(
            text[MANDATE_APPROVAL],
            json!("00000000-0000-4000-8000-0000000000ab")
        );
        store.put(&carried).unwrap();
        assert_eq!(store.get(&carried.commission_id).unwrap(), Some(carried));
    }

    /// The provider allows only while the commission's Suspension is `Pending` for the request the
    /// grant names, for its capability: not before it is held, not for another request, not once
    /// a resume took it, and not after it is released.
    #[test]
    fn a_grant_allows_only_the_request_the_pending_suspension_holds() {
        use b10x_loom_commission::model::primitives::Uuid as HeldUuid;
        use b10x_loom_commission::model::responsibility::{
            ActionRequestId as HeldRequestId, AgentRevisionId, AuthorityContext, CaseId,
            HoldSuspension, HoldSuspensionOutcome, PrincipalId, ProposedActionArguments,
            ReleaseSuspension, RunId, SuspensionKey, TakeSuspension,
        };
        use loom_governor::FileSuspensionStore;

        let dir = tempfile::Builder::new()
            .prefix("grant-authority-")
            .tempdir()
            .unwrap();
        let suspensions = FileSuspensionStore::open(dir.path()).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        let commission = CommissionData {
            commission_id: HeldCommissionId(HeldUuid(id.to_owned())),
            agent_revision_id: AgentRevisionId(HeldUuid("agent".to_owned())),
            case_id: CaseId("case-1".to_owned()),
            principal: PrincipalId("operator".to_owned()),
            authority_context: AuthorityContext(json::Value::Null),
        };
        let granted = grant(id);
        let request = HeldRequestId(HeldUuid(granted.action_request_id.0.0.clone()));
        let key = SuspensionKey {
            commission_id: commission.commission_id.clone(),
        };
        let allows = |grant: Option<GrantData>, capability: &str| {
            matches!(
                GrantAuthority::new(grant, &suspensions).decide(&commission, capability),
                Ok(AuthorityVerdict::Allow(_))
            )
        };
        assert!(
            !allows(Some(granted.clone()), "repository.merge"),
            "nothing held"
        );

        let held = suspensions.hold(HoldSuspension {
            suspension: key.clone(),
            commission_id: commission.commission_id.clone(),
            run_id: RunId(HeldUuid("00000000-0000-4000-8000-000000000003".to_owned())),
            run_case_revision: 1,
            action_request_id: request.clone(),
            case_id: commission.case_id.clone(),
            expected_case_revision: 3,
            action: "repository.merge".to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
            reason: SuspensionReason::Authority(json::Value::Object(vec![(
                "capability".to_owned(),
                json::Value::Text("repository.merge".to_owned()),
            )])),
        });
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "{held:?}"
        );
        assert!(allows(Some(granted.clone()), "repository.merge"));
        assert!(!allows(None, "repository.merge"), "no grant");
        assert!(
            !allows(Some(granted.clone()), "implementation.edit"),
            "another capability"
        );
        let mut other = granted.clone();
        other.action_request_id = ActionRequestId(Uuid("another-request".to_owned()));
        assert!(!allows(Some(other), "repository.merge"), "another request");
        let mut withheld = granted.clone();
        withheld.granted = false;
        assert!(!allows(Some(withheld), "repository.merge"), "not granted");
        // A grant naming a Mandate approval is Mandate's to clear: without it, it is denied.
        let mut carried = granted.clone();
        carried.mandate_approval = Some(MandateApprovalId(Uuid(
            "00000000-0000-4000-8000-0000000000ab".to_owned(),
        )));
        assert!(
            matches!(
                GrantAuthority::new(Some(carried), &suspensions).decide(&commission, "repository.merge"),
                Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny { reason })) if reason.contains("--mandate")
            ),
            "a grant naming a Mandate approval, without Mandate"
        );

        suspensions.take(TakeSuspension {
            suspension: key.clone(),
            action_request_id: request.clone(),
        });
        assert!(!allows(Some(granted.clone()), "repository.merge"), "taken");
        suspensions.release(ReleaseSuspension {
            suspension: key,
            action_request_id: request,
        });
        assert!(!allows(Some(granted), "repository.merge"), "released");
    }

    #[test]
    fn the_capability_is_read_from_an_authority_reason_only() {
        let reason = SuspensionReason::Authority(json::Value::Object(vec![
            (
                "capability".to_owned(),
                json::Value::Text("repository.merge".to_owned()),
            ),
            (
                "request".to_owned(),
                json::Value::Text("approval".to_owned()),
            ),
        ]));
        assert_eq!(
            needed_capability(&reason).as_deref(),
            Some("repository.merge")
        );
        let budget = SuspensionReason::Budget(json::Value::Object(vec![(
            "capability".to_owned(),
            json::Value::Text("repository.merge".to_owned()),
        )]));
        assert_eq!(needed_capability(&budget), None);
    }

    /// A resume holds no Suspension for a merge at a `HEAD` its commission already handed off
    /// (`story:handoff-holds-no-suspension`), so `approve` never meets one there; should a
    /// Suspension for one be held anyway, `approve` still refuses it `AlreadyHandedOff` and writes
    /// no grant, and approves the same Suspension once the case is at a `HEAD` not handed off.
    #[test]
    fn approve_refuses_a_merge_at_a_head_already_handed_off_and_writes_no_grant() {
        use b10x_loom_commission::model::primitives::Uuid as HeldUuid;
        use b10x_loom_commission::model::responsibility::{
            ActionRequestId as HeldRequestId, HoldSuspension, HoldSuspensionOutcome,
            ProposedActionArguments, RunId, SuspensionKey,
        };
        use loom_governor::FileSuspensionStore;

        use crate::handoff::{RecordMergeHandoff, RecordMergeHandoffOutcome};

        let dir = tempfile::Builder::new()
            .prefix("approve-handed-off-")
            .tempdir()
            .unwrap();
        let suspensions = FileSuspensionStore::open(dir.path()).unwrap();
        let grants = GrantStore::open(dir.path()).unwrap();
        let handoffs = MergeHandoffStore::open(dir.path()).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        let commission_id = HeldCommissionId(HeldUuid(id.to_owned()));
        let held = suspensions.hold(HoldSuspension {
            suspension: SuspensionKey {
                commission_id: commission_id.clone(),
            },
            commission_id,
            run_id: RunId(HeldUuid("00000000-0000-4000-8000-000000000003".to_owned())),
            run_case_revision: 1,
            action_request_id: HeldRequestId(HeldUuid(
                "00000000-0000-4000-8000-0000000000a2".to_owned(),
            )),
            case_id: CaseId("case-1".to_owned()),
            expected_case_revision: 3,
            action: MERGE.to_owned(),
            arguments: ProposedActionArguments(json::Value::Object(Vec::new())),
            reason: SuspensionReason::Authority(json::Value::Object(vec![(
                "capability".to_owned(),
                json::Value::Text(MERGE.to_owned()),
            )])),
        });
        assert!(
            matches!(held, HoldSuspensionOutcome::Held { .. }),
            "{held:?}"
        );
        let recorded = handoffs.record(&RecordMergeHandoff {
            action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-0000000000a1".into())),
            commission_id: CommissionId(Uuid(id.to_owned())),
            workspace: "/work/space".to_owned(),
            head: "0123abcd".to_owned(),
        });
        assert!(
            matches!(recorded, RecordMergeHandoffOutcome::Recorded { .. }),
            "{recorded:?}"
        );
        let at = Timestamp("2026-10-13T08:00:00Z".to_owned());

        let refused = approve(
            &suspensions,
            &grants,
            &handoffs,
            &|_| Ok("0123abcd".to_owned()),
            id,
            MERGE,
            at.clone(),
            None,
        );
        assert_eq!(
            refused,
            Err(ApproveRefusal::AlreadyHandedOff(HeadAlreadyHandedOff {
                commission_id: CommissionId(Uuid(id.to_owned())),
                head: "0123abcd".to_owned(),
            }))
        );
        assert_eq!(
            grants.get(&CommissionId(Uuid(id.to_owned()))).unwrap(),
            None,
            "no grant is written"
        );

        let approved = approve(
            &suspensions,
            &grants,
            &handoffs,
            &|_| Ok("4567cdef".to_owned()),
            id,
            MERGE,
            at,
            None,
        );
        assert!(approved.is_ok(), "{approved:?}");
    }
}
