//! Adversary cases for `story:mandate-authority` (wave 2026-10-14-w1, U2): edges of the outcome
//! mapping, contexts and case ids the host did not supply as the domain says, a poisoned
//! allocator, an outage through `run_with_suspension`, and the mutants the unit's own suite does
//! not catch (a case resource type held only by its own constant, the action, the resource id and
//! the subject Mandate is asked about). Ids are synthetic.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use b10x_loom_commission::model::behaviour::Generated;
use b10x_loom_commission::model::json::{self, Value};
use b10x_loom_commission::model::primitives::{Timestamp as CommissionTimestamp, Uuid as Id};
use b10x_loom_commission::model::responsibility::{
    ActionRequestData, ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext,
    AuthorityVerdict, CaseId, Commission, CommissionData, CommissionId, EffectOutcome,
    ExecutorOutcome, ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityProvider, check_authority};
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{LoopContext, run_with_suspension};
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer as Scripted, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;
use loom::authority::Outcome;
use loom_mandate::{MandateAuthority, case_resource_id, outcome};
use mandate_authz::agent::AgentRecords;
use mandate_authz::decision::{DecisionIdAllocator, Denied, FixedChallenge, SequentialDecisionIds};
use mandate_graph::double::GraphDouble;
use mandate_graph::record::Grant;
use mandate_graph::topology::ResourceRegistry;
use mandate_model::Decision;
use mandate_model::graph::Topology;
use mandate_model::tenancy::{MembershipAuthority, Tenancy};
use mandate_policy::double::PolicyDouble;
use mandate_policy::port::{Evaluation, PolicyError, PolicyEvaluator, PolicyRequest};
use mandate_policy::record::{AuthorizationModel, Policy};
use mandate_types::{
    Action, Audience, AuthorityScope, AuthoritySubject, AuthorizationModelId, AuthzRevision,
    CorrelationId, CredentialId, DecisionId, DecisionReason, DenialReason, GrantId, OrganizationId,
    OrganizationMembershipId, PolicyId, PolicyVersion, PrincipalId as MandatePrincipal,
    ResourceRef, ResourceType, Timestamp, Uuid, VerifiedContext,
};

const CAPABILITY: &str = "repository.merge";
const RELATION: &str = "maintainer";
const AUDIENCE: &str = "loom";
const CASE: &str = "0c5e0000-0000-4000-8000-00000000c45e";
/// The resource type the domain and the crate documentation name, written as a literal here and
/// never read from `loom_mandate::CASE_RESOURCE_TYPE`.
const SPECIFIED_CASE_TYPE: &str = "loom.case";

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

fn resource_of(case: &str) -> ResourceRef {
    ResourceRef {
        resource_type: ResourceType::new(SPECIFIED_CASE_TYPE),
        resource_id: case_resource_id(&CaseId(case.to_owned()))
            .unwrap_or_else(|| panic!("{case} derives a resource id")),
    }
}

fn resource() -> ResourceRef {
    resource_of(CASE)
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
        correlation: CorrelationId::new("correlation-adversary-u2"),
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

/// A graph in which the subject holds `maintainer` on `resource` (type `loom.case`, literally).
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

fn allowing() -> PolicyDouble {
    let mut policy = policy();
    policy.allow(organization(), subject(), action(), resource());
    policy
}

fn authority_with<'a>(
    policy: impl PolicyEvaluator + 'a,
    allocator: impl DecisionIdAllocator + 'a,
) -> MandateAuthority<'a> {
    let (graph, revision) = granting_graph();
    MandateAuthority::builder()
        .graph(graph)
        .policy(policy)
        .catalog(self::policy())
        .issuer(FixedChallenge {
            expires_at: Timestamp::new("2026-10-15T00:00:00Z"),
            approver_policy: Some(approver()),
        })
        .agents(AgentRecords::new())
        .allocator(allocator)
        .tenancy(tenancy())
        .topology(Topology::new())
        .audience(Audience::new(AUDIENCE))
        .relation(RELATION)
        .minimum_revision(revision)
        .clock(|| Timestamp::new("2026-10-14T09:00:00Z"))
        .build()
        .unwrap_or_else(|missing| panic!("{missing}"))
}

fn authority<'a>(policy: impl PolicyEvaluator + 'a) -> MandateAuthority<'a> {
    authority_with(policy, SequentialDecisionIds::new())
}

