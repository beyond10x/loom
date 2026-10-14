//! An approval cleared and consumed through Mandate (`story:mandate-approval-on-resume`).
//!
//! Mandate v0.9.0 has no command that records an approval: an approval is one of its
//! `mandate.delegation` records, and `mandate_authz::check_with_approval` consumes it only in the
//! `ApprovedDecisionRecorded` event it returns, which the caller keeps and applies again on the
//! next load. Loom keeps that event, and the approvals `b10x-loom approve --mandate` records, as
//! the approval event log of the state directory (`intake.routing.MandateApprovalEntry`,
//! `ess/intake/domains/routing.yaml`); this module is what reads and writes Mandate's side of it.
//! It opens no file: the command line keeps the log, under the lock the commission's Suspension
//! is held under.
//!
//! [`HostedMandate`] is Mandate configured from the host-trusted file `--mandate <FILE>` names
//! (`loom.authority.MandateHost`, `ess/domains/authority.yaml`), over Mandate's own doubles until
//! Mandate ships a policy adapter:
//!
//! | Mandate's port | What it holds, for the commission's case |
//! |---|---|
//! | tenancy | the verified context's organization, and its subject as a member |
//! | graph | the subject holding the host's `relation` on the case for every capability |
//! | role catalog | `relation` carrying every capability |
//! | policy | an allow of every capability on the case, except those of `approval_required`, which it allows only after an approval |
//! | delegation records | every approval of the log, with every consumption of the log applied |
//!
//! [`HostedMandate::approval`] is the approval `approve --mandate` records for the request a
//! commission's Suspension holds; [`HostedMandate::clear`] asks `check_with_approval` for that
//! request with it and answers the event to append. The case is the Mandate resource
//! [`crate::case_resource`] derives; the subject is the verified context in the commission's
//! authority context, which a resume fills from the host file ([`HostedMandate::authority_context`]).

use std::fmt;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::responsibility::{AuthorityContext, CaseId, CommissionData};
use intake_model::json as intake_json;
use intake_model::primitives::{Timestamp as LogTimestamp, Uuid as LogUuid};
use intake_model::routing::{
    ActionRequestId, CommissionId, MandateApprovalEntry, MandateApprovalId,
    MandateApprovalRecorded, MandateApprovedDecision,
};
use loom::authority::MandateHost;
use loom::json::{self as loom_json, DecodeError, Value};
use mandate_authz::agent::AgentRecords;
use mandate_authz::decision::{ApprovedDecisionRecorded, DecisionIdAllocator, FixedChallenge};
use mandate_authz::delegation::{Approval, ApprovalState};
use mandate_authz::event::{DelegationEvent, DelegationProjection};
use mandate_authz::{CheckRequest, Pdp, check_with_approval};
use mandate_graph::double::GraphDouble;
use mandate_graph::record::Grant;
use mandate_graph::topology::ResourceRegistry;
use mandate_model::Decision;
use mandate_model::graph::Topology;
use mandate_model::tenancy::{MembershipAuthority, Tenancy};
use mandate_policy::double::PolicyDouble;
use mandate_policy::port::{AttributeSet, ChallengeRequirement};
use mandate_policy::record::{AuthorizationModel, Policy};
use mandate_types::{
    Action, ApprovalId, AuthorityScope, AuthoritySubject, AuthorizationModelId, DecisionId,
    GrantId, OrganizationId, OrganizationMembershipId, PolicyId, PolicyVersion, PrincipalId,
    ResourceId, ResourceRef, ResourceType, Timestamp, Uuid, VerifiedContext,
};

use crate::{VERIFIED, case_resource, verified_context};

/// The members of a `loom.authority.MandateHost` document, in the order declared.
const HOST_MEMBERS: [&str; 7] = [
    "verified",
    "approver",
    "approver_policy",
    "relation",
    "capabilities",
    "approval_required",
    "approval_seconds",
];

