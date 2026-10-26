// generated from loom v1
// model digest ab3a9045b7503e022390ae7585a5f18f93fa2b65e8d482b33126c9b72b6c8557
// contract digest 5ff5ff482ca56a9d17ca259eb07404322585c5fcfd49553a38ba3a03930af806
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

/// A2aRefusalReason — `loom.agent.A2aRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2aRefusalReason {
    /// `MissingBearer`.
    MissingBearer,
    /// `WrongBearer`.
    WrongBearer,
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

/// HttpReadRefusalReason — `loom.agent.HttpReadRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpReadRefusalReason {
    /// `MethodNotGet`.
    MethodNotGet,
    /// `PathNotAllowed`.
    PathNotAllowed,
    /// `BearerMissing`.
    BearerMissing,
    /// `Redirected`.
    Redirected,
    /// `FailedStatus`.
    FailedStatus,
}

/// HttpTrustRefusalReason — `loom.agent.HttpTrustRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpTrustRefusalReason {
    /// `Unreadable`.
    Unreadable,
    /// `TooLarge`.
    TooLarge,
    /// `NoCertificate`.
    NoCertificate,
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
    /// `image_repository` — `Optional<String>`.
    pub image_repository: Option<String>,
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

/// RefusalReason — `loom.agent.RefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// `Malformed`.
    Malformed,
    /// `SecretLiteral`.
    SecretLiteral,
    /// `UnknownFormat`.
    UnknownFormat,
    /// `BlankText`.
    BlankText,
    /// `NotAVariableName`.
    NotAVariableName,
    /// `PortOutOfRange`.
    PortOutOfRange,
    /// `NotAPlainPath`.
    NotAPlainPath,
    /// `NotAnInstant`.
    NotAnInstant,
    /// `UnknownProtocol`.
    UnknownProtocol,
    /// `UnknownAction`.
    UnknownAction,
    /// `UnboundSource`.
    UnboundSource,
    /// `HttpMethod`.
    HttpMethod,
    /// `SendWithoutWrites`.
    SendWithoutWrites,
    /// `EmptyChannelAllow`.
    EmptyChannelAllow,
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
    /// `connectors` — `Optional<loom.datasource.ConnectorsCliConfig>`.
    pub connectors: Option<crate::datasource::ConnectorsCliConfig>,
    /// `confinement` — `loom.agent.ConfinementMode`.
    pub confinement: ConfinementMode,
}

/// SendRefusalReason — `loom.agent.SendRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendRefusalReason {
    /// `WritesNone`.
    WritesNone,
    /// `OutsideAllowList`.
    OutsideAllowList,
    /// `NoProposal`.
    NoProposal,
    /// `RateLimited`.
    RateLimited,
}

/// ServeRefusalReason — `loom.agent.ServeRefusalReason`: one of a closed set of names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeRefusalReason {
    /// `NothingToServe`.
    NothingToServe,
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

/// AdmitA2aRequest — the input of `loom.agent.AdmitA2aRequest`.
///
/// Everything it can result in is [`AdmitA2aRequestOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitA2aRequest {
    /// `probe` — `Boolean`.
    pub probe: bool,
    /// `bearer_presented` — `Boolean`.
    pub bearer_presented: bool,
    /// `bearer_matches` — `Boolean`.
    pub bearer_matches: bool,
}

/// Everything `loom.agent.AdmitA2aRequest` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitA2aRequestOutcome {
    /// `missing-bearer` — when `(probe == false and bearer_presented == false)`.
    MissingBearer {
        /// Why it was refused: `loom.agent.A2aRefused`.
        error: A2aRefused,
    },
    /// `wrong-bearer` — when `(probe == false and bearer_matches == false)`.
    WrongBearer {
        /// Why it was refused: `loom.agent.A2aRefused`.
        error: A2aRefused,
    },
    /// `probe` — when `probe == true`.
    Probe,
    /// `admitted` — otherwise.
    Admitted,
}

/// AdmitHttpRead — the input of `loom.agent.AdmitHttpRead`.
///
/// Everything it can result in is [`AdmitHttpReadOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitHttpRead {
    /// `method` — `String`.
    pub method: String,
    /// `path_allowed` — `Boolean`.
    pub path_allowed: bool,
    /// `bearer_missing` — `Boolean`.
    pub bearer_missing: bool,
}

