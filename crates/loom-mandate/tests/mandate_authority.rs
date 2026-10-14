//! Acceptance for `story:mandate-authority`: `MandateAuthority` answers Commission's authority port
//! with Mandate's `check`, mapped as `loom.authority` declares (`ess/domains/authority.yaml`), and
//! every failure grants nothing.
//!
//! Mandate's ports are its own published doubles (`GraphDouble`, `PolicyDouble`, `AgentRecords`,
//! `FixedChallenge`, `SequentialDecisionIds`) and two local policies: one that refuses for a reason
//! it is given, and one that counts what it was asked. The run through `run_with_suspension` uses
//! the Commission testkit's fakes and the governor's `FileSuspensionStore` under
//! `CARGO_TARGET_TMPDIR`. Ids are synthetic.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp as CommissionTimestamp, Uuid as Id};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, RunOutcome, SuspensionReason, SuspensionState, Unit,
    commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityCheck, AuthorityProvider, check_authority};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::ports::suspension::SuspensionPort;
use b10x_loom_commission::runtime::{LoopContext, run_with_suspension};
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer as Scripted, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;
use loom::authority::{Answer, Outcome};
use loom_mandate::{CASE_RESOURCE_TYPE, MandateAuthority, answer};
use mandate_authz::agent::AgentRecords;
use mandate_authz::decision::{DecisionIdAllocator, FixedChallenge, SequentialDecisionIds};
use mandate_graph::double::GraphDouble;
use mandate_graph::record::Grant;
use mandate_graph::topology::ResourceRegistry;
use mandate_model::graph::Topology;
use mandate_model::tenancy::{MembershipAuthority, Tenancy};
use mandate_policy::double::PolicyDouble;
use mandate_policy::port::{
    ChallengeRequirement, Evaluation, PolicyError, PolicyEvaluator, PolicyRequest, Unanswered,
};
use mandate_policy::record::{AuthorizationModel, Policy};
use mandate_types::{
    Action, Audience, AuthorityScope, AuthoritySubject, AuthorizationModelId, AuthzRevision,
    CorrelationId, CredentialId, DenialReason, GrantId, OrganizationId, OrganizationMembershipId,
    PolicyId, PolicyVersion, PrincipalId as MandatePrincipal, ResourceId, ResourceRef,
    ResourceType, Timestamp, Uuid, VerifiedContext,
};

const CAPABILITY: &str = "repository.merge";
const RELATION: &str = "maintainer";
const AUDIENCE: &str = "loom";
const CASE: &str = "0c5e0000-0000-4000-8000-00000000c45e";

fn uuid(byte: u8) -> Uuid {
    Uuid::from_bytes([byte; 16])
}

fn organization() -> OrganizationId {
    OrganizationId::new(uuid(1))
}

fn principal() -> MandatePrincipal {
    MandatePrincipal::new(uuid(2))
}

fn subject() -> AuthoritySubject {
    AuthoritySubject::Principal(principal())
}

fn action() -> Action {
    Action::new(CAPABILITY)
}

fn resource() -> ResourceRef {
    ResourceRef {
        resource_type: ResourceType::new(CASE_RESOURCE_TYPE),
        resource_id: ResourceId::new(
            Uuid::parse(CASE).unwrap_or_else(|_| panic!("CASE is a UUID")),
        ),
    }
}

fn approver() -> PolicyId {
    PolicyId::new(uuid(40))
}

fn subject_context() -> VerifiedContext {
    VerifiedContext {
        subject: principal(),
        actor: None,
        organization: organization(),
        audience: Audience::new(AUDIENCE),
        credential: CredentialId::new(uuid(7)),
        delegation: None,
        execution: None,
        correlation: CorrelationId::new("correlation-u2"),
    }
}

fn tenancy() -> Tenancy {
    let mut tenancy = Tenancy::new();
    tenancy
        .create_organization(&subject_context(), organization(), "acme")
        .unwrap_or_else(|error| panic!("the organization is recorded: {error:?}"));
    tenancy
        .add_organization_membership(
            &subject_context(),
            MembershipAuthority::VerifiedOrganization,
            OrganizationMembershipId::new(uuid(3)),
            organization(),
            principal(),
        )
        .unwrap_or_else(|error| panic!("the subject is a member: {error:?}"));
    tenancy
}

