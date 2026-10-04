# Beyond10x Agent SDK

**Working name:** Beyond10x Agent SDK  
**Proposed repository:** `beyond10x/agent-sdk`  
**Primary Rust crate:** `b10x-agent-sdk`  
**Status:** Design proposal  
**Date:** 2026-10-04

---

## 1. Summary

The Beyond10x Agent SDK is a proposed Rust SDK for building **governed autonomous workers**.

Its purpose is not to provide another abstraction around:

```text
LLM + prompt + tools + loop
```

Its purpose is to make autonomous agents usable as participants in real organizational work.

The core abstraction is:

```text
Agent
  + Assignment
  + Case
  + Protocol
  + Authority
  + Actions
  + Evidence
  = Governed autonomous worker
```

The central product idea is:

> **Build agents you can give responsibility to.**

The technical principle is:

> **Model intelligence inside a deterministic operating envelope.**

The SDK composes existing and proposed Beyond10x boundaries rather than reimplementing them:

- **Canon** defines generic protocol semantics.
- **ELS** defines engineering-domain protocols.
- **AEP** governs live engineering cases.
- **Harness** owns the model/tool loop.
- **Mandate** resolves authority and delegation.
- **Connectors** provide typed external integrations.
- **Substrate** bounds external execution.
- **ESS, Gates, CI, operations, and other verifiers** produce evidence.
- **Entity Runtime** may provide generic deterministic state machinery below services.
- **Agent Platform** may provide a managed hosting/control plane for agents and assignments.

The Agent SDK is the ergonomic application-development layer that composes these systems into one coherent programming model.

---

## 2. Product thesis

Most agent SDKs begin with an agent.

Conceptually:

```text
Agent
  = Model
  + Instructions
  + Tools
```

Some add:

```text
Handoffs
Sessions
Memory
Guardrails
Tracing
Workflow graphs
Human approval
```

These are useful capabilities.

However, they primarily answer:

> **How can this model perform work?**

The Beyond10x Agent SDK begins one level higher:

> **Under what responsibility, protocol, authority, and standard of proof is this agent performing work?**

The primary developer problem is therefore not only agent capability.

It is **delegated responsibility**.

---

## 3. Differentiating idea

A conventional agent runtime often looks like:

```text
Developer registers tools
        ↓
Model receives tools
        ↓
Model chooses tools
        ↓
Runtime executes tools
        ↓
Model declares completion
```

The proposed SDK instead follows:

```text
Case
  +
Protocol frontier
  +
Actor authority
  +
Available integrations
  +
Environment capabilities
        ↓
Dynamic action projection
        ↓
Model receives only currently admissible actions
        ↓
Agent acts
        ↓
Observed results become evidence
        ↓
Case is reevaluated
        ↓
New frontier
```

This changes the meaning of a tool.

A tool is not merely a function available to a model.

It is a projection of a **currently admissible action** into a model/tool runtime.

---

## 4. Positioning

### 4.1 One-line description

> **A Rust SDK for protocol-driven autonomous workers with first-class authority, durable cases, evidence, human escalation, and governed access to real-world systems.**

### 4.2 Short developer pitch

> **Other agent SDKs help you make agents capable. Beyond10x Agent SDK helps you make agents responsible.**

### 4.3 Broader product relationship

The SDK is a developer surface for the broader Beyond10x product idea:

> **Governed Autonomy — from intent to verified outcome.**

It should allow developers to build domain-specific autonomous workers without rebuilding governance, authorization, evidence, execution, and integration semantics in every application.

---

## 5. Current ecosystem context

This proposal builds on boundaries already present in the Beyond10x ecosystem.

As of this design:

### AEP

AEP governs engineering plans and governed tasks. It decides legal and earned moves, consumes evidence, resolves capabilities and obligations, drives workflows, and observes agent/check behavior.

AEP explicitly does not own model execution or credentials.

### Harness

Harness owns the agent loop:

- model turns;
- tool round trips;
- approvals;
- budgets;
- session records;
- the tool catalogue exposed to the model.

Its library boundary is therefore a natural execution adapter for the Agent SDK.

### Connectors

Connectors provides a consistent typed boundary for external integrations through versioned operations and adapters.

This makes it a natural source of external actions exposed through the SDK.

### Entity Runtime

Entity Runtime demonstrates the trusted-shell pattern:

- the model proposes named operations and domain arguments;
- trusted code supplies canonical state, identity, provenance, revision expectations, and authority;
- deterministic policy decides;
- refused operations do not mutate state.

The Agent SDK should preserve this architectural principle.

### Agent Platform

Agent Platform already manages stable agents, immutable revisions, capability profiles, tasks, evidence, approval continuations, and triggers.

The SDK should be able to use this as a managed control plane without requiring it for local development.

---

## 6. Design principles

### 6.1 Responsibility before capability

The SDK should make it natural to ask:

```text
What is this agent responsible for?
```

before:

```text
Which tools does this agent have?
```

### 6.2 Cases outlive conversations

A conversation is not the durable unit of work.

A `Case` may survive:

- multiple model contexts;
- multiple runs;
- multiple agents;
- human intervention;
- waiting periods;
- process restarts;
- days or months of elapsed time.

### 6.3 Authority is external to model intent

A model can request an action.

It cannot grant itself authority to perform it.

### 6.4 Evidence is different from tracing

A trace records what happened during execution.

Evidence is an admissible observation that may change what a governed case can legitimately claim.

The SDK must preserve this distinction.

### 6.5 Tools are projected, not owned

The agent does not permanently "own" consequential tools.

The runtime projects actions based on:

```text
available integration
∩ protocol admissibility
∩ actor authority
∩ environment capability
∩ current case frontier
```

### 6.6 Human involvement is a protocol primitive

Human participation is not an ad hoc callback.

The SDK should distinguish:

```text
approval
judgment
review
attestation
selection
exception
escalation
```

### 6.7 The agent chooses how; the protocol determines what counts

The protocol should constrain work without reducing the agent to a deterministic workflow runner.

### 6.8 Local and managed use share one programming model

An application should be able to move from:

```text
in-process/local
```

to:

```text
managed services
```

without being rewritten around an entirely different agent abstraction.

### 6.9 Composition, not ownership

The SDK should compose ecosystem services.

