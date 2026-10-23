// generated from loom v1
// model digest b774beb53c36941f484cfdcfb927b46287a34f934f9b43c3d7e49306ebe94f34
// contract digest 7b9385bd4805d85156d0039e947a53ea05aeda4435f2bebd60af8072091657ce
// do not edit: regenerate with `ess synthesize --layout crate`

//! Agent — `loom.agent`.
//!
//! An agent as data: one `loom.agent/1` document naming the agent and its brief, its models, the harness session it may run (kind, run budget, stop policy, confinement and tool policy), its outer and inner loop, the states an item is projected by, the bindings it reads and writes through, its channel, write policy, capabilities, exposure, runtime and deployment.
//!
//! Everything this bounded context declares that the synthesis plan marks generated.

/// A2aCard — `loom.agent.A2aCard`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A2aCard {
    /// `name` — `String`.
    pub name: String,
    /// `description` — `String`.
    pub description: String,
    /// `skills` — `List<String>`.
    pub skills: Vec<String>,
}

/// A2aExposure — `loom.agent.A2aExposure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A2aExposure {
    /// `port` — `Integer`.
    pub port: i64,
    /// `path` — `String`.
    pub path: String,
    /// `card` — `loom.agent.A2aCard`.
    pub card: A2aCard,
}

/// ActionBudget — `loom.agent.ActionBudget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBudget {
    /// `action` — `String`.
    pub action: String,
    /// `max_trials` — `Optional<Integer>`.
    pub max_trials: Option<i64>,
    /// `max_tokens_in` — `Optional<Integer>`.
    pub max_tokens_in: Option<i64>,
    /// `max_tokens_out` — `Optional<Integer>`.
    pub max_tokens_out: Option<i64>,
    /// `max_duration_ms` — `Optional<Integer>`.
    pub max_duration_ms: Option<i64>,
    /// `max_cost_micro_usd` — `Optional<Integer>`.
    pub max_cost_micro_usd: Option<i64>,
}

/// AgentId — `loom.agent.AgentId`: a distinct wrapper around `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentId(pub String);

/// AgentIdentity — `loom.agent.AgentIdentity`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    /// `id` — `loom.agent.AgentId`.
    pub id: AgentId,
    /// `name` — `String`.
    pub name: String,
    /// `brief` — `String`.
    pub brief: String,
}

/// AgentSpec — `loom.agent.AgentSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    /// `format` — `String`.
    pub format: String,
    /// `agent` — `loom.agent.AgentIdentity`.
    pub agent: AgentIdentity,
    /// `model` — `loom.models.ModelsConfig`.
    pub model: crate::models::ModelsConfig,
    /// `session` — `Optional<loom.agent.SessionSpec>`.
    pub session: Option<SessionSpec>,
    /// `loops` — `Optional<loom.agent.Loops>`.
    pub loops: Option<Loops>,
    /// `states` — `List<loom.agent.AgentState>`.
    pub states: Vec<AgentState>,
    /// `bindings` — `List<loom.agent.Binding>`.
    pub bindings: Vec<Binding>,
    /// `channel` — `Optional<loom.agent.ChannelSpec>`.
    pub channel: Option<ChannelSpec>,
    /// `writes` — `Optional<loom.agent.WritesSpec>`.
    pub writes: Option<WritesSpec>,
    /// `policy` — `Optional<loom.agent.PolicySpec>`.
    pub policy: Option<PolicySpec>,
    /// `expose` — `Optional<loom.agent.ExposeSpec>`.
    pub expose: Option<ExposeSpec>,
    /// `runtime` — `Optional<loom.agent.RuntimeSpec>`.
    pub runtime: Option<RuntimeSpec>,
    /// `deploy` — `Optional<loom.agent.DeploySpec>`.
    pub deploy: Option<DeploySpec>,
}

/// AgentState — `loom.agent.AgentState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentState {
    /// `name` — `String`.
    pub name: String,
    /// `when` — `loom.agent.StateWhen`.
    pub when: StateWhen,
    /// `actions` — `List<String>`.
    pub actions: Vec<String>,
    /// `sources` — `List<loom.datasource.SourceName>`.
    pub sources: Vec<crate::datasource::SourceName>,
}

/// Binding — `loom.agent.Binding`: one of a fixed set of shapes, tagged on the wire by `kind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// Tagged `connectors` — `loom.agent.ConnectorsBinding`.
    Connectors(ConnectorsBinding),
    /// Tagged `http` — `loom.agent.HttpBinding`.
    Http(HttpBinding),
}

/// BreachStep — `loom.agent.BreachStep`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreachStep {
    /// `DiscardStep`.
    DiscardStep,
    /// `KeepStep`.
    KeepStep,
}