/// Everything `loom.agent.AdmitHttpRead` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitHttpReadOutcome {
    /// `method-not-get` — when `method != GET`.
    MethodNotGet {
        /// Why it was refused: `loom.agent.HttpReadRefused`.
        error: HttpReadRefused,
    },
    /// `path-not-allowed` — when `path_allowed == false`.
    PathNotAllowed {
        /// Why it was refused: `loom.agent.HttpReadRefused`.
        error: HttpReadRefused,
    },
    /// `bearer-missing` — when `bearer_missing == true`.
    BearerMissing {
        /// Why it was refused: `loom.agent.HttpReadRefused`.
        error: HttpReadRefused,
    },
    /// `admitted` — otherwise.
    Admitted,
}

/// AdmitHttpTrust — the input of `loom.agent.AdmitHttpTrust`.
///
/// Everything it can result in is [`AdmitHttpTrustOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitHttpTrust {
    /// `named` — `Boolean`.
    pub named: bool,
    /// `read` — `Boolean`.
    pub read: bool,
    /// `bytes` — `Integer`.
    pub bytes: i64,
    /// `certificates` — `Integer`.
    pub certificates: i64,
}

/// Everything `loom.agent.AdmitHttpTrust` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitHttpTrustOutcome {
    /// `unreadable` — when `(named == true and read == false)`.
    Unreadable {
        /// Why it was refused: `loom.agent.HttpTrustRefused`.
        error: HttpTrustRefused,
    },
    /// `too-large` — when `(named == true and bytes > 1048576)`.
    TooLarge {
        /// Why it was refused: `loom.agent.HttpTrustRefused`.
        error: HttpTrustRefused,
    },
    /// `no-certificate` — when `(named == true and certificates <= 0)`.
    NoCertificate {
        /// Why it was refused: `loom.agent.HttpTrustRefused`.
        error: HttpTrustRefused,
    },
    /// `webpki-only` — when `named == false`.
    WebpkiOnly,
    /// `extended` — otherwise.
    Extended,
}

/// AdmitSend — the input of `loom.agent.AdmitSend`.
///
/// Everything it can result in is [`AdmitSendOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitSend {
    /// `mode` — `loom.agent.WriteMode`.
    pub mode: WriteMode,
    /// `channel_allowed` — `Boolean`.
    pub channel_allowed: bool,
    /// `proposed` — `Boolean`.
    pub proposed: bool,
    /// `granted` — `Boolean`.
    pub granted: bool,
    /// `sent_in_window` — `Integer`.
    pub sent_in_window: i64,
    /// `max_per_hour` — `Integer`.
    pub max_per_hour: i64,
}

/// Everything `loom.agent.AdmitSend` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitSendOutcome {
    /// `writes-none` — when `mode == none`.
    WritesNone {
        /// Why it was refused: `loom.agent.SendRefused`.
        error: SendRefused,
    },
    /// `outside-allow-list` — when `channel_allowed == false`.
    OutsideAllowList {
        /// Why it was refused: `loom.agent.SendRefused`.
        error: SendRefused,
    },
    /// `no-proposal` — when `proposed == false`.
    NoProposal {
        /// Why it was refused: `loom.agent.SendRefused`.
        error: SendRefused,
    },
    /// `rate-limited` — when `sent_in_window >= {fact: max_per_hour}`.
    RateLimited {
        /// Why it was refused: `loom.agent.SendRefused`.
        error: SendRefused,
    },
    /// `held` — when `(mode == approval and granted == false)`.
    Held,
    /// `admitted` — otherwise.
    Admitted,
}

/// AdmitServe — the input of `loom.agent.AdmitServe`.
///
/// Everything it can result in is [`AdmitServeOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitServe {
    /// `channel` — `Boolean`.
    pub channel: bool,
    /// `a2a` — `Boolean`.
    pub a2a: bool,
}

/// Everything `loom.agent.AdmitServe` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmitServeOutcome {
    /// `nothing-to-serve` — when `(channel == false and a2a == false)`.
    NothingToServe {
        /// Why it was refused: `loom.agent.ServeRefused`.
        error: ServeRefused,
    },
    /// `polls-and-answers` — when `(channel == true and a2a == true)`.
    PollsAndAnswers,
    /// `polls` — when `channel == true`.
    Polls,
    /// `answers` — otherwise.
    Answers,
}

/// BoundHttpAnswer — the input of `loom.agent.BoundHttpAnswer`.
///
/// Everything it can result in is [`BoundHttpAnswerOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundHttpAnswer {
    /// `status` — `Integer`.
    pub status: i64,
    /// `bytes` — `Integer`.
    pub bytes: i64,
    /// `max_bytes` — `Integer`.
    pub max_bytes: i64,
}