It must not become a monolith that reimplements their semantics.

---

## 7. Core abstraction model

The proposed conceptual hierarchy is:

```text
Agent
    reusable autonomous worker configuration

AgentRevision
    immutable version of an Agent configuration

Case
    durable organizational undertaking

Assignment
    relationship between AgentRevision and Case under a role/authority context

Run
    one bounded period of agent execution

Session
    model-context continuity

Trace
    execution observability

Observation
    something observed by an integration/verifier/runtime

Evidence
    validated observation admissible to protocol evaluation

Frontier
    current set of obligations, admissible actions, blocked actions,
    unknowns, authority requirements, and available conclusions

Outcome
    protocol-defined terminal disposition
```

---

## 8. Why `Assignment` is the key primitive

An agent configuration is not yet work.

An `Assignment` gives the agent organizational meaning.

Conceptually:

```text
Agent Revision A17
    is assigned to

Case INC-492
    under

Protocol engineering.incident-response/1
    acting as

Principal agent:incident-responder
    with

Authority derived from organization policy
    using

Available integration/environment capabilities
```

This produces a concrete responsibility boundary.

An assignment may be:

- started;
- suspended;
- blocked;
- waiting for authority;
- waiting for external evidence;
- reassigned;
- resumed;
- completed;
- revoked.

---

## 9. Case, assignment, run, and session separation

This distinction should be foundational.

```text
Case
    durable work object

    Assignment 1
        Agent A
        Run 1
            Session A1
        Run 2
            Session A2

    Assignment 2
        Agent B
        Run 3
            Session B1
```

Example:

```text
Case: INC-342

Run 1:
    responder agent investigates

Run 2:
    same agent resumes after production approval

Run 3:
    independent verifier checks recovery

Run 4:
    responder performs post-incident remediation
```

The incident is not equivalent to any one run.

The case is the durable source of organizational continuity.

---

## 10. Proposed high-level Rust API

The following examples illustrate the intended experience rather than fixing exact names.

```rust
use b10x_agent_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let sdk = AgentSdk::builder()
        .executor(HarnessExecutor::local())
        .governor(AepGovernor::connect("https://aep.internal"))
        .authority(MandateAuthority::connect("https://mandate.internal"))
        .integrations(Connectors::connect("https://connectors.internal"))
        .build()
        .await?;

    let engineer = sdk
        .agent("software-engineer")
        .model("openai/gpt-6")
        .instructions(
            "Work toward the assigned objective. \
             Establish required evidence as you go."
        )
        .build();

    let assignment = sdk
        .assign(engineer)
        .case("CHG-1842")
        .await?;

    let outcome = assignment.run_until_blocked().await?;

    println!("{outcome:#?}");

    Ok(())
}
```

Notably absent:

```rust
.tool(github_merge)
.tool(kubectl)
.tool(database_write)
```

Those capabilities should be projected from the governed context.

---

## 11. Protocol binding

The assignment may inherit its protocol from the case.

For explicit construction:

```rust
let assignment = sdk
    .assign(engineer)
    .new_case(NewCase {
        kind: "engineering.software-change",
        protocol: "software.change/1",
        input: change_request,
    })
    .await?;
```

For generic-domain workers:

```rust
let assignment = sdk
    .assign(expense_reviewer)
    .new_case(NewCase {
        kind: "finance.expense-review",
        protocol: "finance.expense-review/1",
        input: expense,
    })
    .await?;
```

Canon defines generic protocol semantics.

ELS may supply engineering protocols.

AEP may serve as the engineering governor.

The SDK should not hardcode engineering concepts into its generic core.

---

## 12. Frontier-driven execution

The central runtime input for an agent should be a `Frontier`.

Illustrative data:

```yaml
case:
  id: INC-492
  revision: 17

claims:
  service.impact_bounded: true
  service.restored: unknown
  cause.identified: unknown

obligations:
  - id: mitigate_customer_impact
    priority: immediate

  - id: establish_incident_timeline
    priority: normal

admissible_actions:
  - inspect.production_metrics
  - inspect.recent_deployments
  - compare.regional_health

conditional_actions:
  rollback.release:
    requires_authority:
      capability: production.rollback

blocked_conclusions:
  close_incident:
    because:
      - service.restored is UNKNOWN
```

The agent should receive an appropriate projection of this information into its execution context.

---

## 13. Runtime loop

Conceptually:

```text
load canonical case
        ↓
evaluate protocol
        ↓
obtain frontier
        ↓
project admissible actions
        ↓
run model
        ↓
model selects action
        ↓
validate action against current frontier
        ↓
resolve authority if required
        ↓
execute through trusted adapter
        ↓
capture observations
        ↓
validate/project evidence
        ↓
record
        ↓
reevaluate protocol
        ↓
continue / block / escalate / complete
```

This loop separates:

- probabilistic selection;
- deterministic admissibility;
- external authority;
- side effects;
- observation;
- evidence;
- protocol truth.

---

## 14. `run_until_blocked`

A primary ergonomic API should support bounded autonomous progress.

```rust
let result = assignment.run_until_blocked().await?;
```

Possible outcomes:

```rust
pub enum RunOutcome {
    Completed(Outcome),
    NeedsAuthority(AuthorityRequest),
    NeedsHumanJudgment(HumanDecisionRequest),
    NeedsExternalEvidence(Vec<EvidenceRequirement>),
    NoAdmissibleAction(Frontier),
    BudgetExhausted(BudgetState),
    Suspended(Suspension),
}
```

This is intentionally different from:

```text
final_output: String
```

A real organizational task often ends a run without ending the case.

---

## 15. Dynamic action projection

This is one of the defining SDK features.

Let:

```text
I = operations currently available from integrations
P = actions admissible under the protocol
A = actions permitted by authority
E = actions executable in the current environment
```

Then approximately:

```text
Model Tool Catalogue = I ∩ P ∩ A ∩ E
```

Some authority may be resolved lazily.

In that case:

```text
I ∩ P ∩ E
```

may produce a tool that is visible but causes a durable approval suspension before execution.

The distinction must be explicit.

---

## 16. Action model

A generic action might be described as:

```rust
pub trait Action {
    type Input: DeserializeOwned + JsonSchema;
    type Observation: Observation;

    fn id(&self) -> ActionId;

    async fn execute(
        &self,
        ctx: &ActionContext,
        input: Self::Input,
    ) -> Result<Self::Observation>;
}
```

An action can declare metadata:

```rust
pub struct ActionDescriptor {
    pub id: ActionId,
    pub input_schema: JsonSchema,
    pub effect: EffectClass,
    pub authority: Option<Capability>,
    pub evidence_kinds: Vec<EvidenceKind>,
}
```

Potential effect classes:

```text
read_only
local_mutation
external_mutation
financial
production
irreversible
custom namespace
```

Effect class is metadata.

Authority remains independently resolved.

---

## 17. Integration bindings

A protocol action may bind to different transports.

### Native Rust

```rust
actions.bind(
    "customer.lookup",
    RustAction::new(customer_lookup),
);
```

### Connector

```rust
actions.bind(
    "customer.lookup",
    connectors.operation("salesforce/customer.get"),
);
```

### MCP

```rust
actions.bind(
    "knowledge.search",
    mcp.tool("company-wiki/search"),
);
```

### Harness tool

```rust
actions.bind(
    "workspace.edit",
    harness.tool("file_edit"),
);
```

### Workflow invocation

```rust
actions.bind(
    "release.execute",
    workflow.invoke("release-production"),
);
```

The model sees action semantics rather than provider transport details.

---

## 18. Connectors integration

Connectors should be a first-class integration provider.

Connector operations can advertise:

- operation ID;
- input schema;
- output schema;
- effect metadata;
- grant requirements;
- provider metadata.

The Agent SDK maps these to protocol actions.

Example:

```text
Connector operations
        ∩
Protocol frontier
        ∩
Actor authority
        ∩
Environment policy
        ↓
Agent-visible action catalogue
```

This avoids treating every connected SaaS operation as permanently available model capability.

---

## 19. Harness integration

Harness should remain the owner of the agent loop.

The Agent SDK supplies:

- model configuration;
- instructions/context projection;
- dynamic tool/action catalogue;
- approval bridge;
- budgets;
- case/run correlation metadata.

Harness supplies:

- provider interaction;
- turns;
- tool-call handling;
- model context;
- token/time/cost budgets;
- run/session records;
- live execution events.

The relationship is:

```text
Agent SDK
    decides what one governed run means

Harness
    performs the model/tool loop
```

The SDK should be able to use `harness-loop` in process.

---

## 20. AEP integration

For engineering cases, AEP is the natural governor.

AEP provides the SDK with:

- canonical case/work state;
- obligations;
- evidence requirements;
- capability decisions;
- legal transitions;
- explanations;
- durable engineering record.

The SDK should request a frontier rather than reimplement AEP policy.

Conceptually:

```rust
let frontier = governor.frontier(&case, &actor).await?;
```

The SDK can then project that result into Harness actions and model context.

After observations:

```rust
governor.submit_evidence(case_id, evidence).await?;
```

Then:

```rust
let frontier = governor.frontier(&case, &actor).await?;
```

again.

---

## 21. Canon and ELS integration

The generic SDK core should depend on generic protocol concepts, not engineering vocabulary.

Potential architecture:

```text
Canon
    Protocol / Case / Claim / Evidence / Obligation / Action / Outcome
        │
        ▼
Agent SDK Core

ELS
    engineering domain protocols
        │
        ▼
AEP adapter
        │
        ▼
Agent SDK
```

A future generic governor may evaluate Canon directly.

The SDK should therefore make the `Governor` boundary generic:

```rust
#[async_trait]
pub trait Governor {
    async fn load_case(&self, id: CaseId) -> Result<CaseSnapshot>;

    async fn frontier(
        &self,
        case: &CaseSnapshot,
        actor: &ActorContext,
    ) -> Result<Frontier>;

    async fn record_observation(
        &self,
        case: CaseId,
        observation: Observation,
    ) -> Result<()>;

    async fn submit_evidence(
        &self,
        case: CaseId,
        evidence: Evidence,
    ) -> Result<()>;
}
```

AEP can implement this for engineering.

---

## 22. Authority integration

Authority is not a boolean field provided by the model.

The runtime asks an authority provider.

```rust
#[async_trait]
pub trait AuthorityProvider {
    async fn decide(
        &self,
        principal: &Principal,
        capability: &Capability,
        context: &AuthorityContext,
    ) -> Result<AuthorityDecision>;
}
```

Possible decisions:

```rust
pub enum AuthorityDecision {
    Allow(Grant),
    Deny(Refusal),
    ApprovalRequired(ApprovalRequest),
}
```

Mandate is the natural managed implementation.

A local test implementation may be static.

---

## 23. Durable approvals

An approval requirement must be resumable.

Incorrect:

```rust
if dangerous {
    println!("ask user");
}
```

Desired:

```text
Run requests action
    ↓
authority provider returns approval-required
    ↓
assignment becomes durably suspended
    ↓
approval request obtains stable ID
    ↓
process can exit
    ↓
human approves later
    ↓
assignment resumes from exact checkpoint
    ↓
action is revalidated against current case revision
```

Approvals must not bypass revision checking.

If the case changed while waiting, the action must be reevaluated.

---

## 24. Human decision model

Human involvement should be typed.

```rust
pub enum HumanDecisionKind {
    Approval,
    Judgment,
    Review,
    Attestation,
    Selection,
    Exception,
    Escalation,
}
```

Examples:

```text
Approval
    "May this deployment occur?"

Judgment
    "Which interpretation of this ambiguous requirement is correct?"

Review
    "Does this architecture satisfy organizational standards?"

Attestation
    "I independently observed the physical control."

Selection
    "Choose one of these strategic options."

Exception
    "Permit deviation from standard protocol."

Escalation
    "The protocol cannot resolve the situation autonomously."
```

These should not be collapsed into one generic `human_input` callback.

---

## 25. Observation versus evidence

The SDK should enforce this distinction.

### Observation

Something the runtime or an external system reports.

```rust
Observation {
    source,
    subject,
    observed_at,
    payload,
}
```

Examples:

- command exited zero;
- deployment API returned success;
- metric value was 812ms;
- model called a tool;
- Git commit exists.

### Evidence

A validated observation with semantics recognized by the governing protocol.