/// A graph in which the subject holds `maintainer` on the case.
fn granting_graph() -> (GraphDouble, AuthzRevision) {
    let mut graph = GraphDouble::new();
    graph.admit_subject(organization(), subject());
    graph
        .register_resource(&subject_context(), &resource(), None)
        .unwrap_or_else(|error| panic!("the case is registered: {error:?}"));
    let revision = graph.record_grant(Grant::recorded(
        GrantId::new(uuid(30)),
        organization(),
        subject(),
        AuthorityScope {
            actions: vec![action()],
            resources: vec![resource()],
            space: None,
        },
        RELATION,
    ));
    (graph, revision)
}

/// A policy that can answer, and a catalog in which `maintainer` carries the capability.
fn policy() -> PolicyDouble {
    let mut policy = PolicyDouble::new();
    policy.record_policy(Policy::recorded(
        approver(),
        organization(),
        PolicyVersion::new("v1"),
        "source",
    ));
    policy.record_model(AuthorizationModel::recorded(
        AuthorizationModelId::new(uuid(41)),
        organization(),
        PolicyVersion::new("v1"),
        "schema",
    ));
    policy.record_role(organization(), RELATION, vec![action()]);
    policy
}

/// The provider over `graph`, `policy` and the role catalog of [`policy`], issuing approvals.
fn authority<'a>(
    graph: GraphDouble,
    revision: AuthzRevision,
    policy: impl PolicyEvaluator + 'a,
) -> MandateAuthority<'a> {
    MandateAuthority::builder()
        .graph(graph)
        .policy(policy)
        .catalog(self::policy())
        .issuer(FixedChallenge {
            expires_at: Timestamp::new("2026-10-15T00:00:00Z"),
            approver_policy: Some(approver()),
        })
        .agents(AgentRecords::new())
        .allocator(SequentialDecisionIds::new())
        .tenancy(tenancy())
        .topology(Topology::new())
        .audience(Audience::new(AUDIENCE))
        .relation(RELATION)
        .minimum_revision(revision)
        .clock(|| Timestamp::new("2026-10-14T09:00:00Z"))
        .build()
        .unwrap_or_else(|missing| panic!("{missing}"))
}

fn id(n: u64) -> Id {
    Id(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// The authority context a host fills: a `loom.authority.HostContext` holding `context`.
fn host_context(context: &VerifiedContext) -> AuthorityContext {
    let verified = serde_json::to_string(context)
        .unwrap_or_else(|error| panic!("the verified context serializes: {error}"));
    context_from(&format!(r#"{{"verified":{verified}}}"#))
}

fn context_from(text: &str) -> AuthorityContext {
    AuthorityContext(json::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}")))
}

fn commission_with(
    case: &str,
    context: AuthorityContext,
) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(id(0x81)),
        agent_revision_id: AgentRevisionId(id(0x82)),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-u2".to_owned()),
        authority_context: context,
    })
}

fn commission() -> Commission<commission_state::Assigned> {
    commission_with(CASE, host_context(&subject_context()))
}

/// Refuses every request for the reason it was given, as a policy deciding against the caller.
struct Refusing(DenialReason);

impl PolicyEvaluator for Refusing {
    fn evaluate(&self, _request: &PolicyRequest<'_>) -> Result<Evaluation, PolicyError> {
        Err(PolicyError::Denied(self.0))
    }
}

/// Counts what it was asked, and answers as `inner` does.
struct Counting<'a> {
    asked: &'a Cell<usize>,
    inner: PolicyDouble,
}

impl PolicyEvaluator for Counting<'_> {
    fn evaluate(&self, request: &PolicyRequest<'_>) -> Result<Evaluation, PolicyError> {
        self.asked.set(self.asked.get() + 1);
        self.inner.evaluate(request)
    }
}

fn allowing() -> PolicyDouble {
    let mut policy = policy();
    policy.allow(organization(), subject(), action(), resource());
    policy
}

#[test]
fn a_request_mandate_allows_is_granted() {
    let (graph, revision) = granting_graph();
    let provider = authority(graph, revision, allowing());
    let commission = commission();

    assert_eq!(
        provider.decide(commission.data(), CAPABILITY),
        Ok(AuthorityVerdict::Allow(Unit(true)))
    );
    assert!(check_authority(&provider, &commission, CAPABILITY).allows());
}

