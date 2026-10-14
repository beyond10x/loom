//! Security review of `story:mandate-authority`: integrity conformance checks of
//! `MandateAuthority` against its own contract (`ess/domains/authority.yaml`, `AGENTS.md` § Rules
//! and § Mandate). Authority fails toward less authority, and the model never supplies identity or
//! authority.
//!
//! Each case states the invariant it holds. Mandate's ports are its own published doubles; ids are
//! synthetic.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use b10x_loom_commission::model::json;
use b10x_loom_commission::model::primitives::{Timestamp as CommissionTimestamp, Uuid as Id};
use b10x_loom_commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, AuthorityVerdict, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, ExecutorOutcome,
    ExecutorOutcomeProposedAction, FrontierAction, ObservationId, PrincipalId,
    ProposedActionArguments, RunId, Unit, commission_state,
};
use b10x_loom_commission::outcome::RunStore;
use b10x_loom_commission::ports::authority::{AuthorityCheck, check_authority};
use b10x_loom_commission::ports::authority::AuthorityProvider;
use b10x_loom_commission::ports::effect::{AdmittedRequest, EffectError, EffectPort};
use b10x_loom_commission::runtime::{
    LoopContext, ResumeError, ResumeRefusal, resume_after_grant, run_until_blocked,
    run_with_suspension,
};
use b10x_loom_commission_testkit::fake_executor::ScriptedExecutor;
use b10x_loom_commission_testkit::fake_governor::{Answer as Scripted, FakeGovernor};
use b10x_loom_commission_testkit::file_suspension::FileSuspensionStore;
use loom::authority::{Answer, Outcome};
use loom_mandate::{CASE_RESOURCE_TYPE, MandateAuthority, answer, case_resource, outcome};
use mandate_authz::agent::AgentRecords;
use mandate_authz::decision::{DecisionIdAllocator, Denied, FixedChallenge, SequentialDecisionIds};
use mandate_graph::double::GraphDouble;
use mandate_graph::record::Grant;
use mandate_graph::topology::ResourceRegistry;
use mandate_model::graph::Topology;
use mandate_model::tenancy::{MembershipAuthority, Tenancy};
use mandate_model::{Decision, DecisionChallenge};
use mandate_policy::double::PolicyDouble;
use mandate_policy::port::{
    ChallengeRequirement, Evaluation, PolicyError, PolicyEvaluator, PolicyRequest,
};
use mandate_policy::record::{AuthorizationModel, Policy};
use mandate_types::{
    Action, Audience, AuthorityScope, AuthoritySubject, AuthorizationModelId, AuthzRevision,
    CorrelationId, CredentialId, DecisionId, DecisionReason, DenialReason, GrantId,
    OrganizationId, OrganizationMembershipId, PolicyId, PolicyVersion,
    PrincipalId as MandatePrincipal, ResourceId, ResourceRef, ResourceType, Timestamp, Uuid,
    VerifiedContext,
};

const CAPABILITY: &str = "repository.merge";
const RELATION: &str = "maintainer";
const AUDIENCE: &str = "loom";
/// The case the granted subject holds `maintainer` on.
const CASE: &str = "0c5e0000-0000-4000-8000-00000000c45e";
/// A case nobody holds anything on.
const OTHER_CASE: &str = "0c5e0000-0000-4000-8000-0000000000b2";

fn uuid(byte: u8) -> Uuid {
    Uuid::from_bytes([byte; 16])
}

fn organization() -> OrganizationId {
    OrganizationId::new(uuid(1))
}

/// The subject that holds the grant.
fn granted() -> MandatePrincipal {
    MandatePrincipal::new(uuid(2))
}

/// A member of the same organization that holds nothing.
fn ungranted() -> MandatePrincipal {
    MandatePrincipal::new(uuid(9))
}

fn action() -> Action {
    Action::new(CAPABILITY)
}

fn resource_of(case: &str) -> ResourceRef {
    ResourceRef {
        resource_type: ResourceType::new(CASE_RESOURCE_TYPE),
        resource_id: ResourceId::new(Uuid::parse(case).unwrap_or_else(|_| panic!("{case}"))),
    }
}