/// The policy version the host's policy and authorization model are recorded under.
const HOST_POLICY_VERSION: &str = "loom-mandate-host";

/// Why a host file, an approval or a log entry was refused, or why Mandate did not clear an
/// approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn refused(why: impl Into<String>) -> Refused {
    Refused(why.into())
}

/// Mandate configured from a host file (see the module documentation).
#[derive(Debug, Clone)]
pub struct HostedMandate {
    host: MandateHost,
    verified: VerifiedContext,
    approver: PrincipalId,
    approver_policy: PolicyId,
    capabilities: Vec<Action>,
    approval_required: Vec<Action>,
}

/// The request a commission's Suspension holds, as an approval is recorded for it.
#[derive(Debug, Clone, Copy)]
pub struct HeldRequest<'a> {
    /// The commission holding the Suspension.
    pub commission_id: &'a CommissionId,
    /// The request the Suspension holds.
    pub action_request_id: &'a ActionRequestId,
    /// The case the request is on.
    pub case_id: &'a CaseId,
    /// The capability the request needs.
    pub capability: &'a str,
}

impl HostedMandate {
    /// The host file `text` holds: a `loom.authority.MandateHost` naming exactly its declared
    /// members.
    ///
    /// # Errors
    /// [`Refused`] naming the member, for text that is not JSON, a missing or unknown member, a
    /// member of the wrong type, a `verified` that is no Mandate verified context, an id that is
    /// no UUID, no capability or an empty one, an approval-requiring capability that is not one of
    /// the capabilities, a negative `approval_seconds`, and an approver who is the verified
    /// subject or the verified actor.
    pub fn parse(text: &str) -> Result<Self, Refused> {
        let value =
            loom_json::parse(text).map_err(|error| refused(format!("not JSON: {error}")))?;
        let host = host(&value).map_err(|error| refused(error.to_string()))?;
        Self::from_host(host)
    }

    fn from_host(host: MandateHost) -> Result<Self, Refused> {
        let mut verified_text = String::new();
        loom_json::push_value(&mut verified_text, &host.verified);
        let verified: VerifiedContext = serde_json::from_str(&verified_text).map_err(|error| {
            refused(format!(
                "`verified` is no Mandate verified context: {error}"
            ))
        })?;
        let approver = PrincipalId::new(mandate_uuid("approver", &host.approver.0)?);
        let approver_policy =
            PolicyId::new(mandate_uuid("approver_policy", &host.approver_policy.0)?);
        if host.relation.is_empty() {
            return Err(refused("`relation` is empty"));
        }
        if host.capabilities.is_empty() {
            return Err(refused("`capabilities` names no capability"));
        }
        if host.capabilities.iter().any(String::is_empty) {
            return Err(refused("`capabilities` names an empty capability"));
        }
        if let Some(other) = host
            .approval_required
            .iter()
            .find(|capability| !host.capabilities.contains(capability))
        {
            return Err(refused(format!(
                "`approval_required` names `{other}`, which `capabilities` does not"
            )));
        }
        if host.approval_seconds < 0 {
            return Err(refused("`approval_seconds` is negative"));
        }
        if approver == verified.subject {
            return Err(refused(
                "`approver` is the verified subject: an approval is a second party's",
            ));
        }
        if verified.actor == Some(approver) {
            return Err(refused(
                "`approver` is the verified actor: an approval is a second party's",
            ));
        }
        Ok(Self {
            capabilities: host.capabilities.iter().map(Action::new).collect(),
            approval_required: host.approval_required.iter().map(Action::new).collect(),
            verified,
            approver,
            approver_policy,
            host,
        })
    }

    /// The host file as declared.
    #[must_use]
    pub const fn host(&self) -> &MandateHost {
        &self.host
    }

    /// How long an approval lasts, in seconds (`approval_seconds`).
    #[must_use]
    pub const fn approval_seconds(&self) -> i64 {
        self.host.approval_seconds
    }