#[test]
fn a_request_mandate_denies_is_denied() {
    let commission = commission();
    // Every reason Mandate refuses for that is a decision against the caller: the approval
    // requirement and `Unavailable` are the cases below and their own tests.
    let reasons: Vec<DenialReason> = DenialReason::VARIANTS
        .iter()
        .copied()
        .filter(|reason| {
            !matches!(
                reason,
                DenialReason::ApprovalRequired | DenialReason::Unavailable
            )
        })
        .collect();
    assert_eq!(reasons.len(), 5, "Mandate's decisions against the caller");
    for reason in reasons {
        let (graph, revision) = granting_graph();
        let provider = authority(graph, revision, Refusing(reason));
        match provider.decide(commission.data(), CAPABILITY) {
            Ok(AuthorityVerdict::Deny(deny)) => assert!(
                deny.reason.contains(&format!("{reason:?}")),
                "{reason:?}: the denial names Mandate's reason: {}",
                deny.reason
            ),
            other => panic!("{reason:?}: a Mandate refusal is a denial, got {other:?}"),
        }
        assert!(!check_authority(&provider, &commission, CAPABILITY).allows());
    }

    // Policy's own deny, and a credential issued for another audience, reach Mandate's decision
    // by its own path and are denials too.
    let (graph, revision) = granting_graph();
    let mut denying = policy();
    denying.deny(organization(), subject(), action(), resource());
    let provider = authority(graph, revision, denying);
    assert!(matches!(
        provider.decide(commission.data(), CAPABILITY),
        Ok(AuthorityVerdict::Deny(_))
    ));

    let (graph, revision) = granting_graph();
    let provider = authority(graph, revision, allowing());
    let other_audience = VerifiedContext {
        audience: Audience::new("another-api"),
        ..subject_context()
    };
    match provider.decide(
        commission_with(CASE, host_context(&other_audience)).data(),
        CAPABILITY,
    ) {
        Ok(AuthorityVerdict::Deny(deny)) => assert!(
            deny.reason.contains("AudienceMismatch"),
            "the denial names the mismatch: {}",
            deny.reason
        ),
        other => panic!("another audience is denied, got {other:?}"),
    }
}

#[derive(Debug, Default)]
struct Context {
    requests: u64,
    observations: u64,
}

impl LoopContext for Context {
    fn action_request_id(&mut self) -> ActionRequestId {
        self.requests += 1;
        ActionRequestId(id(0x300 + self.requests))
    }

    fn observation_id(&mut self) -> ObservationId {
        self.observations += 1;
        ObservationId(id(0x400 + self.observations))
    }

    fn now(&mut self) -> CommissionTimestamp {
        CommissionTimestamp("2026-10-14T09:00:00Z".to_owned())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(4)
    }
}

/// Performs every action, and counts what it was handed.
#[derive(Default)]
struct Effects {
    invoked: Cell<usize>,
}

impl EffectPort for Effects {
    fn performs(&self, _action: &str) -> bool {
        true
    }

    fn invoke(
        &self,
        _commission: &Commission<commission_state::Assigned>,
        _request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        self.invoked.set(self.invoked.get() + 1);
        Err(EffectError::new("no effect is performed in this test"))
    }
}

fn store_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("mandate-authority")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    }
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    dir
}