fn approver() -> PolicyId {
    PolicyId::new(uuid(40))
}

fn context_of(subject: MandatePrincipal) -> VerifiedContext {
    VerifiedContext {
        subject,
        actor: None,
        organization: organization(),
        audience: Audience::new(AUDIENCE),
        credential: CredentialId::new(uuid(7)),
        delegation: None,
        execution: None,
        correlation: CorrelationId::new("correlation-sec"),
    }
}

fn tenancy() -> Tenancy {
    let mut tenancy = Tenancy::new();
    let host = context_of(granted());
    tenancy
        .create_organization(&host, organization(), "acme")
        .unwrap_or_else(|error| panic!("the organization is recorded: {error:?}"));
    for (n, member) in [(3, granted()), (4, ungranted())] {
        tenancy
            .add_organization_membership(
                &host,
                MembershipAuthority::VerifiedOrganization,
                OrganizationMembershipId::new(uuid(n)),
                organization(),
                member,
            )
            .unwrap_or_else(|error| panic!("a member is recorded: {error:?}"));
    }
    tenancy
}

/// A graph in which only [`granted`] holds `maintainer` on [`CASE`]; both cases are registered.
fn graph() -> (GraphDouble, AuthzRevision) {
    let mut graph = GraphDouble::new();
    let host = context_of(granted());
    graph.admit_subject(organization(), AuthoritySubject::Principal(granted()));
    graph.admit_subject(organization(), AuthoritySubject::Principal(ungranted()));
    for case in [CASE, OTHER_CASE] {
        graph
            .register_resource(&host, &resource_of(case), None)
            .unwrap_or_else(|error| panic!("{case} is registered: {error:?}"));
    }
    let revision = graph.record_grant(Grant::recorded(
        GrantId::new(uuid(30)),
        organization(),
        AuthoritySubject::Principal(granted()),
        AuthorityScope {
            actions: vec![action()],
            resources: vec![resource_of(CASE)],
            space: None,
        },
        RELATION,
    ));
    (graph, revision)
}

fn base_policy() -> PolicyDouble {
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

/// Policy allows the granted subject on [`CASE`] and nothing else.
fn allowing() -> PolicyDouble {
    let mut policy = base_policy();
    policy.allow(
        organization(),
        AuthoritySubject::Principal(granted()),
        action(),
        resource_of(CASE),
    );
    policy
}

fn requiring_approval() -> PolicyDouble {
    let mut policy = base_policy();
    policy.require_approval(
        organization(),
        AuthoritySubject::Principal(granted()),
        action(),
        resource_of(CASE),
        ChallengeRequirement::Approval,
    );
    policy
}

fn provider<'a>(
    policy: impl PolicyEvaluator + 'a,
    allocator: impl DecisionIdAllocator + 'a,
) -> MandateAuthority<'a> {
    let (graph, revision) = graph();
    MandateAuthority::builder()
        .graph(graph)
        .policy(policy)
        .catalog(base_policy())
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

fn id(n: u64) -> Id {
    Id(format!("00000000-0000-4000-8000-{n:012x}"))
}

fn verified_json(context: &VerifiedContext) -> String {
    serde_json::to_string(context).unwrap_or_else(|error| panic!("serializes: {error}"))
}