/// BudgetLimit — `loom.agent.BudgetLimit`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetLimit {
    /// `MaxSteps`.
    MaxSteps,
    /// `MaxTrials`.
    MaxTrials,
    /// `MaxTokensIn`.
    MaxTokensIn,
    /// `MaxTokensOut`.
    MaxTokensOut,
    /// `MaxDurationMs`.
    MaxDurationMs,
    /// `MaxCostMicroUsd`.
    MaxCostMicroUsd,
    /// `MaxTokens`.
    MaxTokens,
    /// `Deadline`.
    Deadline,
}

/// ChannelSpec — `loom.agent.ChannelSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSpec {
    /// `binding` — `loom.datasource.SourceName`.
    pub binding: crate::datasource::SourceName,
    /// `allow` — `List<String>`.
    pub allow: Vec<String>,
    /// `bot_user_id` — `String`.
    pub bot_user_id: String,
}

/// ConfinementMode — `loom.agent.ConfinementMode`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfinementMode {
    /// `none`.
    None,
}

/// ConnectorsBinding — `loom.agent.ConnectorsBinding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorsBinding {
    /// `name` — `loom.datasource.SourceName`.
    pub name: crate::datasource::SourceName,
    /// `adapter` — `loom.datasource.AdapterAlias`.
    pub adapter: crate::datasource::AdapterAlias,
    /// `connection` — `loom.datasource.ConnectionId`.
    pub connection: crate::datasource::ConnectionId,
    /// `list` — `Optional<loom.datasource.OperationId>`.
    pub list: Option<crate::datasource::OperationId>,
    /// `search` — `Optional<loom.datasource.OperationId>`.
    pub search: Option<crate::datasource::OperationId>,
    /// `get` — `Optional<loom.datasource.OperationId>`.
    pub get: Option<crate::datasource::OperationId>,
    /// `write` — `Optional<loom.datasource.OperationId>`.
    pub write: Option<crate::datasource::OperationId>,
}

/// DecisionMode — `loom.agent.DecisionMode`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionMode {
    /// `frame`.
    Frame,
    /// `ask`.
    Ask,
    /// `observe`.
    Observe,
}

/// DeploySpec — `loom.agent.DeploySpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploySpec {
    /// `kubernetes` — `Optional<loom.agent.KubernetesDeploy>`.
    pub kubernetes: Option<KubernetesDeploy>,
}

/// EgressAperture — `loom.agent.EgressAperture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressAperture {
    /// `name` — `String`.
    pub name: String,
    /// `host` — `String`.
    pub host: String,
    /// `port` — `Integer`.
    pub port: i64,
    /// `max_bytes` — `Optional<Integer>`.
    pub max_bytes: Option<i64>,
}

/// ExposeSpec — `loom.agent.ExposeSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposeSpec {
    /// `a2a` — `Optional<loom.agent.A2aExposure>`.
    pub a2a: Option<A2aExposure>,
}

/// HttpBinding — `loom.agent.HttpBinding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpBinding {
    /// `name` — `loom.datasource.SourceName`.
    pub name: crate::datasource::SourceName,
    /// `base_url` — `String`.
    pub base_url: String,
    /// `allow_paths` — `List<String>`.
    pub allow_paths: Vec<String>,
    /// `method` — `String`.
    pub method: String,
    /// `bearer_env` — `Optional<String>`.
    pub bearer_env: Option<String>,
    /// `max_bytes` — `Integer`.
    pub max_bytes: i64,
}

/// InnerLoop — `loom.agent.InnerLoop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerLoop {
    /// `protocol` — `String`.
    pub protocol: String,
    /// `step_budget` — `Integer`.
    pub step_budget: i64,
    /// `model_budget` — `Integer`.
    pub model_budget: i64,
    /// `classify_threshold` — `Optional<Decimal>`.
    pub classify_threshold: Option<crate::primitives::Decimal>,
}

/// KubernetesDeploy — `loom.agent.KubernetesDeploy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesDeploy {
    /// `namespace` — `String`.
    pub namespace: String,
    /// `service_account` — `String`.
    pub service_account: String,
    /// `image_repository` — `String`.
    pub image_repository: String,
    /// `resources` — `loom.agent.KubernetesResources`.
    pub resources: KubernetesResources,
    /// `egress` — `List<loom.agent.KubernetesEgress>`.
    pub egress: Vec<KubernetesEgress>,
    /// `ingress` — `List<loom.agent.KubernetesIngress>`.
    pub ingress: Vec<KubernetesIngress>,
}

/// KubernetesEgress — `loom.agent.KubernetesEgress`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesEgress {
    /// `name` — `String`.
    pub name: String,
    /// `host` — `Optional<String>`.
    pub host: Option<String>,
    /// `namespace` — `Optional<String>`.
    pub namespace: Option<String>,
    /// `selector` — `List<loom.agent.Label>`.
    pub selector: Vec<Label>,
    /// `port` — `Integer`.
    pub port: i64,
}

/// KubernetesIngress — `loom.agent.KubernetesIngress`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesIngress {
    /// `name` — `String`.
    pub name: String,
    /// `from_namespace` — `String`.
    pub from_namespace: String,
    /// `port` — `Integer`.
    pub port: i64,
}