#[test]
fn an_approval_required_decision_suspends_as_the_gate_does() {
    let (graph, revision) = granting_graph();
    let mut requiring = policy();
    requiring.require_approval(
        organization(),
        subject(),
        action(),
        resource(),
        ChallengeRequirement::Approval,
    );
    let provider = authority(graph, revision, requiring);
    let commission = commission();

    // The decision the provider reports is Mandate's first decision, by its identity.
    let first = SequentialDecisionIds::new().next_decision_id().to_string();
    match provider.decide(commission.data(), CAPABILITY) {
        Ok(AuthorityVerdict::ApprovalRequired(required)) => assert_eq!(required.request, first),
        other => panic!("an approval requirement is Commission's, got {other:?}"),
    }

    // Through the runtime: the frontier gates `merge` behind the capability, the executor
    // proposes it, Mandate requires an approval, and the Run is suspended with its Suspension held.
    let case = CaseId(CASE.to_owned());
    let governor = FakeGovernor::new();
    governor.script(
        case.clone(),
        [Scripted::at(3).with_items(
            Vec::new(),
            Vec::new(),
            vec![FrontierAction {
                action: "merge".to_owned(),
                status: ActionStatus::ApprovalRequired,
                capability: Some(CAPABILITY.to_owned()),
                reasons: Vec::new(),
            }],
        )],
    );
    let executor = ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: "merge".to_owned(),
            arguments: ProposedActionArguments(Value::Null),
        },
    )]);
    let effects = Effects::default();
    let suspensions = FileSuspensionStore::open(store_dir("approval"))
        .unwrap_or_else(|error| panic!("the suspension store opens: {error:?}"));
    let mut issued = 0u64;
    let mut runs = Generated::new(RunStore::new(move || {
        issued += 1;
        RunId(id(0x100 + issued))
    }));

    let end = run_with_suspension(
        &governor,
        &executor,
        &provider,
        &effects,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::default(),
    )
    .unwrap_or_else(|error| panic!("the loop failed: {error:?}"));

    let second = {
        let mut ids = SequentialDecisionIds::new();
        ids.next_decision_id();
        ids.next_decision_id().to_string()
    };
    match &end.outcome {
        RunOutcome::NeedsAuthority(needs) => assert_eq!(needs.request, second),
        other => panic!("the run ends needing authority, got {other:?}"),
    }
    assert_eq!(effects.invoked.get(), 0, "nothing was performed");
    let held = suspensions
        .get(&commission.data().commission_id)
        .unwrap_or_else(|error| panic!("the suspension store reads: {error:?}"))
        .unwrap_or_else(|| panic!("the run holds a Suspension"));
    assert_eq!(held.state, SuspensionState::Pending);
    assert_eq!(held.data.action, "merge");
    match &held.data.reason {
        SuspensionReason::Authority(reason) => {
            assert_eq!(
                reason.member("capability"),
                Some(&Value::Text(CAPABILITY.to_owned()))
            );
            assert_eq!(reason.member("request"), Some(&Value::Text(second)));
        }
        other => panic!("the Suspension is for authority, got {other:?}"),
    }
}

#[test]
fn an_unavailable_mandate_grants_nothing() {
    let commission = commission();

    // A policy that cannot be reached, for each reason policy cannot answer, and one that has no
    // policy at all: Mandate answers `Unavailable`, and Commission refuses.
    let mut policies: Vec<Box<dyn PolicyEvaluator>> = vec![Box::new(PolicyDouble::new())];
    for unanswered in Unanswered::ALL {
        policies.push(Box::new(CouldNotAnswer(*unanswered)));
    }
    for policy in policies {
        let (graph, revision) = granting_graph();
        let provider = authority(graph, revision, BoxedPolicy(policy));
        let decided = provider.decide(commission.data(), CAPABILITY);
        assert!(
            decided.is_err(),
            "an outage decides nothing, got {decided:?}"
        );
        match check_authority(&provider, &commission, CAPABILITY) {
            AuthorityCheck::Refused(error) => assert!(
                error.message.contains("Unavailable"),
                "the refusal names the outage: {}",
                error.message
            ),
            other => panic!("an outage is refused, got {other:?}"),
        }
    }
}

/// A policy that cannot answer, for the reason it was given.
struct CouldNotAnswer(Unanswered);

impl PolicyEvaluator for CouldNotAnswer {
    fn evaluate(&self, _request: &PolicyRequest<'_>) -> Result<Evaluation, PolicyError> {
        Err(PolicyError::CouldNotAnswer(self.0))
    }
}

struct BoxedPolicy(Box<dyn PolicyEvaluator>);

impl PolicyEvaluator for BoxedPolicy {
    fn evaluate(&self, request: &PolicyRequest<'_>) -> Result<Evaluation, PolicyError> {
        self.0.evaluate(request)
    }
}