```rust
Evidence {
    kind,
    subject,
    subject_revision,
    producer,
    observed_at,
    facts,
    provenance,
}
```

Not every observation becomes evidence.

An evidence adapter performs the mapping.

---

## 26. Evidence adapters

Example:

```rust
#[async_trait]
pub trait EvidenceAdapter<O> {
    async fn interpret(
        &self,
        observation: &O,
        context: &EvidenceContext,
    ) -> Result<Vec<Evidence>>;
}
```

Example flow:

```text
GitHub Actions run
    ↓
Connector observation
    ↓
CI evidence adapter
    ↓
test_result evidence
    ↓
Governor
    ↓
claim tests.pass becomes TRUE
```

The adapter can validate:

- producer identity;
- subject revision;
- timestamps;
- result integrity;
- required metadata.

---

## 27. Trace versus evidence

The SDK should document this distinction prominently.

### Trace

Answers:

> What happened inside execution?

May contain:

- prompts;
- model outputs;
- turns;
- tool calls;
- latency;
- token use;
- handoffs;
- guardrails;
- errors.

### Evidence

Answers:

> What observation is admissible for determining case truth?

A trace may produce evidence, for example through an independent trace-conformance verifier.

But raw tracing should never automatically become protocol truth.

---

## 28. Agent definition

An agent should remain comparatively lightweight.

Illustrative:

```rust
let agent = sdk
    .agent("incident-responder")
    .model(ModelRef::new("openai/gpt-6"))
    .instructions(include_str!("incident-responder.md"))
    .competency("engineering.incident-investigation")
    .competency("engineering.production-diagnosis")
    .default_budget(Budget::minutes(30))
    .build();
```

The definition should not embed organizational authority.

This is important:

```text
Agent competency ≠ authority
```

An agent may be technically capable of production diagnosis without being permitted to access one particular production system.

---

## 29. Competencies

Competencies are a possible future workforce primitive.

```rust
pub struct Competency {
    pub id: CompetencyId,
    pub revision: Revision,
}
```

An assignment may require:

```text
engineering.incident-investigation
engineering.database-migration
finance.invoice-review
```

Competency claims could eventually be backed by evaluation evidence.

This enables scheduling questions such as:

```text
Who can perform this assignment?

Who is authorized?

Who is independent from the implementer?

Who has the required competency revision?

Who is available?
```

Competency must not silently become authority.

---

## 30. Multi-agent model

Multi-agent systems should be **case-centric**, not conversation-centric.

Example:

```text
Case CHG-42

Assignment: architecture
    Agent architect-7
    responsible for claims A/B

Assignment: implementation
    Agent engineer-3
    responsible for obligations C/D

Assignment: verification
    Agent verifier-12
    must be independent of engineer-3

Assignment: release
    Agent operator-2
    authority required for production release
```

The protocol may constrain assignments.

Example:

```yaml
obligation:
  implementation.security_reviewed:

    evidence:
      kind: security_review

    producer:
      independent_of:
        - implementation.author
```

This is more precise than a generic handoff graph.

---

## 31. Agent handoff versus assignment transfer

These should remain distinct.

### Handoff

Execution-level delegation:

```text
Agent A asks Agent B to perform a bounded subtask during a run.
```

Harness-level concern.

### Assignment transfer

Organizational responsibility changes:

```text
Case responsibility moves from Agent A to Agent B.
```

This should be governed and recorded.

An assignment transfer may require:

- authority;
- explicit reason;
- current case revision;
- open-obligation acknowledgement.

---

## 32. Local mode

The SDK should be highly usable without managed infrastructure.

Illustrative:

```rust
let sdk = AgentSdk::local()
    .model(openai)
    .case_store(SqliteCaseStore::new("./agent.db"))
    .build()
    .await?;
```

Possible local composition:

```text
Canon evaluator in-process
local ELS protocols
local AEP/governor adapter
Harness in-process
SQLite/filesystem state
static/local authority
local connector adapters
Substrate local confinement
```

Local mode is suitable for:

- development;
- tests;
- command-line workers;
- internal tools;
- desktop applications;
- single-operator automation.

---

## 33. Managed mode

Managed mode may compose services such as:

```text
Agent Platform
AEP Service
Mandate
Identity
Connectors
Secrets
remote Harness workers
Substrate
event infrastructure
```

Illustrative:

```rust
let sdk = AgentSdk::managed(
    Beyond10x::from_env()?
).await?;
```

The same agent/application logic should remain usable.

The differences should largely live behind adapters.

---

## 34. Triggered and long-lived work

A case may be activated by:

- API request;
- schedule;
- webhook;
- event;
- new evidence;
- authority decision;
- human message;
- timeout;
- condition change.

The SDK should therefore avoid assuming:

```text
request -> synchronous run -> final text
```

A better model is:

```text
event
    ↓
load case
    ↓
determine whether an assignment should run
    ↓
run until blocked
    ↓
persist result
    ↓
wait for next event
```

---

## 35. Suspension model

A run may suspend because:

```text
approval required
human judgment required
external evidence required
timer/freshness window
integration unavailable
budget exhausted
no useful admissible action
dependency case incomplete
```

Suspension should be an explicit result, not an exception.

```rust
pub enum SuspensionReason {
    Authority(AuthorityRequest),
    Human(HumanDecisionRequest),
    Evidence(Vec<EvidenceRequirement>),
    Time(WakeupCondition),
    Dependency(Vec<CaseId>),
    Budget(BudgetState),
    ExternalAvailability(ResourceRef),
}
```

---

## 36. Concurrency and stale intent

Every mutating action should be bound to the case revision/frontier on which it was selected.

Conceptually:

```rust
ActionRequest {
    case_id,
    expected_case_revision,
    action_id,
    arguments,
    actor,
    correlation,
}
```

Before execution:

1. reload current canonical case;
2. verify expected revision;
3. reevaluate action admissibility;
4. reevaluate authority where necessary;
5. execute only if still valid.

This prevents a delayed model/tool request from silently acting on a newer case state.

---

## 37. Side-effect discipline

Action execution should follow:

```text
decide
    ↓
authorize
    ↓
reserve / record intent where required
    ↓
execute effect
    ↓
observe result
    ↓
record observation/evidence
    ↓
reevaluate
```

