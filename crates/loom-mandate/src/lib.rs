#![forbid(unsafe_code)]

//! Commission's authority port answered by Mandate (`story:mandate-authority`).
//!
//! [`MandateAuthority`] implements Commission's [`AuthorityProvider`]: for each decision it asks
//! Mandate's `mandate_authz::check` and maps the answer to Commission's verdict, as the
//! `loom.authority` domain declares (`ess/domains/authority.yaml`, `loom::authority::Outcome`
//! and its `answer` attribute). This is the one crate that links Mandate; Commission, the governor
//! and the command line do not; the command line uses it only through [`approval`], behind
//! `--mandate`.
//!
//! What Mandate is asked:
//!
//! | Mandate's input | Where it comes from |
//! |---|---|
//! | the verified context (subject, organization, audience, credential) | the commission's authority context, a `loom.authority.HostContext` the host filled, never the model |
//! | the action | the capability Commission asks about |
//! | the resource | the case: resource type [`CASE_RESOURCE_TYPE`], the resource id [`case_resource_id`] derives from the commission's case id |
//! | the expected audience, the relation, the minimum revision, the attributes | the host, through the builder |
//! | the instant | the host's clock, through the builder |
//! | the graph, policy, role catalog, challenge issuer, delegation records, decision ids, tenancy and topology | the host, through the builder; this crate configures no policy of its own |
//!
//! How each answer maps (`loom.authority.Outcome`):
//!
//! | Mandate answers | Commission is answered |
//! |---|---|
//! | an allowed decision | `Allow` |
//! | `ApprovalRequired` with its challenge | `ApprovalRequired`, the request naming Mandate's decision id |
//! | `Denied`, `InvalidCredential`, `TenantMismatch`, `AudienceMismatch`, `StaleEpoch` | `Deny`, naming the reason and the decision id |
//! | `Unavailable` (a port that cannot answer) | an `Err`: Commission refuses, never allows |
//!
//! Two failures never reach Mandate and are an `Err` too: an authority context that is no
//! `HostContext` holding a Mandate verified context (`NoMandateContext`), and the empty case id,
//! which derives no Mandate resource id (`NoMandateResource`). A decision id allocator that
//! panicked in an earlier decision is `Unavailable`, and Mandate is not asked. A Mandate answer
//! that contradicts itself (an allow whose decision is not allowed, an approval requirement
//! without a challenge) is `Contradictory`, an `Err` as well. Every failure fails toward less
//! authority (`AGENTS.md` § Rules).
//!
//! [`approval`] is the second use (`story:mandate-approval-on-resume`): Mandate configured from the
//! host file `b10x-loom approve --mandate` and `resume --mandate` name, recording an approval of
//! the request a commission's Suspension holds and clearing it once through Mandate's
//! `check_with_approval`.

pub mod approval;

use std::fmt;
use std::sync::Mutex;

use b10x_loom_commission::model::json as commission_json;
use b10x_loom_commission::model::responsibility::{
    AuthorityContext, AuthorityVerdict, AuthorityVerdictApprovalRequired, AuthorityVerdictDeny,
    CaseId, CommissionData, Unit,
};
use b10x_loom_commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom::authority::{Answer, HostContext, Outcome};
use loom::json::{self as loom_json, DecodeError, Value};
use mandate_authz::agent::AgentAuthorityRead;
use mandate_authz::decision::{ChallengeIssuer, DecisionIdAllocator, Denied};
use mandate_authz::{CheckRequest, Pdp, check};
use mandate_graph::port::GraphRead;
use mandate_graph::topology::ResourceLookup;
use mandate_model::Decision;
use mandate_model::graph::Topology;
use mandate_model::tenancy::Tenancy;
use mandate_policy::port::{AttributeSet, PolicyEvaluator};
use mandate_policy::precedence::RoleCatalog;
use mandate_types::{
    Action, Audience, AuthzRevision, DecisionReason, DenialReason, ResourceId, ResourceRef,
    ResourceType, Timestamp, Uuid, VerifiedContext,
};
use sha1::{Digest, Sha1};

/// The Mandate resource type a case is: every decision is about the commission's case.
pub const CASE_RESOURCE_TYPE: &str = "loom.case";