fn id(n: u64) -> Id {
    Id(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn context_from(text: &str) -> AuthorityContext {
    AuthorityContext(json::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}")))
}

fn host_context(context: &VerifiedContext) -> AuthorityContext {
    let verified = serde_json::to_string(context)
        .unwrap_or_else(|error| panic!("the verified context serializes: {error}"));
    context_from(&format!(r#"{{"verified":{verified}}}"#))
}

fn commission_with(
    case: &str,
    context: AuthorityContext,
) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(id(0x81)),
        agent_revision_id: AgentRevisionId(id(0x82)),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-adversary-u2".to_owned()),
        authority_context: context,
    })
}

fn commission() -> Commission<commission_state::Assigned> {
    commission_with(CASE, host_context(&subject_context()))
}

/// The action request every decision here is about. `MandateAuthority` does not read it.
fn request() -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(id(0xe1)),
        run_id: RunId(id(0xe2)),
        case_id: CaseId(CASE.to_owned()),
        expected_case_revision: 1,
        action: "merge".to_owned(),
        arguments: ProposedActionArguments(Value::Null),
    }
}

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

/// A Commission case id is an opaque `String` (`commission.responsibility.CaseId`), so two
/// spellings are two cases; Mandate's `Uuid::parse` accepts upper case, so both are one Mandate
/// resource. A grant on one case must not be a grant on another.
#[test]
fn a_case_id_spelled_in_upper_case_is_not_granted_the_lower_case_case() {
    let provider = authority(allowing());
    // The control: the case the grant is on is granted.
    assert_eq!(
        provider.decide(commission().data(), &request(), CAPABILITY),
        Ok(AuthorityVerdict::Allow(Unit(true)))
    );
    let other = CASE.to_uppercase();
    assert_ne!(other, CASE, "a different Commission case id");
    let decided = provider.decide(
        commission_with(&other, host_context(&subject_context())).data(),
        &request(),
        CAPABILITY,
    );
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "case {other} is another Commission case than {CASE}, yet the grant on {CASE} granted it: {decided:?}"
    );
}

/// A decision id allocator that panics, as a host's allocator may.
struct Panicking;

impl DecisionIdAllocator for Panicking {
    fn next_decision_id(&mut self) -> DecisionId {
        panic!("the allocator fails (adversary)")
    }
}

#[test]
fn a_poisoned_allocator_asks_mandate_nothing_and_grants_nothing() {
    let asked = Cell::new(0);
    let provider = authority_with(
        Counting {
            asked: &asked,
            inner: allowing(),
        },
        Panicking,
    );
    let commission = commission();
    let first = catch_unwind(AssertUnwindSafe(|| {
        provider.decide(commission.data(), &request(), CAPABILITY)
    }));
    assert!(first.is_err(), "the allocator panicked through decide");
    let before = asked.get();

    let second = provider.decide(commission.data(), &request(), CAPABILITY);
    match &second {
        Err(error) => assert!(
            error.message.contains("Unavailable"),
            "a poisoned allocator is Unavailable: {}",
            error.message
        ),
        Ok(verdict) => panic!("a poisoned allocator decides nothing, got {verdict:?}"),
    }
    assert!(!check_authority(&provider, &commission, &request(), CAPABILITY).allows());
    assert_eq!(
        asked.get(),
        before,
        "Mandate was not asked after the poison"
    );
}