    /// The commission's authority context: a `loom.authority.HostContext` holding the host file's
    /// verified context, which a resume puts in the commission it rebuilds.
    #[must_use]
    pub fn authority_context(&self) -> AuthorityContext {
        let mut verified = String::new();
        loom_json::push_value(&mut verified, &self.host.verified);
        let mut text = String::new();
        loom_json::push_text(&mut text, VERIFIED);
        let document = format!("{{{text}:{verified}}}");
        // The host's verified context was parsed as JSON, so the document is JSON.
        AuthorityContext(commission_json::parse(&document).unwrap_or(commission_json::Value::Null))
    }

    /// The Mandate approval `approve --mandate` records for `request`, under `id`: the approver's
    /// approval of the request's capability on its case, for the host file's subject and actor,
    /// recorded at `recorded_at` and expiring at `expires_at`.
    ///
    /// # Errors
    /// [`Refused`] when the capability is not one the host file names, or the case derives no
    /// Mandate resource (the empty case id).
    pub fn approval(
        &self,
        request: &HeldRequest<'_>,
        id: &MandateApprovalId,
        recorded_at: &LogTimestamp,
        expires_at: &LogTimestamp,
    ) -> Result<MandateApprovalRecorded, Refused> {
        let action = Action::new(request.capability);
        if !self.capabilities.contains(&action) {
            return Err(refused(format!(
                "the host file names no capability `{}`",
                request.capability
            )));
        }
        let resource = case_resource(request.case_id)
            .ok_or_else(|| refused("the case id is empty, so it derives no Mandate resource"))?;
        mandate_uuid("the approval id", &id.0.0)?;
        Ok(MandateApprovalRecorded {
            approval: id.clone(),
            commission_id: request.commission_id.clone(),
            action_request_id: request.action_request_id.clone(),
            organization: log_uuid(self.verified.organization.to_string()),
            subject: log_uuid(self.verified.subject.to_string()),
            actor: self.verified.actor.map(|actor| log_uuid(actor.to_string())),
            action: request.capability.to_owned(),
            resource_type: resource.resource_type.as_str().to_owned(),
            resource_id: log_uuid(resource.resource_id.to_string()),
            approver: log_uuid(self.approver.to_string()),
            recorded_at: recorded_at.clone(),
            expires_at: expires_at.clone(),
        })
    }