/// The namespace a case's Mandate resource id is derived under,
/// `735b2acd-620b-43ec-b1ab-144c2aec7ba7`: the `namespace` attribute of
/// `loom.authority.CaseResource` (`ess/domains/authority.yaml`).
/// It never changes; changing it moves every case to another resource.
pub const CASE_RESOURCE_NAMESPACE: Uuid = Uuid::from_bytes([
    0x73, 0x5b, 0x2a, 0xcd, 0x62, 0x0b, 0x43, 0xec, 0xb1, 0xab, 0x14, 0x4c, 0x2a, 0xec, 0x7b, 0xa7,
]);

/// The member of a `loom.authority.HostContext` that holds Mandate's verified context.
const VERIFIED: &str = "verified";

/// Mandate's relationship graph as a decision reads it: the authority read and where a resource
/// sits. Implemented for every type that is both.
pub trait MandateGraph: GraphRead<Revision = AuthzRevision> + ResourceLookup {}

impl<T> MandateGraph for T where T: GraphRead<Revision = AuthzRevision> + ResourceLookup + ?Sized {}

/// The host's trusted clock: the instant a decision is taken at, never a model's.
pub type Clock<'a> = Box<dyn Fn() -> Timestamp + 'a>;

/// Commission's [`AuthorityProvider`] over Mandate's `check`. Built with
/// [`MandateAuthority::builder`]; see the crate documentation for what it asks and answers.
pub struct MandateAuthority<'a> {
    graph: Box<dyn MandateGraph + 'a>,
    policy: Box<dyn PolicyEvaluator + 'a>,
    catalog: Box<dyn RoleCatalog + 'a>,
    issuer: Box<dyn ChallengeIssuer + 'a>,
    agents: Box<dyn AgentAuthorityRead + 'a>,
    allocator: Mutex<Box<dyn DecisionIdAllocator + 'a>>,
    tenancy: Tenancy,
    topology: Topology,
    audience: Audience,
    relation: String,
    minimum: AuthzRevision,
    attributes: AttributeSet,
    clock: Clock<'a>,
}

impl fmt::Debug for MandateAuthority<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MandateAuthority")
            .field("audience", &self.audience)
            .field("relation", &self.relation)
            .field("minimum", &self.minimum)
            .finish_non_exhaustive()
    }
}

impl<'a> MandateAuthority<'a> {
    /// A builder holding nothing yet. Every port and value is required except the attributes,
    /// which default to none.
    #[must_use]
    pub fn builder() -> MandateAuthorityBuilder<'a> {
        MandateAuthorityBuilder::default()
    }
}

/// What [`MandateAuthorityBuilder::build`] was not given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingPart(pub &'static str);

impl fmt::Display for MissingPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MandateAuthority needs its {}", self.0)
    }
}

impl std::error::Error for MissingPart {}

/// Collects Mandate's ports and the host's values for a [`MandateAuthority`].
#[derive(Default)]
pub struct MandateAuthorityBuilder<'a> {
    graph: Option<Box<dyn MandateGraph + 'a>>,
    policy: Option<Box<dyn PolicyEvaluator + 'a>>,
    catalog: Option<Box<dyn RoleCatalog + 'a>>,
    issuer: Option<Box<dyn ChallengeIssuer + 'a>>,
    agents: Option<Box<dyn AgentAuthorityRead + 'a>>,
    allocator: Option<Box<dyn DecisionIdAllocator + 'a>>,
    tenancy: Option<Tenancy>,
    topology: Option<Topology>,
    audience: Option<Audience>,
    relation: Option<String>,
    minimum: Option<AuthzRevision>,
    attributes: Option<AttributeSet>,
    clock: Option<Clock<'a>>,
}

impl fmt::Debug for MandateAuthorityBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MandateAuthorityBuilder")
            .field("audience", &self.audience)
            .field("relation", &self.relation)
            .field("minimum", &self.minimum)
            .finish_non_exhaustive()
    }
}

impl<'a> MandateAuthorityBuilder<'a> {
    /// The relationship graph: the authority read, and where a resource sits.
    #[must_use]
    pub fn graph(mut self, graph: impl MandateGraph + 'a) -> Self {
        self.graph = Some(Box::new(graph));
        self
    }

    /// The policy evaluator.
    #[must_use]
    pub fn policy(mut self, policy: impl PolicyEvaluator + 'a) -> Self {
        self.policy = Some(Box::new(policy));
        self
    }