The SDK must not treat a model tool call as proof that the external effect happened.

---

## 38. Idempotency

Long-lived autonomous work requires explicit retry semantics.

A mutating action should support a stable invocation identity where the integration permits it.

```rust
InvocationId
CaseId
ExpectedRevision
ActionId
ArgumentsDigest
Actor
```

Retries with identical intent should be recoverable.

Reuse of the same invocation ID for different intent should be refused.

---

## 39. Failure model

The SDK should make failures typed.

Broad categories:

```text
ModelFailure
GovernorRefusal
AuthorityDenied
ApprovalRequired
RevisionConflict
ActionUnavailable
IntegrationFailure
ExecutionFailure
EvidenceRejected
BudgetExceeded
ProtocolBlocked
PersistenceFailure
InternalInvariantViolation
```

A policy refusal is not an infrastructure error.

An approval requirement is not a failure.

A contradiction in evidence is not an SDK exception.

The type system and public result model should preserve these distinctions.

---

## 40. Security model

The SDK is part of a trusted application shell.

The model must not be allowed to choose:

- its own identity;
- its authority;
- trusted timestamps;
- canonical case state;
- current revisions;
- approval results;
- evidence producer identity;
- connector credentials;
- protocol version;
- organization/tenant scope.

The model may propose:

- action selection;
- action arguments within declared schemas;
- hypotheses;
- plans;
- explanations;
- candidate artifacts.

Trusted runtime code supplies ambient organizational facts.

---

## 41. Secret handling

Secrets should not enter prompts unless explicitly required by a narrowly defined action.

The SDK should prefer:

```text
model chooses semantic action
    ↓
trusted adapter invokes integration
    ↓
credential redeemed at execution boundary
```

rather than:

```text
model receives credential
    ↓
model manually calls arbitrary endpoint
```

This aligns naturally with Connectors, Secrets, and Harness provider boundaries.

---

## 42. Model-provider abstraction

The SDK should not become another large model-provider abstraction if Harness already owns this role.

Public SDK configuration may expose:

```rust
.model("openai/gpt-6")
```

but this should resolve through Harness/model infrastructure.

A lower-level escape hatch may accept a Harness-compatible model port.

---

## 43. Budget model

Budget is part of assignment execution policy.

Possible dimensions:

```text
turns
tokens
cost
wall-clock duration
tool invocations
external effects
model calls
sub-agent assignments
```

Budgets may be:

- hard ceilings;
- soft escalation thresholds;
- protocol-derived;
- assignment-specific.

Budget exhaustion should suspend or terminate according to policy.

---

## 44. Context projection

The agent should not automatically receive the full case history.

A context projector should derive:

```text
objective
current claims
open obligations
current unknowns
relevant artifacts
available actions
blocked actions
authority constraints
recent evidence
protocol guidance
```

This produces a compact working context.

The full durable record remains outside the model context.

---

## 45. Context compaction

Because cases can outlive context windows, the SDK must assume model context is disposable.

After compaction or model replacement, the agent reconstructs working state from canonical case data.

Therefore:

```text
model memory is convenience

case record is authority
```

This principle should be non-negotiable.

---

## 46. Structured outputs

Where a model decision affects runtime behavior, prefer typed structures.

Example:

```rust
pub struct ActionSelection {
    pub action: ActionId,
    pub arguments: serde_json::Value,
    pub rationale: Option<String>,
}
```

The rationale may help humans and tracing.

Only the typed action and validated arguments influence execution.

---

## 47. Developer-defined actions

The SDK should make safe custom actions ergonomic.

Potential derive/macro style:

```rust
#[agent_action(
    id = "crm.customer.lookup",
    effect = "read_only"
)]
async fn lookup_customer(
    ctx: ActionContext,
    input: LookupCustomer,
) -> Result<CustomerObservation> {
    // ...
}
```

The macro may generate:

- JSON Schema;
- descriptor metadata;
- Harness tool adapter;
- tracing spans;
- observation envelope wiring.

It must not infer authority from arbitrary function code.

---

## 48. Action catalogue introspection

Developers and operators should be able to inspect:

```bash
agent-sdk actions list --case INC-492
```

Conceptual output:

```text
ACTION                     STATUS       REASON
metrics.inspect            allowed
logs.search                allowed
release.rollback           approval     production.rollback
database.write             blocked      protocol does not admit action
incident.close             blocked      service.restored = UNKNOWN
```

The same structured result should be available via Rust APIs.

---

## 49. Explainability

Every unavailable action should have a structured reason.

```rust
BlockedAction {
    action: "release.rollback",
    reasons: vec![
        BlockReason::AuthorityRequired("production.rollback"),
    ],
}
```

Or:

```rust
BlockedAction {
    action: "incident.close",
    reasons: vec![
        BlockReason::ClaimUnknown("service.restored"),
        BlockReason::ObligationOpen("verify.customer_impact"),
    ],
}
```

The SDK should make the governance envelope inspectable to both humans and models.

---

## 50. Testing philosophy

The SDK should support two distinct test layers.

### 50.1 Deterministic governance tests

Example:

```rust
#[tokio::test]
async fn stale_test_evidence_does_not_enable_merge() {
    let case = TestCase::from_protocol("software.change/1")
        .implementation("R2")
        .evidence(test_pass("R1"));

    let frontier = case.evaluate().await?;

    assert_eq!(
        frontier.claim("tests.pass"),
        Truth::Unknown
    );

    assert!(!frontier.allows("repository.merge"));

    Ok(())
}
```

### 50.2 Probabilistic agent behavior tests

Example:

```rust
#[tokio::test]
async fn engineer_reobtains_tests_after_revision_change() {
    let scenario = Scenario::builder()
        .agent(engineer())
        .case(fixture("changed-after-tests"))
        .model(FakeModel::scripted([...]))
        .integration(FakeGit::default())
        .build();

    let result = scenario.run().await?;

    result.assert_no_unauthorized_effects();
    result.assert_evidence("test_result");
    result.assert_terminal_outcome("accepted");

    Ok(())
}
```

This separation is important.

A protocol can be correct even if a model behaves poorly.

A model can appear successful even if governance semantics are incorrect.

Both need independent testing.

---

## 51. Simulation

The SDK should make whole-case simulation inexpensive.