    /// Asks Mandate's `check_with_approval` whether approval `approval` clears `capability` for
    /// `request` of `commission`, at `at`, over every entry of `log` folded into Mandate's
    /// delegation records; `decision` is the identity Mandate's decision is taken under. Answers
    /// the `ApprovedDecisionRecorded` event to append, which consumes the approval.
    ///
    /// # Errors
    /// [`Refused`] naming why, and nothing is consumed: the commission's authority context is no
    /// Mandate context, the case derives no resource, the approval id is no UUID, a log entry does
    /// not read back as Mandate's, the record Mandate reads for the approval (found by its UUID,
    /// however the id is written, and the last one recorded) is for another commission or
    /// request, or Mandate refused it
    /// (unrecorded, of another organization, expired, naming another subject, actor, action or
    /// resource, already consumed, or a request Mandate refuses for another reason).
    #[allow(clippy::too_many_arguments)]
    pub fn clear(
        &self,
        commission: &CommissionData,
        capability: &str,
        request: &ActionRequestId,
        approval: &MandateApprovalId,
        log: &[MandateApprovalEntry],
        at: &LogTimestamp,
        decision: &str,
    ) -> Result<MandateApprovedDecision, Refused> {
        let context = verified_context(&commission.authority_context).ok_or_else(|| {
            refused("the authority context is no loom.authority.HostContext holding a Mandate verified context")
        })?;
        let resource = case_resource(&commission.case_id)
            .ok_or_else(|| refused("the case id is empty, so it derives no Mandate resource"))?;
        let approval_id = ApprovalId::new(mandate_uuid("the approval id", &approval.0.0)?);
        let records = fold(log)?;
        // The record Mandate reads: by the parsed id, never its text, and the last one recorded,
        // as Mandate's own lookup takes it, so the binding below holds for the very record
        // Mandate clears. Every recorded id parses, or `fold` refused the log.
        let recorded = log.iter().rev().find_map(|entry| match entry {
            MandateApprovalEntry::ApprovalRecorded(recorded)
                if Uuid::parse(&recorded.approval.0.0)
                    .is_ok_and(|id| ApprovalId::new(id) == approval_id) =>
            {
                Some(recorded)
            }
            _ => None,
        });
        if let Some(recorded) = recorded
            && (recorded.commission_id.0.0 != commission.commission_id.0.0
                || recorded.action_request_id != *request)
        {
            return Err(refused(format!(
                "Mandate approval {} was recorded for request {} of commission {}, not this one",
                approval.0.0, recorded.action_request_id.0.0, recorded.commission_id.0.0
            )));
        }
        let mut allocator =
            OneDecision(DecisionId::new(mandate_uuid("the decision id", decision)?));
        let action = Action::new(capability);
        let mut graph = GraphDouble::new();
        let organization = context.organization;
        let subject = AuthoritySubject::Principal(context.subject);
        graph.admit_subject(organization, subject.clone());
        graph
            .register_resource(&context, &resource, None)
            .map_err(|error| refused(format!("Mandate's graph refused the case: {error:?}")))?;
        let minimum = graph.record_grant(Grant::recorded(
            GrantId::new(derived(&resource.resource_id, 0x31)),
            organization,
            subject.clone(),
            AuthorityScope {
                actions: self.capabilities.clone(),
                resources: vec![resource.clone()],
                space: None,
            },
            self.host.relation.clone(),
        ));
        let policy = self.policy(organization, &subject, &resource);
        let tenancy = tenancy(&context)?;
        let at_mandate = Timestamp::new(at.0.clone());
        let issuer = FixedChallenge {
            expires_at: at_mandate.clone(),
            approver_policy: Some(self.approver_policy),
        };
        let pdp = Pdp {
            graph: &graph,
            policy: &policy,
            catalog: &policy,
            issuer: &issuer,
            tenancy: &tenancy,
            topology: &Topology::new(),
            agents: &records,
        };
        let check = CheckRequest {
            context: &context,
            action: &action,
            resource: &resource,
            expected_audience: &context.audience,
            relation: &self.host.relation,
            minimum: &minimum,
            attributes: &AttributeSet::default(),
            scope: None,
            at: &at_mandate,
        };
        let cleared =
            check_with_approval(&pdp, &mut allocator, &check, approval_id).map_err(|denied| {
                refused(format!(
                    "Mandate refused approval {} ({}): {:?} (decision {})",
                    approval.0.0,
                    denied.outcome.ir_name(),
                    denied.denied.reason,
                    denied.denied.decision.decision_id
                ))
            })?;
        Ok(MandateApprovedDecision {
            approval: approval.clone(),
            commission_id: CommissionId(LogUuid(commission.commission_id.0.0.clone())),
            action_request_id: request.clone(),
            context: to_log_json(serde_json::to_string(&cleared.context))?,
            decision: to_log_json(serde_json::to_string(&cleared.decision))?,
            recorded_at: at.clone(),
        })
    }

    /// The host's policy, which is also its role catalog: `relation` carries every capability,
    /// and each is allowed on the case for the subject, after an approval when it needs one.
    fn policy(
        &self,
        organization: OrganizationId,
        subject: &AuthoritySubject,
        resource: &ResourceRef,
    ) -> PolicyDouble {
        let mut policy = PolicyDouble::new();
        policy.record_policy(Policy::recorded(
            self.approver_policy,
            organization,
            PolicyVersion::new(HOST_POLICY_VERSION),
            "b10x-loom --mandate",
        ));
        policy.record_model(AuthorizationModel::recorded(
            AuthorizationModelId::new(derived(&resource.resource_id, 0x41)),
            organization,
            PolicyVersion::new(HOST_POLICY_VERSION),
            "b10x-loom --mandate",
        ));
        policy.record_role(
            organization,
            self.host.relation.clone(),
            self.capabilities.clone(),
        );
        for capability in &self.capabilities {
            if self.approval_required.contains(capability) {
                policy.require_approval(
                    organization,
                    subject.clone(),
                    capability.clone(),
                    resource.clone(),
                    ChallengeRequirement::Approval,
                );
            } else {
                policy.allow(
                    organization,
                    subject.clone(),
                    capability.clone(),
                    resource.clone(),
                );
            }
        }
        policy
    }
}