    /// What actions a role name carries.
    #[must_use]
    pub fn catalog(mut self, catalog: impl RoleCatalog + 'a) -> Self {
        self.catalog = Some(Box::new(catalog));
        self
    }

    /// What an approval-required decision needs and no port answers: its deadline and approver.
    #[must_use]
    pub fn issuer(mut self, issuer: impl ChallengeIssuer + 'a) -> Self {
        self.issuer = Some(Box::new(issuer));
        self
    }

    /// The `mandate.delegation` records a delegated request is bounded by.
    #[must_use]
    pub fn agents(mut self, agents: impl AgentAuthorityRead + 'a) -> Self {
        self.agents = Some(Box::new(agents));
        self
    }

    /// The identities decisions are recorded under.
    #[must_use]
    pub fn allocator(mut self, allocator: impl DecisionIdAllocator + 'a) -> Self {
        self.allocator = Some(Box::new(allocator));
        self
    }

    /// The tenancy projection: which organizations admit authority, and who is a member.
    #[must_use]
    pub fn tenancy(mut self, tenancy: Tenancy) -> Self {
        self.tenancy = Some(tenancy);
        self
    }

    /// The resource projection, read for a resource's space.
    #[must_use]
    pub fn topology(mut self, topology: Topology) -> Self {
        self.topology = Some(topology);
        self
    }

    /// The audience a verified context must have been issued for.
    #[must_use]
    pub fn audience(mut self, audience: Audience) -> Self {
        self.audience = Some(audience);
        self
    }

    /// The relation or role name the graph is asked about.
    #[must_use]
    pub fn relation(mut self, relation: impl Into<String>) -> Self {
        self.relation = Some(relation.into());
        self
    }

    /// The revision a graph read must not answer below.
    #[must_use]
    pub fn minimum_revision(mut self, minimum: AuthzRevision) -> Self {
        self.minimum = Some(minimum);
        self
    }

    /// The attribute input policy evaluates against; none when not given.
    #[must_use]
    pub fn attributes(mut self, attributes: AttributeSet) -> Self {
        self.attributes = Some(attributes);
        self
    }

    /// The host's trusted clock.
    #[must_use]
    pub fn clock(mut self, clock: impl Fn() -> Timestamp + 'a) -> Self {
        self.clock = Some(Box::new(clock));
        self
    }

    /// The provider, once every port and value is given.
    ///
    /// # Errors
    ///
    /// [`MissingPart`] naming the first part not given, in the order the builder lists them.
    pub fn build(self) -> Result<MandateAuthority<'a>, MissingPart> {
        Ok(MandateAuthority {
            graph: self.graph.ok_or(MissingPart("graph"))?,
            policy: self.policy.ok_or(MissingPart("policy"))?,
            catalog: self.catalog.ok_or(MissingPart("catalog"))?,
            issuer: self.issuer.ok_or(MissingPart("issuer"))?,
            agents: self.agents.ok_or(MissingPart("agents"))?,
            allocator: Mutex::new(self.allocator.ok_or(MissingPart("allocator"))?),
            tenancy: self.tenancy.ok_or(MissingPart("tenancy"))?,
            topology: self.topology.ok_or(MissingPart("topology"))?,
            audience: self.audience.ok_or(MissingPart("audience"))?,
            relation: self.relation.ok_or(MissingPart("relation"))?,
            minimum: self.minimum.ok_or(MissingPart("minimum revision"))?,
            attributes: self.attributes.unwrap_or_default(),
            clock: self.clock.ok_or(MissingPart("clock"))?,
        })
    }
}

/// The answer `loom.authority` declares for each outcome (`ess/domains/authority.yaml`, the
/// `answer` attribute of `loom.authority.Outcome`). The synthesized model carries no accessor for
/// an enum attribute, so the table is written here once; `tests/mandate_authority.rs` holds it to
/// the specification, variant by variant.
#[must_use]
pub const fn answer(outcome: Outcome) -> Answer {
    match outcome {
        Outcome::Allowed => Answer::Allow,
        Outcome::ApprovalRequired => Answer::ApprovalRequired,
        Outcome::Denied
        | Outcome::InvalidCredential
        | Outcome::TenantMismatch
        | Outcome::AudienceMismatch
        | Outcome::StaleEpoch => Answer::Deny,
        Outcome::Unavailable
        | Outcome::NoMandateContext
        | Outcome::NoMandateResource
        | Outcome::Contradictory => Answer::Undecided,
    }
}