fn host_context(context: &VerifiedContext) -> AuthorityContext {
    context_from(&format!(r#"{{"verified":{}}}"#, verified_json(context)))
}

fn context_from(text: &str) -> AuthorityContext {
    AuthorityContext(json::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}")))
}

fn commission_with(
    case: &str,
    context: AuthorityContext,
) -> Commission<commission_state::Assigned> {
    Commission::new(CommissionData {
        commission_id: CommissionId(id(0x91)),
        agent_revision_id: AgentRevisionId(id(0x92)),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("principal-sec".to_owned()),
        authority_context: context,
    })
}

/// Counts what policy was asked, and answers as `inner` does.
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

/// A governor whose frontier gates `merge` behind [`CAPABILITY`] on `case`.
fn gating_governor(case: &str) -> FakeGovernor {
    let governor = FakeGovernor::new();
    governor.script(
        CaseId(case.to_owned()),
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
    governor
}

/// An executor (the model) proposing `merge` with `arguments`.
fn proposing(arguments: &str) -> ScriptedExecutor {
    ScriptedExecutor::new([ExecutorOutcome::ProposedAction(
        ExecutorOutcomeProposedAction {
            action: "merge".to_owned(),
            arguments: ProposedActionArguments(
                json::parse(arguments).unwrap_or_else(|error| panic!("{arguments}: {error:?}")),
            ),
        },
    )])
}

fn runs() -> RunStore {
    let mut issued = 0u64;
    RunStore::new(move || {
        issued += 1;
        RunId(id(0x100 + issued))
    })
}

/// How many effects one run through the runtime performed.
fn effects_performed(
    provider: &MandateAuthority<'_>,
    commission: &Commission<commission_state::Assigned>,
    arguments: &str,
) -> usize {
    let governor = gating_governor(&commission.data().case_id.0);
    let executor = proposing(arguments);
    let effects = Effects::default();
    let mut runs = runs();
    // The run's result is not the subject: an invoked effect fails by construction.
    let _ = run_until_blocked(
        &governor,
        &executor,
        provider,
        &effects,
        commission,
        &mut runs,
        &mut Context::default(),
    );
    effects.invoked.get()
}

/// The model proposes the action with arguments naming the granted subject's verified context,
/// a host-shaped authority context and the granted case. Neither the subject nor the resource
/// Mandate is asked about may come from those arguments: only the commission's authority context
/// (the host's) and its case id.
#[test]
fn security_a_model_proposal_naming_a_granted_subject_or_case_grants_nothing() {
    let granted_verified = verified_json(&context_of(granted()));
    let impersonating = format!(
        r#"{{"verified":{granted_verified},"authority_context":{{"verified":{granted_verified}}},"subject":"{}","case_id":"{CASE}","resource":{{"resource_type":"{CASE_RESOURCE_TYPE}","resource_id":"{CASE}"}}}}"#,
        granted()
    );

    // Control: the host's context for the granted subject on the granted case is allowed, so
    // the run reaches the effect. The refusals below are about who and what, not the setup.
    let allowed = commission_with(CASE, host_context(&context_of(granted())));
    let control = provider(allowing(), SequentialDecisionIds::new());
    assert_eq!(
        control.decide(allowed.data(), CAPABILITY),
        Ok(AuthorityVerdict::Allow(Unit(true))),
        "control: the granted subject on the granted case is allowed"
    );
    assert_eq!(
        effects_performed(&control, &allowed, &impersonating),
        1,
        "control: an allowed run performs the effect"
    );

    // The subject: the host says the ungranted member acts; the model names the granted one.
    let as_ungranted = commission_with(CASE, host_context(&context_of(ungranted())));
    let subject_side = provider(allowing(), SequentialDecisionIds::new());
    let decided = subject_side.decide(as_ungranted.data(), CAPABILITY);
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "the host's ungranted subject must not be allowed, got {decided:?}"
    );
    assert_eq!(
        effects_performed(&subject_side, &as_ungranted, &impersonating),
        0,
        "a model naming the granted subject in its arguments must not make it the Mandate subject"
    );

    // The resource: the commission is for a case nobody holds; the model names the granted case.
    let other_case = commission_with(OTHER_CASE, host_context(&context_of(granted())));
    let resource_side = provider(allowing(), SequentialDecisionIds::new());
    let decided = resource_side.decide(other_case.data(), CAPABILITY);
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "the granted subject on another case must not be allowed, got {decided:?}"
    );
    assert_eq!(
        effects_performed(&resource_side, &other_case, &impersonating),
        0,
        "a model naming the granted case in its arguments must not make it the Mandate resource"
    );
}

/// The verdict the provider answers is the same whatever a model could vary: the capability is
/// the only string the runtime passes besides the commission, and it comes from the governor's
/// frontier. A capability that is not the granted action is not allowed, however close it is.
#[test]
fn security_only_the_exact_granted_action_is_allowed() {
    let commission = commission_with(CASE, host_context(&context_of(granted())));
    let provider = provider(allowing(), SequentialDecisionIds::new());
    for capability in [
        "",
        "repository.merge ",
        " repository.merge",
        "REPOSITORY.MERGE",
        "repository.merge\0",
        "repository.*",
        "*",
        "repository",
        "repository.merge.force",
    ] {
        let decided = provider.decide(commission.data(), capability);
        assert!(
            !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
            "{capability:?} is not the granted action and must not be allowed, got {decided:?}"
        );
    }
}

/// Answers the decision ids [`SequentialDecisionIds`] answers, after panicking on its first call.
struct PanicsOnce {
    panicked: bool,
    ids: SequentialDecisionIds,
}

impl DecisionIdAllocator for PanicsOnce {
    fn next_decision_id(&mut self) -> DecisionId {
        if !self.panicked {
            self.panicked = true;
            panic!("the allocator fails once");
        }
        self.ids.next_decision_id()
    }
}

/// An allocator that panicked while a decision held it poisons its lock; every later decision is
/// `Unavailable` (an `Err`) even though Mandate would allow it, never an allow.
#[test]
fn security_a_poisoned_allocator_grants_nothing_afterwards() {
    let commission = commission_with(CASE, host_context(&context_of(granted())));
    let provider = provider(
        allowing(),
        PanicsOnce {
            panicked: false,
            ids: SequentialDecisionIds::new(),
        },
    );
    let first = catch_unwind(AssertUnwindSafe(|| {
        provider.decide(commission.data(), CAPABILITY)
    }));
    assert!(first.is_err(), "the first decision panics with the allocator");

    for _ in 0..2 {
        let decided = provider.decide(commission.data(), CAPABILITY);
        match &decided {
            Err(error) => assert!(
                error.message.contains("Unavailable"),
                "a poisoned allocator is Unavailable: {}",
                error.message
            ),
            other => panic!("a poisoned allocator must decide nothing, got {other:?}"),
        }
        assert!(!check_authority(&provider, &commission, CAPABILITY).allows());
    }
}

fn decision_id() -> DecisionId {
    SequentialDecisionIds::new().next_decision_id()
}

fn decision(allowed: bool, reason: DecisionReason, challenged: bool) -> Decision {
    Decision {
        allowed,
        reason,
        decision_id: decision_id(),
        revision: None,
        challenge: challenged.then(|| DecisionChallenge {
            action: action(),
            resource: resource_of(CASE),
            approver_policy: approver(),
            expires_at: Timestamp::new("2026-10-15T00:00:00Z"),
        }),
        policy_version: None,
        model_version: None,
    }
}

/// Every Mandate answer, contradictory ones included, maps to `Allow` only when it is an allowed
/// decision for the reason `Allowed`; a refusal never maps to `Allow`, whatever its decision says;
/// an approval requirement is never `Allow`; and a contradiction is `Undecided`.
#[test]
fn security_no_mandate_answer_but_a_clean_allow_is_a_grant() {
    for &reason in DecisionReason::VARIANTS {
        for allowed in [true, false] {
            for challenged in [true, false] {
                let ok = Ok(decision(allowed, reason, challenged));
                let got = outcome(&ok);
                let clean = allowed && reason == DecisionReason::Allowed;
                if clean {
                    assert_eq!(got, Outcome::Allowed, "{ok:?}");
                } else {
                    assert_eq!(
                        got,
                        Outcome::Contradictory,
                        "an Ok that is not a clean allow is a contradiction: {ok:?}"
                    );
                    assert_eq!(answer(got), Answer::Undecided);
                }
            }
        }
    }

    for &reason in DenialReason::VARIANTS {
        for &decided in DecisionReason::VARIANTS {
            for allowed in [true, false] {
                for challenged in [true, false] {
                    let refused: Result<Decision, Denied> = Err(Denied {
                        reason,
                        decision: Box::new(decision(allowed, decided, challenged)),
                    });
                    let got = outcome(&refused);
                    assert_ne!(
                        answer(got),
                        Answer::Allow,
                        "a refusal must never be a grant: {refused:?} -> {got:?}"
                    );
                    if allowed {
                        assert_eq!(
                            got,
                            Outcome::Contradictory,
                            "a refusal whose decision allowed is a contradiction: {refused:?}"
                        );
                    }
                    if reason == DenialReason::ApprovalRequired && !challenged {
                        assert_eq!(
                            got,
                            Outcome::Contradictory,
                            "an approval requirement with no challenge is a contradiction"
                        );
                    }
                    if reason == DenialReason::Unavailable {
                        assert_eq!(answer(got), Answer::Undecided, "an outage decides nothing");
                    }
                }
            }
        }
    }
}

/// A Mandate verified context that names a member twice is ambiguous about who the subject is.
/// The host context refuses a duplicated `verified`; a duplicated `subject` inside it must be
/// refused too, in either order, and Mandate asked nothing.
#[test]
fn security_a_verified_context_naming_its_subject_twice_grants_nothing() {
    let granted_json = verified_json(&context_of(granted()));
    let body = granted_json
        .strip_prefix('{')
        .unwrap_or_else(|| panic!("an object: {granted_json}"));
    let texts = [
        format!(r#"{{"verified":{{"subject":"{}",{body}}}"#, ungranted()),
        format!(
            r#"{{"verified":{},"subject":"{}"}}}}"#,
            granted_json.trim_end_matches('}'),
            ungranted()
        ),
    ];
    for text in texts {
        let asked = Cell::new(0);
        let provider = provider(
            Counting {
                asked: &asked,
                inner: allowing(),
            },
            SequentialDecisionIds::new(),
        );
        let commission = commission_with(CASE, context_from(&text));
        let decided = provider.decide(commission.data(), CAPABILITY);
        assert!(
            decided.is_err(),
            "{text}: a context naming its subject twice decides nothing, got {decided:?}"
        );
        assert_eq!(asked.get(), 0, "{text}: Mandate was asked nothing");
    }
}

/// An approval requirement never grants: not at the call, and not when the run is resumed while
/// Mandate still requires the approval. The resume asks Mandate again and is refused.
#[test]
fn security_an_approval_requirement_is_not_granted_on_resume() {
    let commission = commission_with(CASE, host_context(&context_of(granted())));
    let provider = provider(requiring_approval(), SequentialDecisionIds::new());
    assert!(!check_authority(&provider, &commission, CAPABILITY).allows());

    let governor = gating_governor(CASE);
    let executor = proposing("null");
    let effects = Effects::default();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("security-w10-u2")
        .join("resume");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    }
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    let suspensions = FileSuspensionStore::open(dir.clone())
        .unwrap_or_else(|error| panic!("the suspension store opens: {error:?}"));
    let mut runs = runs();
    run_with_suspension(
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
    assert_eq!(effects.invoked.get(), 0, "nothing performed before the approval");

    let resumed = resume_after_grant(
        &governor,
        &executor,
        &provider,
        &effects,
        &commission,
        &mut runs,
        &suspensions,
        &mut Context::default(),
    );
    match resumed {
        Err(ResumeError::Refused(ResumeRefusal::NotGranted(AuthorityCheck::Decided(
            AuthorityVerdict::ApprovalRequired(_),
        )))) => {}
        other => panic!("a resume Mandate still requires approval for is refused, got {other:?}"),
    }
    assert_eq!(effects.invoked.get(), 0, "a resume without approval performs nothing");
}

/// Two distinct Loom cases are two distinct Mandate resources. Loom compares case ids as exact
/// strings, so two case ids that differ only in the letter case of their hex digits are two cases;
/// a grant on one must not be read as a grant on the other.
#[test]
fn security_distinct_case_ids_are_distinct_mandate_resources() {
    let lower = CaseId(CASE.to_owned());
    let upper = CaseId(CASE.to_uppercase());
    assert_ne!(lower, upper, "Loom holds these as two case ids");
    let lower_resource = case_resource(&lower);
    let upper_resource = case_resource(&upper);
    assert!(
        upper_resource.is_none() || upper_resource != lower_resource,
        "case ids {} and {} map to one Mandate resource {lower_resource:?}: a grant on one case answers for the other",
        lower.0,
        upper.0
    );

    // Through the provider: the granted subject holds `maintainer` on CASE only.
    let commission = commission_with(&CASE.to_uppercase(), host_context(&context_of(granted())));
    let provider = provider(allowing(), SequentialDecisionIds::new());
    let decided = provider.decide(commission.data(), CAPABILITY);
    assert!(
        !matches!(decided, Ok(AuthorityVerdict::Allow(_))),
        "case {} was granted through the grant on case {CASE}: {decided:?}",
        CASE.to_uppercase()
    );
}

/// The packages of the workspace's `Cargo.lock`, each with its direct dependencies' names.
fn locked_packages() -> BTreeMap<String, BTreeSet<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut packages = BTreeMap::new();
    for block in text.split("[[package]]").skip(1) {
        let name = block
            .lines()
            .find_map(|line| line.strip_prefix("name = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("a package has a name: {block}"))
            .to_owned();
        let mut dependencies = BTreeSet::new();
        if let Some((_, list)) = block.split_once("dependencies = [") {
            let list = list.split(']').next().unwrap_or_default();
            for entry in list.split(',') {
                let entry = entry.trim().trim_matches('"');
                if let Some(dependency) = entry.split_whitespace().next() {
                    dependencies.insert(dependency.to_owned());
                }
            }
        }
        packages.entry(name).or_insert_with(BTreeSet::new).extend(dependencies);
    }
    packages
}

/// Mandate is linked by `b10x-loom-mandate` alone: no other package of the lock depends on it or
/// on a Mandate crate, Commission included; and what it links is closed over Mandate's pure crates
/// and serde, so no token, network or identity crate (which would hold or fetch a credential) comes
/// with it. The provider's own source opens no file, environment variable, process or socket.
#[test]
fn security_mandate_is_linked_by_one_crate_and_reads_no_credential() {
    let packages = locked_packages();
    let is_mandate = |name: &str| name == "mandate" || name.starts_with("mandate-");
    for (package, dependencies) in &packages {
        if package == "b10x-loom-mandate" || is_mandate(package) {
            continue;
        }
        let linked: Vec<&String> = dependencies
            .iter()
            .filter(|dependency| is_mandate(dependency) || *dependency == "b10x-loom-mandate")
            .collect();
        assert!(linked.is_empty(), "{package} links {linked:?}");
    }

    let mut reached = BTreeSet::new();
    let mut pending: Vec<String> = packages["b10x-loom-mandate"]
        .iter()
        .filter(|dependency| is_mandate(dependency))
        .cloned()
        .collect();
    while let Some(next) = pending.pop() {
        if !reached.insert(next.clone()) {
            continue;
        }
        if let Some(dependencies) = packages.get(&next) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    let allowed = ["serde", "serde_json", "serde_derive", "itoa", "ryu", "zmij", "memchr"];
    let beyond: Vec<&String> = reached
        .iter()
        .filter(|name| {
            !is_mandate(name)
                && !allowed.contains(&name.as_str())
                && !name.starts_with("proc-macro")
                && !["quote", "syn", "unicode-ident", "serde_core"].contains(&name.as_str())
        })
        .collect();
    assert!(beyond.is_empty(), "Mandate brings {beyond:?}");
    for credential in ["mandate-token", "mandate-client", "mandate-identity", "mandate-server"] {
        assert!(
            !reached.contains(credential),
            "b10x-loom-mandate links {credential}"
        );
    }

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let text = std::fs::read_to_string(&source)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    for reach in ["std::env", "std::fs", "std::process", "std::net", "env!(", "option_env!("] {
        assert!(
            !text.contains(reach),
            "{} reaches {reach}",
            source.display()
        );
    }
}