/// Decodes a `loom.authority.MandateHost`, refusing a member it does not declare.
fn host(value: &Value) -> Result<MandateHost, DecodeError> {
    const AT: &str = "mandate host";
    let members = loom_json::members_at(value, AT, "a loom.authority.MandateHost")?;
    if let Some((name, found)) = members
        .iter()
        .find(|(name, _)| !HOST_MEMBERS.contains(&name.as_str()))
    {
        return Err(DecodeError::of(
            &loom_json::nested(AT, name),
            "no member MandateHost does not declare",
            found,
        ));
    }
    let texts = |name: &str| -> Result<Vec<String>, DecodeError> {
        let at = loom_json::nested(AT, name);
        loom_json::items_at(
            loom_json::member_at(value, AT, name)?,
            &at,
            "a list of text",
        )?
        .iter()
        .map(|item| loom_json::text_at(item, &at, "text").map(str::to_owned))
        .collect()
    };
    let text = |name: &str| -> Result<String, DecodeError> {
        loom_json::text_at(
            loom_json::member_at(value, AT, name)?,
            &loom_json::nested(AT, name),
            "text",
        )
        .map(str::to_owned)
    };
    let uuid = |name: &str| -> Result<loom::primitives::Uuid, DecodeError> {
        loom_json::uuid_at(
            loom_json::member_at(value, AT, name)?,
            &loom_json::nested(AT, name),
            "a UUID",
        )
        .map(|id| loom::primitives::Uuid(id.to_owned()))
    };
    Ok(MandateHost {
        verified: loom_json::member_at(value, AT, "verified")?.clone(),
        approver: uuid("approver")?,
        approver_policy: uuid("approver_policy")?,
        relation: text("relation")?,
        capabilities: texts("capabilities")?,
        approval_required: texts("approval_required")?,
        approval_seconds: loom_json::integer_at(
            loom_json::member_at(value, AT, "approval_seconds")?,
            &loom_json::nested(AT, "approval_seconds"),
            "an integer",
        )?,
    })
}

/// Mandate's delegation records with every entry of `log` applied, in order: each recorded
/// approval as Mandate's `Approval`, and each kept `ApprovedDecisionRecorded` folded as Mandate's
/// own delegation fold applies it, consuming its approval.
///
/// # Errors
/// [`Refused`] for an entry that does not read back as Mandate's.
pub fn fold(log: &[MandateApprovalEntry]) -> Result<DelegationProjection, Refused> {
    let mut approvals = Vec::new();
    let mut events = Vec::new();
    for entry in log {
        match entry {
            MandateApprovalEntry::ApprovalRecorded(recorded) => {
                approvals.push(approval_record(recorded)?);
            }
            MandateApprovalEntry::ApprovedDecisionRecorded(decided) => {
                events.push(DelegationEvent::from(ApprovedDecisionRecorded {
                    context: serde_json::from_str::<VerifiedContext>(&log_text(&decided.context))
                        .map_err(not_mandates("context"))?,
                    decision: serde_json::from_str::<Decision>(&log_text(&decided.decision))
                        .map_err(not_mandates("decision"))?,
                    approval: ApprovalId::new(mandate_uuid(
                        "an approval id",
                        &decided.approval.0.0,
                    )?),
                }));
            }
        }
    }
    Ok(DelegationProjection::fold(
        AgentRecords {
            approvals,
            ..AgentRecords::new()
        },
        &events,
    ))
}