/// The `loom.authority.Outcome` a Mandate answer is.
///
/// An allow is `Allowed` only when its decision is allowed for the reason `Allowed`; an
/// approval requirement is `ApprovalRequired` only when its decision carries the challenge that
/// would clear it; a refusal whose decision says it allowed is no refusal Mandate makes. Each of
/// those is `Contradictory`.
#[must_use]
pub fn outcome(answer: &Result<Decision, Denied>) -> Outcome {
    match answer {
        Ok(decision) if decision.allowed && decision.reason == DecisionReason::Allowed => {
            Outcome::Allowed
        }
        Ok(_) => Outcome::Contradictory,
        Err(denied) if denied.decision.allowed => Outcome::Contradictory,
        Err(denied) => match denied.reason {
            DenialReason::ApprovalRequired if denied.decision.challenge.is_some() => {
                Outcome::ApprovalRequired
            }
            DenialReason::ApprovalRequired => Outcome::Contradictory,
            DenialReason::Denied => Outcome::Denied,
            DenialReason::InvalidCredential => Outcome::InvalidCredential,
            DenialReason::TenantMismatch => Outcome::TenantMismatch,
            DenialReason::AudienceMismatch => Outcome::AudienceMismatch,
            DenialReason::StaleEpoch => Outcome::StaleEpoch,
            DenialReason::Unavailable => Outcome::Unavailable,
        },
    }
}

/// Mandate's verified context from a commission's authority context: a `loom.authority.HostContext`
/// with exactly the member `verified`, holding a `mandate.core.VerifiedContext` as Mandate writes
/// it. `None` for anything else.
#[must_use]
pub fn verified_context(context: &AuthorityContext) -> Option<VerifiedContext> {
    let mut text = String::new();
    commission_json::push_value(&mut text, &context.0);
    let value = loom_json::parse(&text).ok()?;
    let host = host_context(&value).ok()?;
    let mut verified = String::new();
    loom_json::push_value(&mut verified, &host.verified);
    serde_json::from_str(&verified).ok()
}

/// Decodes a `loom.authority.HostContext`, refusing a member it does not declare.
fn host_context(value: &Value) -> Result<HostContext, DecodeError> {
    const AT: &str = "authority_context";
    let members = loom_json::members_at(value, AT, "a loom.authority.HostContext")?;
    if let Some((name, found)) = members.iter().find(|(name, _)| name != VERIFIED) {
        return Err(DecodeError::of(
            &loom_json::nested(AT, name),
            "no member HostContext does not declare",
            found,
        ));
    }
    if members.iter().filter(|(name, _)| name == VERIFIED).count() != 1 {
        return Err(DecodeError::of(AT, "one `verified` member", value));
    }
    Ok(HostContext {
        verified: loom_json::member_at(value, AT, VERIFIED)?.clone(),
    })
}