#[test]
fn a_verified_context_with_a_duplicated_member_grants_nothing() {
    let verified = serde_json::to_string(&subject_context())
        .unwrap_or_else(|error| panic!("the verified context serializes: {error}"));
    let other_subject = MandatePrincipal::new(uuid(9)).to_string();
    // A second `subject` in front of the host's: a last-wins or first-wins reader would take one.
    let injected = verified.replacen('{', &format!(r#"{{"subject":"{other_subject}","#), 1);
    let contexts = [
        format!(r#"{{"verified":{injected}}}"#),
        // `actor` written explicitly null, where Mandate preserves absent as absent.
        verified.replacen('{', r#"{"actor":null,"#, 1),
        // A subject that is a number, and one that is an array.
        verified.replace(&format!(r#""subject":"{}""#, principal()), r#""subject":2"#),
        r#"{"verified":[]}"#.to_owned(),
        r#"{"verified":"{}"}"#.to_owned(),
    ];
    for (index, text) in contexts.iter().enumerate() {
        // The second is wrapped here, not above, so its inner text stays readable.
        let text = if index == 1 || index == 2 {
            format!(r#"{{"verified":{text}}}"#)
        } else {
            text.clone()
        };
        let asked = Cell::new(0);
        let provider = authority(Counting {
            asked: &asked,
            inner: allowing(),
        });
        let decided = provider.decide(
            commission_with(CASE, context_from(&text)).data(),
            &request(),
            CAPABILITY,
        );
        assert!(
            !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
            "{text}: granted {decided:?}"
        );
        if index != 1 {
            assert!(
                decided.is_err(),
                "{text}: no Mandate context, got {decided:?}"
            );
            assert_eq!(asked.get(), 0, "{text}: Mandate was asked nothing");
        }
    }
}

/// The case is the resource of type `loom.case` (`ess/domains/authority.yaml`, the crate
/// documentation). Every case in the unit's suite builds its resource from
/// `loom_mandate::CASE_RESOURCE_TYPE`, so a change to that constant stays green there; this
/// grants on the literal type and must be allowed.
#[test]
fn the_case_resource_is_of_the_specified_type_loom_case() {
    let provider = authority(allowing());
    assert_eq!(
        provider.decide(commission().data(), &request(), CAPABILITY),
        Ok(AuthorityVerdict::Allow(Unit(true))),
        "a grant on a `{SPECIFIED_CASE_TYPE}` resource is the grant the provider asks about"
    );
}

/// Mandate is asked about the commission's case, the capability asked and the context's subject:
/// a grant on one of them is no grant on another.
#[test]
fn a_grant_is_on_its_case_its_capability_and_its_subject_only() {
    let provider = authority(allowing());
    let another_case = "0c5e0000-0000-4000-8000-00000000c45f";
    let decided = provider.decide(
        commission_with(another_case, host_context(&subject_context())).data(),
        &request(),
        CAPABILITY,
    );
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "another case: {decided:?}"
    );

    let decided = provider.decide(commission().data(), &request(), "repository.delete");
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "another capability: {decided:?}"
    );

    let stranger = VerifiedContext {
        subject: MandatePrincipal::new(uuid(9)),
        ..subject_context()
    };
    let decided = provider.decide(
        commission_with(CASE, host_context(&stranger)).data(),
        &request(),
        CAPABILITY,
    );
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "another subject: {decided:?}"
    );
}

fn decision(allowed: bool, reason: DecisionReason) -> Decision {
    Decision {
        allowed,
        reason,
        decision_id: DecisionId::new(uuid(50)),
        revision: None,
        challenge: None,
        policy_version: None,
        model_version: None,
    }
}

fn denied(reason: DenialReason, decision: Decision) -> Result<Decision, Denied> {
    Err(Denied {
        reason,
        decision: Box::new(decision),
    })
}

#[test]
fn an_answer_that_contradicts_itself_is_contradictory_and_never_a_grant() {
    let cases: Vec<(&str, Result<Decision, Denied>, Outcome)> = vec![
        (
            "an allow that is not allowed",
            Ok(decision(false, DecisionReason::Allowed)),
            Outcome::Contradictory,
        ),
        (
            "an allow for a denial reason",
            Ok(decision(true, DecisionReason::Denied)),
            Outcome::Contradictory,
        ),
        (
            "a refusal whose decision allowed",
            denied(
                DenialReason::Denied,
                decision(true, DecisionReason::Allowed),
            ),
            Outcome::Contradictory,
        ),
        (
            "an approval requirement with no challenge",
            denied(
                DenialReason::ApprovalRequired,
                decision(false, DecisionReason::ApprovalRequired),
            ),
            Outcome::Contradictory,
        ),
        (
            "a refusal whose decision names allow but did not allow",
            denied(
                DenialReason::Denied,
                decision(false, DecisionReason::Allowed),
            ),
            Outcome::Denied,
        ),
        (
            "an outage",
            denied(
                DenialReason::Unavailable,
                decision(false, DecisionReason::Unavailable),
            ),
            Outcome::Unavailable,
        ),
    ];
    for (what, answer, expected) in cases {
        assert_eq!(outcome(&answer), expected, "{what}");
        assert_ne!(
            loom_mandate::answer(outcome(&answer)),
            loom::authority::Answer::Allow,
            "{what}: no grant"
        );
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
        .join("mandate-authority-adversary")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    }
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    dir
}

/// Through the runtime, a Mandate that cannot answer (no policy recorded) performs nothing, for a
/// frontier that gates the action behind the capability. (An `Admissible` action is admitted by
/// Commission's `admit` without asking authority at all, by design, so it is not a case here.)
#[test]
fn an_unavailable_mandate_through_run_with_suspension_performs_nothing() {
    {
        let (name, status) = ("approval", ActionStatus::ApprovalRequired);
        let provider = authority(PolicyDouble::new());
        let commission = commission();
        let case = CaseId(CASE.to_owned());
        let governor = FakeGovernor::new();
        governor.script(
            case.clone(),
            [Scripted::at(3).with_items(
                Vec::new(),
                Vec::new(),
                vec![FrontierAction {
                    action: "merge".to_owned(),
                    status,
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
        let suspensions = FileSuspensionStore::open(store_dir(name))
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
        );
        assert_eq!(
            effects.invoked.get(),
            0,
            "{name}: an outage performed the action; the run ended {:?}",
            end.map(|end| end.outcome)
        );
    }
}