```rust
let sim = Simulation::new(protocol)
    .authority(FakeAuthority::allow_all())
    .integration(FakeGithub::default())
    .clock(FakeClock::at(t0))
    .model(ScriptedModel::new(script));

let result = sim.run(case).await?;
```

Simulation should allow:

- deterministic clocks;
- scripted authority;
- fake integrations;
- fake model behavior;
- forced failures;
- concurrent revision changes;
- delayed evidence;
- approval delays.

---

## 52. Conformance

The SDK itself should have conformance suites for adapters.

Examples:

```text
Governor conformance
Authority-provider conformance
Action-adapter conformance
Connector binding conformance
Harness executor conformance
Persistence conformance
Evidence-adapter conformance
```

This matters because a security property implemented only by convention is not portable.

---

## 53. Observability

The SDK should emit OpenTelemetry-compatible spans/events where practical.

Correlation hierarchy:

```text
Case
    Assignment
        Run
            Model Turn
            Action Invocation
            Authority Decision
            Observation
            Evidence Submission
            Governor Evaluation
```

Tracing IDs should not become case identity.

The durable case record should contain stable correlation references.

---

## 54. Audit record

A durable audit record should be able to explain:

```text
which agent revision acted
which principal it represented
which case revision it observed
which protocol governed the work
which action it requested
which authority decision applied
which integration performed the effect
what observation came back
what evidence was admitted
how the frontier changed
```

This is stronger than a model trace.

---

## 55. Developer quickstart target

The SDK's quickstart should demonstrate responsibility rather than novelty tool calling.

Example:

```rust
let worker = sdk
    .agent("expense-reviewer")
    .model("openai/gpt-6")
    .build();

let outcome = sdk
    .assign(worker)
    .new_case(NewCase::new(
        "finance.expense-review/1",
        expense,
    ))
    .run_until_blocked()
    .await?;
```

Then show:

```text
Case EXP-991

Established
  ✓ receipt.present
  ✓ amount.within_policy

Unknown
  ? manager.approved

Admissible
  ✓ inspect_receipt
  ✓ compare_policy
  ✓ request_manager_approval

Unavailable
  ✗ issue_payment
      manager.approved = UNKNOWN

Result
  Waiting for manager approval
```

The developer should feel:

> I did not hand-code the approval state machine, but the agent cannot pay the expense before the protocol permits it.

---

## 56. Engineering quickstart target

For the Beyond10x Engineering product:

```rust
let engineer = sdk
    .agent("autonomous-engineer")
    .model("openai/gpt-6")
    .build();

let assignment = sdk
    .assign(engineer)
    .case("CHG-1842")
    .await?;

assignment.run_until_blocked().await?;
```

The case may derive:

```text
Current objective
    Add enterprise SSO

Open obligations
    system specification
    implementation
    security review
    automated verification

Admissible actions
    inspect_repository
    update_specification
    edit_workspace
    run_tests

Unavailable
    merge
    release
    deploy
```

As evidence accumulates, the frontier expands.

---

## 57. Incident-response example

Initial frontier:

```text
Claims
  incident.declared = TRUE
  impact.bounded = UNKNOWN
  service.healthy = FALSE
  cause.identified = UNKNOWN

Urgent obligation
  mitigate_customer_impact

Actions
  metrics.inspect
  logs.search
  deployment.inspect
  traffic.shift [approval required]
  release.rollback [approval required]
```

After rollback and fresh health evidence:

```text
Claims
  service.healthy = TRUE
  impact.bounded = TRUE
  cause.identified = UNKNOWN

Actions
  timeline.reconstruct
  hypothesis.test
  recent_change.compare

Unavailable conclusion
  postmortem.complete
      cause.identified = UNKNOWN
```

The protocol drives focus without prescribing every investigative step.

---

## 58. Research-agent example

```text
Case:
    EXP-retrieval-042

Claims:
    experiment.valid = TRUE
    candidate.beats_baseline = TRUE
    latency.acceptable = FALSE
    result.reproducible = UNKNOWN

Actions:
    replicate_experiment
    profile_latency
    inspect_retrieval_pipeline
    run_ablation

Unavailable outcome:
    hypothesis.supported

Because:
    result.reproducible = UNKNOWN
    latency.acceptable = FALSE
```

The same SDK model works outside software-change execution.

---

## 59. Comparison with common agent SDK centers of gravity

The goal is not to claim that other frameworks cannot implement similar behavior.

The distinction is what the framework makes **first-class and natural**.

### OpenAI Agents SDK

Its core abstraction is an agent configured with instructions, tools, handoffs, guardrails, and runtime behavior. It includes sessions, human-in-the-loop support, and tracing.

### LangGraph

Its center of gravity is durable, stateful orchestration for long-running agents and workflows, including persistence and human-in-the-loop behavior.

### Rig

Rig provides Rust-native model/provider abstractions, agents, contextual/dynamic tools, hooks, memory, and agent runtime facilities.

### Beyond10x Agent SDK

Its intended center of gravity is:

> **an actor assigned to a durable governed case, operating through dynamically projected actions under explicit authority, where evidence—not model assertion—determines progression and completion.**

A concise comparison:

| Framework category | Primary question |
|---|---|
| Model/agent SDK | How does an agent reason and call tools? |
| Workflow/graph runtime | How does long-running execution proceed? |
| Beyond10x Agent SDK | Under what responsibility, authority, evidence, and protocol may autonomous work proceed? |

---

## 60. Proposed public API modules

Potential façade:

```rust
b10x_agent_sdk::{
    Agent,
    AgentRevision,
    Assignment,
    Case,
    Frontier,
    Action,
    Observation,
    Evidence,
    AuthorityDecision,
    HumanDecision,
    Outcome,
    RunOutcome,
}
```

Adapter modules may be feature-gated:

```rust
b10x_agent_sdk::harness
b10x_agent_sdk::aep
b10x_agent_sdk::canon
b10x_agent_sdk::connectors
b10x_agent_sdk::mandate
b10x_agent_sdk::substrate
b10x_agent_sdk::otel
b10x_agent_sdk::test
```

---

## 61. Proposed repository layout