/// The Mandate resource id of a case, as `loom.authority.CaseResource` declares: the UUID version
/// 5 (RFC 9562 § 5.5, SHA-1) of the case id's exact UTF-8 bytes under
/// [`CASE_RESOURCE_NAMESPACE`]. The case id is never parsed, so two distinct case ids (two
/// spellings of one UUID included) are two resources. A host computes the same id with this
/// function, or with any UUID version 5 implementation, when it writes Mandate's graph or policy.
/// `None` only for the empty case id, which names no case.
#[must_use]
pub fn case_resource_id(case: &CaseId) -> Option<ResourceId> {
    if case.0.is_empty() {
        return None;
    }
    let mut hasher = Sha1::new();
    hasher.update(CASE_RESOURCE_NAMESPACE.as_bytes());
    hasher.update(case.0.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // The version (5) in the high nibble of octet 6, the RFC 9562 variant in octet 8.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Some(ResourceId::new(Uuid::from_bytes(bytes)))
}

/// The Mandate resource a case is: [`CASE_RESOURCE_TYPE`] and the id [`case_resource_id`]
/// derives. `None` only for the empty case id.
#[must_use]
pub fn case_resource(case: &CaseId) -> Option<ResourceRef> {
    Some(ResourceRef {
        resource_type: ResourceType::new(CASE_RESOURCE_TYPE),
        resource_id: case_resource_id(case)?,
    })
}

/// One decision: its outcome, the Mandate decision it was, when one was taken, and what is said
/// about it otherwise.
struct Decided {
    outcome: Outcome,
    decision: Option<String>,
    detail: String,
}

impl Decided {
    /// An outcome no Mandate decision was taken for.
    fn undecided(outcome: Outcome, detail: &str) -> Self {
        Self {
            outcome,
            decision: None,
            detail: detail.to_owned(),
        }
    }
}

impl MandateAuthority<'_> {
    /// Asks Mandate about `capability` on the commission's case, as the commission's verified
    /// context. Nothing is asked when the context or the case is not Mandate's.
    fn ask(&self, commission: &CommissionData, capability: &str) -> Decided {
        let Some(context) = verified_context(&commission.authority_context) else {
            return Decided::undecided(
                Outcome::NoMandateContext,
                "the authority context is no loom.authority.HostContext holding a Mandate verified context",
            );
        };
        let Some(resource) = case_resource(&commission.case_id) else {
            return Decided::undecided(
                Outcome::NoMandateResource,
                "the case id is empty, so it derives no Mandate resource id",
            );
        };
        let Ok(mut allocator) = self.allocator.lock() else {
            return Decided::undecided(
                Outcome::Unavailable,
                "the decision id allocator panicked in an earlier decision",
            );
        };
        let action = Action::new(capability);
        let at = (self.clock)();
        let pdp = Pdp {
            graph: &*self.graph,
            policy: &*self.policy,
            catalog: &*self.catalog,
            issuer: &*self.issuer,
            tenancy: &self.tenancy,
            topology: &self.topology,
            agents: &*self.agents,
        };
        let request = CheckRequest {
            context: &context,
            action: &action,
            resource: &resource,
            expected_audience: &self.audience,
            relation: &self.relation,
            minimum: &self.minimum,
            attributes: &self.attributes,
            scope: None,
            at: &at,
        };
        let answered = check(&pdp, &mut **allocator, &request);
        let decision = match &answered {
            Ok(decision) => decision.decision_id.to_string(),
            Err(denied) => denied.decision.decision_id.to_string(),
        };
        Decided {
            outcome: outcome(&answered),
            decision: Some(decision),
            detail: String::new(),
        }
    }
}

impl AuthorityProvider for MandateAuthority<'_> {
    fn decide(
        &self,
        commission: &CommissionData,
        capability: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        let decided = self.ask(commission, capability);
        match (answer(decided.outcome), decided.decision) {
            (Answer::Allow, Some(_)) => Ok(AuthorityVerdict::Allow(Unit(true))),
            (Answer::ApprovalRequired, Some(decision)) => Ok(AuthorityVerdict::ApprovalRequired(
                AuthorityVerdictApprovalRequired { request: decision },
            )),
            (Answer::Deny, Some(decision)) => Ok(AuthorityVerdict::Deny(AuthorityVerdictDeny {
                reason: format!(
                    "Mandate refused {capability}: {:?} (decision {decision})",
                    decided.outcome
                ),
            })),
            // An undecided outcome, or one no Mandate decision was taken for: a failure to
            // decide, which Commission refuses.
            (_, decision) => Err(AuthorityProviderError::new(match decision {
                Some(decision) => format!(
                    "Mandate did not decide {capability}: {:?} (decision {decision})",
                    decided.outcome
                ),
                None => format!(
                    "Mandate was not asked about {capability}: {:?}: {}",
                    decided.outcome, decided.detail
                ),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{case_resource, case_resource_id};
    use b10x_loom_commission::model::responsibility::CaseId;

    #[test]
    fn only_the_empty_case_id_is_no_mandate_resource() {
        assert_eq!(case_resource(&CaseId(String::new())), None);
        assert_eq!(case_resource_id(&CaseId(String::new())), None);
        for case in ["case-1", "CHG-1842", "00000000-0000-4000-8000-000000000001"] {
            let resource = case_resource(&CaseId(case.to_owned()))
                .unwrap_or_else(|| panic!("{case:?} is a resource"));
            assert_eq!(resource.resource_type.as_str(), "loom.case");
            assert_eq!(
                Some(resource.resource_id),
                case_resource_id(&CaseId(case.to_owned()))
            );
        }
    }
}