/// KubernetesResources — `loom.agent.KubernetesResources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesResources {
    /// `cpu` — `String`.
    pub cpu: String,
    /// `memory` — `String`.
    pub memory: String,
}

/// Label — `loom.agent.Label`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// `name` — `String`.
    pub name: String,
    /// `value` — `String`.
    pub value: String,
}

/// LimitBreachStep — `loom.agent.LimitBreachStep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitBreachStep {
    /// `limit` — `loom.agent.BudgetLimit`.
    pub limit: BudgetLimit,
    /// `step` — `loom.agent.BreachStep`.
    pub step: BreachStep,
}

/// Loops — `loom.agent.Loops`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loops {
    /// `outer` — `loom.agent.OuterLoop`.
    pub outer: OuterLoop,
    /// `inner` — `loom.agent.InnerLoop`.
    pub inner: InnerLoop,
}

/// OuterLoop — `loom.agent.OuterLoop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuterLoop {
    /// `poll_interval_seconds` — `Integer`.
    pub poll_interval_seconds: i64,
    /// `sources` — `List<loom.datasource.SourceName>`.
    pub sources: Vec<crate::datasource::SourceName>,
    /// `min_age_minutes` — `Integer`.
    pub min_age_minutes: i64,
    /// `lookback_minutes` — `Optional<Integer>`.
    pub lookback_minutes: Option<i64>,
}

/// PolicySpec — `loom.agent.PolicySpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySpec {
    /// `capabilities` — `List<String>`.
    pub capabilities: Vec<String>,
}

/// RunBudget — `loom.agent.RunBudget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunBudget {
    /// `max_steps` — `Optional<Integer>`.
    pub max_steps: Option<i64>,
    /// `max_tokens_in` — `Optional<Integer>`.
    pub max_tokens_in: Option<i64>,
    /// `max_tokens_out` — `Optional<Integer>`.
    pub max_tokens_out: Option<i64>,
    /// `max_duration_ms` — `Optional<Integer>`.
    pub max_duration_ms: Option<i64>,
    /// `max_cost_micro_usd` — `Optional<Integer>`.
    pub max_cost_micro_usd: Option<i64>,
    /// `max_tokens` — `Optional<Integer>`.
    pub max_tokens: Option<i64>,
    /// `deadline` — `Optional<Timestamp>`.
    pub deadline: Option<crate::primitives::Timestamp>,
    /// `per_action` — `List<loom.agent.ActionBudget>`.
    pub per_action: Vec<ActionBudget>,
}

/// RuntimeSpec — `loom.agent.RuntimeSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSpec {
    /// `state_dir` — `String`.
    pub state_dir: String,
    /// `connectors` — `loom.datasource.ConnectorsCliConfig`.
    pub connectors: crate::datasource::ConnectorsCliConfig,
    /// `confinement` — `loom.agent.ConfinementMode`.
    pub confinement: ConfinementMode,
}

/// SessionSpec — `loom.agent.SessionSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpec {
    /// `kind` — `loom.session.SessionKind`.
    pub kind: crate::session::SessionKind,
    /// `budget` — `loom.agent.RunBudget`.
    pub budget: RunBudget,
    /// `stop` — `loom.agent.StopPolicy`.
    pub stop: StopPolicy,
    /// `confinement` — `loom.agent.ConfinementMode`.
    pub confinement: ConfinementMode,
    /// `tools` — `loom.agent.ToolPolicy`.
    pub tools: ToolPolicy,
}

/// StateWhen — `loom.agent.StateWhen`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWhen {
    /// `intent` — `loom.plugin.Intent`.
    pub intent: crate::plugin::Intent,
}

/// StopPolicy — `loom.agent.StopPolicy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopPolicy {
    /// `approval_gate` — `Boolean`.
    pub approval_gate: bool,
    /// `idle_bound` — `Optional<Integer>`.
    pub idle_bound: Option<i64>,
    /// `on_breach` — `loom.agent.BreachStep`.
    pub on_breach: BreachStep,
    /// `breach_by_limit` — `List<loom.agent.LimitBreachStep>`.
    pub breach_by_limit: Vec<LimitBreachStep>,
}

/// ToolPolicy — `loom.agent.ToolPolicy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPolicy {
    /// `allow` — `List<String>`.
    pub allow: Vec<String>,
    /// `deny` — `List<String>`.
    pub deny: Vec<String>,
    /// `egress` — `List<loom.agent.EgressAperture>`.
    pub egress: Vec<EgressAperture>,
    /// `decisions` — `loom.agent.DecisionMode`.
    pub decisions: DecisionMode,
}

/// WriteMode — `loom.agent.WriteMode`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// `auto`.
    Auto,
    /// `approval`.
    Approval,
    /// `none`.
    None,
}

/// WritesSpec — `loom.agent.WritesSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritesSpec {
    /// `mode` — `loom.agent.WriteMode`.
    pub mode: WriteMode,
    /// `max_per_hour` — `Integer`.
    pub max_per_hour: i64,
}