/// Everything `loom.agent.BoundHttpAnswer` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundHttpAnswerOutcome {
    /// `redirected` — when `(status >= 300 and status <= 399)`.
    Redirected {
        /// Why it was refused: `loom.agent.HttpReadRefused`.
        error: HttpReadRefused,
    },
    /// `failed-status` — when `(status <= 199 or status >= 400)`.
    FailedStatus {
        /// Why it was refused: `loom.agent.HttpReadRefused`.
        error: HttpReadRefused,
    },
    /// `cut` — when `bytes > {fact: max_bytes}`.
    Cut,
    /// `whole` — otherwise.
    Whole,
}

/// CheckAgentSpec — the input of `loom.agent.CheckAgentSpec`.
///
/// Everything it can result in is [`CheckAgentSpecOutcome`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckAgentSpec {
    /// `path` — `String`.
    pub path: String,
}

/// Everything `loom.agent.CheckAgentSpec` can result in — one variant per declared outcome.
///
/// An infrastructure failure is deliberately not in here: a refusal is a fact about the domain,
/// a transport fault is a fact about the run, and conflating the two is what the declared
/// outcomes exist to prevent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckAgentSpecOutcome {
    /// `malformed` — externally decided (the document at the path does not decode as a loom.agent/1 document).
    Malformed {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `secret-literal` — externally decided (a string of the document holds a credential).
    SecretLiteral {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `unknown-format` — externally decided (the document's format is not loom.agent/1).
    UnknownFormat {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `blank-text` — externally decided (a text member of the document is empty or only whitespace).
    BlankText {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `not-a-variable-name` — externally decided (a bearer_env of the document is not an environment variable name).
    NotAVariableName {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `port-out-of-range` — externally decided (a port of the document is outside 1 to 65535).
    PortOutOfRange {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `not-a-plain-path` — externally decided (an allow_paths entry of an http binding is not a plain path).
    NotAPlainPath {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `not-an-instant` — externally decided (a Timestamp member of the document is not an RFC 3339 instant).
    NotAnInstant {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `unknown-protocol` — externally decided (the inner loop names a protocol the plugin catalog does not hold, or states are declared with no inner loop).
    UnknownProtocol {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `unknown-action` — externally decided (a state names an action the inner loop's protocol does not declare).
    UnknownAction {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `unbound-source` — externally decided (a state, the outer loop or the channel names a source no binding declares).
    UnboundSource {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `http-method` — externally decided (an http binding's method is not GET).
    HttpMethod {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `send-without-writes` — externally decided (reply.send is a capability while writes are none or undeclared).
    SendWithoutWrites {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `empty-channel-allow` — externally decided (the channel's allow list is empty).
    EmptyChannelAllow {
        /// Why it was refused: `loom.agent.SpecRefused`.
        error: SpecRefused,
    },
    /// `serveable` — otherwise.
    Serveable,
}

/// The declared error `loom.agent.A2aRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A2aRefused {
    /// `reason` — `loom.agent.A2aRefusalReason`.
    pub reason: A2aRefusalReason,
}

/// The declared error `loom.agent.HttpReadRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReadRefused {
    /// `reason` — `loom.agent.HttpReadRefusalReason`.
    pub reason: HttpReadRefusalReason,
}

/// The declared error `loom.agent.HttpTrustRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpTrustRefused {
    /// `reason` — `loom.agent.HttpTrustRefusalReason`.
    pub reason: HttpTrustRefusalReason,
}

/// The declared error `loom.agent.SendRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendRefused {
    /// `reason` — `loom.agent.SendRefusalReason`.
    pub reason: SendRefusalReason,
    /// `final` — `Boolean`.
    pub r#final: bool,
}

/// The declared error `loom.agent.ServeRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeRefused {
    /// `reason` — `loom.agent.ServeRefusalReason`.
    pub reason: ServeRefusalReason,
}

/// The declared error `loom.agent.SpecRefused`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRefused {
    /// `reason` — `loom.agent.RefusalReason`.
    pub reason: RefusalReason,
    /// `member` — `String`.
    pub member: String,
    /// `detail` — `String`.
    pub detail: String,
}

/// What this bounded context owes its implementor, and the seams of what is generated.
///
/// One trait per obligation in the synthesis plan, each carrying the plan's own contract, and one
/// per generated behaviour, which [`Generated`](crate::behaviour::Generated) implements.
/// [`Unimplemented`](obligations::Unimplemented) satisfies every owed trait by refusing in the type system.
pub mod obligations {
    /// The behaviour `loom.agent.AdmitA2aRequest` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait AdmitA2aRequestBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.AdmitA2aRequest`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn admit_a2a_request(&mut self, input: super::AdmitA2aRequest) -> Result<super::AdmitA2aRequestOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.AdmitHttpRead` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait AdmitHttpReadBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.AdmitHttpRead`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn admit_http_read(&mut self, input: super::AdmitHttpRead) -> Result<super::AdmitHttpReadOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.AdmitHttpTrust` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait AdmitHttpTrustBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.AdmitHttpTrust`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn admit_http_trust(&mut self, input: super::AdmitHttpTrust) -> Result<super::AdmitHttpTrustOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.AdmitSend` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait AdmitSendBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.AdmitSend`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn admit_send(&mut self, input: super::AdmitSend) -> Result<super::AdmitSendOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.AdmitServe` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait AdmitServeBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.AdmitServe`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn admit_serve(&mut self, input: super::AdmitServe) -> Result<super::AdmitServeOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.BoundHttpAnswer` — generated.
    ///
    /// The specification fully determines it: [`crate::behaviour::Generated`] implements it
    /// over the storage and context ports. Implement it yourself to replace that behaviour.
    pub trait BoundHttpAnswerBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.BoundHttpAnswer`.
        ///
        /// `Err` is the typed refusal of a request the model declares no outcome for.
        fn bound_http_answer(&mut self, input: super::BoundHttpAnswer) -> Result<super::BoundHttpAnswerOutcome, crate::obligation::UnmetObligation>;
    }

    /// The behaviour `loom.agent.CheckAgentSpec` — an implementation obligation.
    ///
    /// Why it is not generated: kept an obligation by the fields of error `loom.agent.SpecRefused`, which the specification gives no source, in `malformed`.
    ///
    /// Contract: given `loom.agent.CheckAgentSpec` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `malformed` externally decided (the document at the path does not decode as a loom.agent/1 document), error `loom.agent.SpecRefused`; `secret-literal` externally decided (a string of the document holds a credential), error `loom.agent.SpecRefused`; `unknown-format` externally decided (the document's format is not loom.agent/1), error `loom.agent.SpecRefused`; `blank-text` externally decided (a text member of the document is empty or only whitespace), error `loom.agent.SpecRefused`; `not-a-variable-name` externally decided (a bearer_env of the document is not an environment variable name), error `loom.agent.SpecRefused`; `port-out-of-range` externally decided (a port of the document is outside 1 to 65535), error `loom.agent.SpecRefused`; `not-a-plain-path` externally decided (an allow_paths entry of an http binding is not a plain path), error `loom.agent.SpecRefused`; `not-an-instant` externally decided (a Timestamp member of the document is not an RFC 3339 instant), error `loom.agent.SpecRefused`; `unknown-protocol` externally decided (the inner loop names a protocol the plugin catalog does not hold, or states are declared with no inner loop), error `loom.agent.SpecRefused`; `unknown-action` externally decided (a state names an action the inner loop's protocol does not declare), error `loom.agent.SpecRefused`; `unbound-source` externally decided (a state, the outer loop or the channel names a source no binding declares), error `loom.agent.SpecRefused`; `http-method` externally decided (an http binding's method is not GET), error `loom.agent.SpecRefused`; `send-without-writes` externally decided (reply.send is a capability while writes are none or undeclared), error `loom.agent.SpecRefused`; `empty-channel-allow` externally decided (the channel's allow list is empty), error `loom.agent.SpecRefused`; `serveable` otherwise.
    pub trait CheckAgentSpecBehavior {
        /// Decides and enacts exactly one declared outcome of `loom.agent.CheckAgentSpec`.
        ///
        /// `Err` is the typed refusal of an obligation nothing has satisfied; a satisfying
        /// implementation never returns it.
        fn check_agent_spec(&mut self, input: super::CheckAgentSpec) -> Result<super::CheckAgentSpecOutcome, crate::obligation::UnmetObligation>;
    }

    /// Every obligation of this bounded context, refused in the type system.
    ///
    /// Each method returns the typed refusal naming what is owed — never a panic, never a guessed
    /// value — so a workspace built on this stub compiles and reports its own gaps.
    pub struct Unimplemented;

    impl CheckAgentSpecBehavior for Unimplemented {
        fn check_agent_spec(&mut self, _input: super::CheckAgentSpec) -> Result<super::CheckAgentSpecOutcome, crate::obligation::UnmetObligation> {
            Err(crate::obligation::UnmetObligation { capability: "command behaviour", source: "loom.agent.CheckAgentSpec" })
        }
    }
}