```text
agent-sdk/
├── Cargo.toml
├── README.md
├── LICENSE
├── b10x.docs.yaml
│
├── crates/
│   ├── agent-sdk-core/
│   ├── agent-sdk-runtime/
│   ├── agent-sdk-action/
│   ├── agent-sdk-evidence/
│   ├── agent-sdk-harness/
│   ├── agent-sdk-governor/
│   ├── agent-sdk-aep/
│   ├── agent-sdk-canon/
│   ├── agent-sdk-connectors/
│   ├── agent-sdk-authority/
│   ├── agent-sdk-mandate/
│   ├── agent-sdk-substrate/
│   ├── agent-sdk-otel/
│   ├── agent-sdk-test/
│   └── b10x-agent-sdk/
│
├── examples/
│   ├── expense-reviewer/
│   ├── incident-responder/
│   ├── research-agent/
│   └── autonomous-engineer/
│
├── conformance/
│
└── website/
```

Most application developers should depend only on:

```toml
[dependencies]
b10x-agent-sdk = "..."
```

The façade should hide internal crate composition.

---

## 62. Crate responsibilities

### `agent-sdk-core`

Pure shared types:

- Agent ID/revision;
- Assignment;
- Case references;
- Frontier;
- action descriptors;
- run outcomes;
- suspension reasons.

No model providers or network IO.

### `agent-sdk-runtime`

The assignment/run coordinator.

Owns:

- load/evaluate/project/run/record cycle;
- stale-revision checking;
- suspension handling;
- orchestration between adapters.

Does not own provider-specific APIs.

### `agent-sdk-action`

Action traits, schemas, dynamic catalogues, bindings, and optional derive macros.

### `agent-sdk-evidence`

Observation and evidence adapter contracts.

### `agent-sdk-harness`

Harness executor adapter.

### `agent-sdk-governor`

Generic governor trait and test conformance.

### `agent-sdk-aep`

AEP implementation of governor integration.

### `agent-sdk-canon`

Direct Canon protocol evaluator integration for generic/local use if needed.

### `agent-sdk-connectors`

Connectors operation bindings.

### `agent-sdk-authority`

Generic authority interfaces and local policies.

### `agent-sdk-mandate`

Mandate authority provider.

### `agent-sdk-substrate`

Bindings for bounded local/external execution.

### `agent-sdk-otel`

Tracing and metrics projection.

### `agent-sdk-test`

Simulation, scripted model, fake integrations, clocks, assertions, and fixtures.

### `b10x-agent-sdk`

Primary façade crate and prelude.

---

## 63. Feature flags

Possible:

```toml
b10x-agent-sdk = {
    version = "...",
    features = [
        "harness",
        "aep",
        "connectors",
        "mandate",
        "otel",
    ],
}
```

Avoid one feature per tiny implementation detail.

Prefer coherent integration bundles.

---

## 64. Minimal viable release

The first SDK release should prove the architecture rather than cover every ecosystem service.

### Required

1. `Agent`
2. `Assignment`
3. durable `CaseRef`
4. generic `Governor` interface
5. `Frontier`
6. dynamic action projection
7. Harness executor
8. one authority interface
9. observations/evidence distinction
10. `run_until_blocked`
11. durable suspension representation
12. scripted testing runtime
13. AEP adapter
14. one Connector action adapter
15. OpenTelemetry correlation

### Explicitly defer

- workforce scheduling;
- competency marketplace;
- arbitrary multi-agent topology language;
- generalized memory framework;
- built-in vector database;
- generic workflow engine;
- new model-provider layer;
- secret store;
- authorization engine;
- custom persistence service.

---

## 65. Release milestones

### M0 — Core types

Implement:

```text
Agent
Assignment
CaseRef
Frontier
Action
RunOutcome
Governor
AuthorityProvider
Observation
Evidence
```

Use fake adapters only.

### M1 — Harness-backed runs

Execute frontier-projected actions through Harness.

Demonstrate:

```text
case -> frontier -> tools -> model -> action -> observation
```

### M2 — AEP engineering case

Run one real `software.change` case against AEP.

Demonstrate stale-evidence invalidation and a blocked release.

### M3 — Durable authority suspension

Integrate an authority/approval provider.

Demonstrate process restart between request and approval.

### M4 — Connectors

Expose a real external connector operation through dynamic action projection.

### M5 — Managed Agent Platform integration

Use stable agent revisions and remote execution/control-plane services.

### M6 — Generic Canon case

Run a non-engineering protocol to prove the SDK is not accidentally software-specific.

---

## 66. Non-goals

The Agent SDK must not become:

- a new model-provider ecosystem;
- a vector database;
- a generic memory product;
- a prompt-management service;
- a workflow engine;
- a protocol compiler;
- an authorization service;
- a credential store;
- a planning store;
- a CI system;
- a deployment service;
- a general integration marketplace;
- a replacement for Harness;
- a replacement for AEP;
- a replacement for Canon/ELS;
- a replacement for Connectors.

It composes these capabilities into a developer-facing runtime.

---

## 67. Architectural boundaries

| Concern | Owner |
|---|---|
| Generic protocol semantics | Canon |
| Engineering protocols | ELS |
| Engineering-case governance | AEP |
| System specification | ESS |
| Model/tool loop | Harness |
| External integrations | Connectors |
| Identity/authentication | Identity |
| Delegation/authorization | Mandate |
| Credentials | Secrets / integration boundary |
| Confined execution | Substrate |
| Generic state kernel | Entity Runtime |
| Generic workflow execution | Workflow |
| Agent application composition | **Agent SDK** |

---

## 68. Design invariants

The following should be treated as architectural invariants.

### I1

A model cannot grant itself authority.

### I2

A model cannot declare its own output to be admissible evidence unless the protocol explicitly permits self-produced evidence of that kind.

### I3

A case persists independently of model session state.

### I4

A mutating action is revalidated against current case revision immediately before execution.

### I5

An action unavailable in the current frontier cannot be invoked through an alternate SDK path.

### I6

A run finishing does not imply a case completing.

### I7

Trace data does not automatically become evidence.

### I8

A human approval is bound to the action/case context it approved.

### I9

Integration credentials are supplied by trusted infrastructure, not the model.

### I10

The SDK does not duplicate domain semantics owned by the governor/protocol.

---

## 69. Open questions

### 69.1 Does the SDK expose `Case` as full data or only a `CaseRef`?

Preference:

Use `CaseRef` publicly for managed cases and expose snapshots explicitly.

Avoid stale mutable case objects.

### 69.2 Where does context projection live?

Possibilities:

- Agent SDK runtime;
- governor;
- domain package;
- agent definition.

Likely answer:

generic mechanism in SDK, domain-specific projection hints from protocol/governor.

### 69.3 How are projected actions described to models?

The SDK needs a stable mapping from:

```text
protocol action
```

to:

```text
Harness tool descriptor
```

Descriptions should avoid leaking hidden governance logic while still helping the model choose correctly.

### 69.4 Should conditional actions be visible before approval?

Different products may choose:

```text
hide until authorized
```

or:

```text
show as available-but-approval-required
```

The SDK should support both without weakening enforcement.

### 69.5 Who owns evidence normalization?

Preference:

The producer/integration adapter should produce an observation.

A domain-specific evidence adapter should interpret it.

The governor validates admissibility.

### 69.6 How generic should `Competency` become?

Do not make workforce scheduling a prerequisite for the first SDK.

### 69.7 Should `Assignment` itself be governed by Canon?

Potentially yes.

Creating, transferring, suspending, or revoking responsibility is itself consequential work.

This may become self-hosting later.

---

## 70. Self-hosting opportunity

The Agent SDK eventually creates a recursive but useful possibility.

An autonomous engineer built with the Agent SDK can work on:

```text
beyond10x/agent-sdk
```

under an ELS software-change protocol governed by AEP.

Therefore:

```text
Agent SDK
    builds an agent

agent
    receives assignment

assignment
    modifies Agent SDK

change
    is governed by AEP / ELS

Agent SDK
    executes the governed work
```

This self-hosting loop is desirable.

It creates a demanding real-world test of:

- revision binding;
- protocol evolution;
- tool authority;
- independent verification;
- compatibility;
- evidence;
- deployment safety.

---

## 71. Success criteria

The SDK is successful if:

1. a developer can build a useful governed agent without manually implementing approval/workflow state machines;
2. the same agent logic works in local and managed execution;
3. tools exposed to the model dynamically reflect current case permissions;
4. stale case state cannot lead to silently executed mutating actions;
5. human approval can suspend and resume work durably;
6. evidence can change protocol truth independently of model claims;
7. one case can survive multiple agents, runs, sessions, and process restarts;
8. a non-engineering protocol can use the same generic runtime;
9. the SDK remains thinner than the services it composes;
10. the developer can explain why every consequential action was available, authorized, executed, and accepted.

---

## 72. Final product statement

### Name

**Beyond10x Agent SDK**

### Tagline

> **Build agents you can give responsibility to.**

### Technical description

> **A Rust SDK for protocol-driven autonomous workers with first-class authority, evidence, durable cases, human escalation, and governed access to real-world systems.**

### Architectural thesis

> **An agent is not merely a model with tools. It is an actor assigned to a governed case.**

### Runtime thesis

> **The model chooses among admissible actions. The protocol determines what is legitimate. Authority determines what may happen. Evidence determines what became true.**

### Broader vision

> **Model intelligence inside a deterministic operating envelope.**

---

## 73. References

This document is a design proposal. The Agent SDK described here does not yet exist as specified. The references below ground statements about current ecosystem boundaries and the center of gravity of adjacent agent frameworks as of 2026-10-04.

### Beyond10x

1. **AEP Overview** — AEP currently decides legal/earned engineering moves, consumes evidence, and resolves capabilities/obligations while explicitly not running models or holding credentials.  
   https://beyond10x.github.io/docs/aep/

2. **Harness Overview** — Harness owns the model/tool loop, tool catalogue, approvals, budgets, and session/run records, including an embeddable Rust library boundary.  
   https://beyond10x.github.io/docs/harness/

3. **Connectors v2** — Connectors provides typed, versioned operations and independent provider adapters for applications and agents.  
   https://beyond10x.github.io/docs/connectors/

4. **Entity Runtime: Connect an agent safely** — illustrates the trusted-shell boundary and requires trusted identity/provenance/revision context rather than model-supplied authority.  
   https://beyond10x.github.io/docs/entity-runtime/guide/agent-integration/

5. **Entity Runtime: Rust libraries** — defines the IO-free deterministic kernel and trusted-shell boundary.  
   https://beyond10x.github.io/docs/entity-runtime/guide/library/

6. **Agent Platform** — current service boundary for stable agents, immutable revisions, capabilities, tasks, evidence, approvals, and triggers.  
   https://beyond10x.github.io/docs/agent-platform/

7. **Beyond10x Public Ecosystem** — generated catalogue of current public project boundaries.  
   https://beyond10x.github.io/ecosystem/

### Adjacent agent frameworks

8. **OpenAI Agents SDK** — centers agents as LLMs with instructions/tools plus handoffs, guardrails, sessions, human-in-the-loop mechanisms, and tracing.  
   https://openai.github.io/openai-agents-python/

9. **OpenAI Agents SDK: Tracing** — traces model generations, tool calls, handoffs, guardrails, and custom events during agent runs.  
   https://openai.github.io/openai-agents-python/tracing/

10. **LangGraph Reference** — describes LangGraph as a low-level orchestration framework for long-running stateful agents with durable execution, persistence, memory, streaming, and human-in-the-loop support.  
    https://langchain-ai.github.io/langgraph/reference/

11. **Rig Rust crate** — Rust-native agent/model tooling, including agent orchestration, providers, contextual tools, hooks, and tool registries.  
    https://docs.rs/rig/latest/rig/

---

## 74. Closing thesis

Autonomous systems become genuinely valuable to organizations when they can be entrusted with outcomes rather than merely prompted for outputs.

That requires more than model intelligence.

It requires a runtime in which:

```text
responsibility is explicit;
authority is external;
actions are bounded;
state is durable;
evidence is first-class;
progress is earned;
human judgment is resumable;
and completion has objective semantics.
```

The Beyond10x Agent SDK should make that operating model feel like a normal Rust application-development primitive.

The intended developer experience is therefore not:

```text
I gave an LLM some functions.
```

It is:

```text
I assigned an autonomous worker a real case,
and the system knows what it may do,
what it still owes,
what has actually been proven,
and when the responsibility has been fulfilled.
```