/// The Mandate `Approval` a recorded entry is, not yet consumed.
fn approval_record(recorded: &MandateApprovalRecorded) -> Result<Approval, Refused> {
    Ok(Approval {
        id: ApprovalId::new(mandate_uuid("an approval id", &recorded.approval.0.0)?),
        organization_id: OrganizationId::new(mandate_uuid(
            "an organization",
            &recorded.organization.0,
        )?),
        subject: PrincipalId::new(mandate_uuid("a subject", &recorded.subject.0)?),
        actor: recorded
            .actor
            .as_ref()
            .map(|actor| mandate_uuid("an actor", &actor.0).map(PrincipalId::new))
            .transpose()?,
        action: Action::new(recorded.action.clone()),
        resource: ResourceRef {
            resource_type: ResourceType::new(recorded.resource_type.clone()),
            resource_id: ResourceId::new(mandate_uuid("a resource id", &recorded.resource_id.0)?),
        },
        expires_at: Timestamp::new(recorded.expires_at.0.clone()),
        approver: PrincipalId::new(mandate_uuid("an approver", &recorded.approver.0)?),
        state: ApprovalState::Recorded,
    })
}

/// The tenancy in which the verified subject is a member of the verified organization.
fn tenancy(context: &VerifiedContext) -> Result<Tenancy, Refused> {
    let mut tenancy = Tenancy::new();
    tenancy
        .create_organization(context, context.organization, "b10x-loom --mandate")
        .map_err(|error| {
            refused(format!(
                "Mandate's tenancy refused the organization: {error:?}"
            ))
        })?;
    tenancy
        .add_organization_membership(
            context,
            MembershipAuthority::VerifiedOrganization,
            OrganizationMembershipId::new(derived_from(context.organization.as_uuid(), 0x51)),
            context.organization,
            context.subject,
        )
        .map_err(|error| refused(format!("Mandate's tenancy refused the subject: {error:?}")))?;
    Ok(tenancy)
}

/// Mandate's `Uuid` of the text `what` names.
fn mandate_uuid(what: &str, text: &str) -> Result<Uuid, Refused> {
    Uuid::parse(text).map_err(|_| refused(format!("{what} `{text}` is no UUID")))
}

fn log_uuid(text: String) -> LogUuid {
    LogUuid(text)
}

/// A fixed identity derived from `from`, for the one record of its kind a decision builds.
fn derived(from: &ResourceId, kind: u8) -> Uuid {
    derived_from(from.as_uuid(), kind)
}

fn derived_from(from: &Uuid, kind: u8) -> Uuid {
    let mut bytes = *from.as_bytes();
    bytes[0] ^= kind;
    Uuid::from_bytes(bytes)
}

/// Mandate's record, serialized as Mandate writes it (`serialized`), as the log's JSON.
fn to_log_json(serialized: serde_json::Result<String>) -> Result<intake_json::Value, Refused> {
    let text = serialized
        .map_err(|error| refused(format!("Mandate's record does not serialize: {error}")))?;
    intake_json::parse(&text)
        .map_err(|error| refused(format!("Mandate's record is not JSON: {error}")))
}

/// The text of a log entry's JSON member, for Mandate to read back.
fn log_text(value: &intake_json::Value) -> String {
    let mut text = String::new();
    intake_json::push_value(&mut text, value);
    text
}

/// Why a log entry's member `what` is not Mandate's.
fn not_mandates(what: &str) -> impl FnOnce(serde_json::Error) -> Refused + '_ {
    move |error| refused(format!("a log entry's `{what}` is not Mandate's: {error}"))
}

/// The one decision identity a `check_with_approval` is taken under.
struct OneDecision(DecisionId);

impl DecisionIdAllocator for OneDecision {
    fn next_decision_id(&mut self) -> DecisionId {
        self.0
    }
}