#[test]
fn an_authority_context_that_is_no_mandate_context_grants_nothing() {
    let verified = serde_json::to_value(subject_context())
        .unwrap_or_else(|error| panic!("the verified context serializes: {error}"));
    let mut unknown_member = verified.clone();
    unknown_member["scopes"] = serde_json::json!(["ledger"]);
    let mut missing_subject = verified.clone();
    if let Some(object) = missing_subject.as_object_mut() {
        object.remove("subject");
    }
    let mut not_a_uuid = verified.clone();
    not_a_uuid["subject"] = serde_json::json!("principal-u2");
    let contexts = [
        "null".to_owned(),
        r#""principal-u2""#.to_owned(),
        "{}".to_owned(),
        // Mandate's context itself, not wrapped in the host's context.
        verified.to_string(),
        format!(r#"{{"verified":{verified},"extra":true}}"#),
        format!(r#"{{"verified":{verified},"verified":{verified}}}"#),
        r#"{"verified":null}"#.to_owned(),
        format!(r#"{{"verified":{unknown_member}}}"#),
        format!(r#"{{"verified":{missing_subject}}}"#),
        format!(r#"{{"verified":{not_a_uuid}}}"#),
    ];

    for text in contexts {
        let asked = Cell::new(0);
        let (graph, revision) = granting_graph();
        let provider = authority(
            graph,
            revision,
            Counting {
                asked: &asked,
                inner: allowing(),
            },
        );
        let commission = commission_with(CASE, context_from(&text));
        let decided = provider.decide(commission.data(), CAPABILITY);
        assert!(
            decided.is_err(),
            "{text}: no Mandate context decides nothing, got {decided:?}"
        );
        assert!(
            !check_authority(&provider, &commission, CAPABILITY).allows(),
            "{text}: no grant"
        );
        assert_eq!(asked.get(), 0, "{text}: Mandate was asked nothing");
    }

    // A case id that is no Mandate resource id is never asked about either.
    let asked = Cell::new(0);
    let (graph, revision) = granting_graph();
    let provider = authority(
        graph,
        revision,
        Counting {
            asked: &asked,
            inner: allowing(),
        },
    );
    let commission = commission_with("CHG-1842", host_context(&subject_context()));
    assert!(provider.decide(commission.data(), CAPABILITY).is_err());
    assert_eq!(asked.get(), 0, "Mandate was asked nothing");

    // The same provider, given the host's context, is asked and grants: the refusals above are
    // about the context, not the provider.
    let asked = Cell::new(0);
    let (graph, revision) = granting_graph();
    let provider = authority(
        graph,
        revision,
        Counting {
            asked: &asked,
            inner: allowing(),
        },
    );
    assert_eq!(
        provider.decide(self::commission().data(), CAPABILITY),
        Ok(AuthorityVerdict::Allow(Unit(true)))
    );
    assert_eq!(asked.get(), 1);
}

/// Every `loom.authority.Outcome` answers as the specification's `answer` attribute declares.
/// The synthesized model carries no accessor for the attribute, so `loom_mandate::answer` writes
/// the table again; this holds the two together, variant by variant, both ways.
#[test]
fn every_outcome_answers_as_the_specification_declares() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ess/domains/authority.yaml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let domain: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let types = domain["types"]
        .as_sequence()
        .unwrap_or_else(|| panic!("the domain declares types"));
    let declared = types
        .iter()
        .find(|ty| ty["name"].as_str() == Some("loom.authority.Outcome"))
        .unwrap_or_else(|| panic!("the domain declares loom.authority.Outcome"));
    let specified: BTreeMap<String, String> = declared["variants"]
        .as_sequence()
        .unwrap_or_else(|| panic!("Outcome has variants"))
        .iter()
        .map(|variant| {
            let name = variant["name"]
                .as_str()
                .unwrap_or_else(|| panic!("a named variant"));
            let answer = variant["attributes"]["answer"]
                .as_str()
                .unwrap_or_else(|| panic!("{name} declares its answer"));
            (name.to_owned(), answer.to_owned())
        })
        .collect();

    let outcomes = [
        Outcome::Allowed,
        Outcome::ApprovalRequired,
        Outcome::Denied,
        Outcome::InvalidCredential,
        Outcome::TenantMismatch,
        Outcome::AudienceMismatch,
        Outcome::StaleEpoch,
        Outcome::Unavailable,
        Outcome::NoMandateContext,
        Outcome::NoMandateResource,
        Outcome::Contradictory,
    ];
    let implemented: BTreeMap<String, String> = outcomes
        .iter()
        .map(|outcome| (format!("{outcome:?}"), format!("{:?}", answer(*outcome))))
        .collect();
    assert_eq!(implemented, specified);
    assert!(
        outcomes
            .iter()
            .all(|outcome| answer(*outcome) != Answer::Allow || *outcome == Outcome::Allowed),
        "only Mandate's allow is a grant"
    );
}
