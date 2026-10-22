//! An agent as data: the `loom.agent` domain's generated types and the decoder of one
//! `loom.agent/1` document.
//!
//! [`AgentSpec`] is the typed form of the document: who the agent is, its models (exactly a
//! [`crate::models::ModelsConfig`]), the harness session it may run with its [`ToolPolicy`], its
//! loops, states, bindings, channel, write policy, capabilities, exposure, runtime and
//! deployment. [`parse`] decodes YAML or JSON (JSON is read as YAML) into it. A member the domain
//! does not declare is refused, naming where it stands, so a misspelt key is never ignored. A
//! required member that is missing, a value of the wrong type and an enum spelling the domain does
//! not declare are refused the same way. An optional member may be absent or `null`; a list may
//! be absent and is then empty. A decimal is a number or a string spelling one. `format` must be
//! `loom.agent/1`. Text is never empty or blank, no value carries a YAML tag, a `bearer_env` is
//! an environment variable name (never a token, and a refusal does not repeat it) and a port is
//! 1 to 65535. A document holding more than [`MAX_FLOW_OPENERS`] `[` or `{` is refused before
//! it is read.
//!
//! Decoding checks the shape only. Whether a spec can be served (a state naming an action its
//! protocol lacks, a source no binding declares) is a later check's.

use std::collections::BTreeMap;
use std::fmt;

use serde_yaml_ng::Value;

/// The types of other domains an [`AgentSpec`] holds, so a host builds one in code through this
/// module alone.
pub use models_config::datasource::{
    AdapterAlias, ConnectionId, ConnectorsCliConfig, OperationId, SourceName,
};
pub use models_config::plugin::Intent;
pub use models_config::primitives::{Decimal, Timestamp};
pub use models_config::session::SessionKind;

/// The generated types of the `loom.agent` domain (`ess/domains/agent.yaml`).
pub use models_config::agent::{
    A2aCard, A2aExposure, ActionBudget, AgentId, AgentIdentity, AgentSpec, AgentState, Binding,
    BreachStep, BudgetLimit, ChannelSpec, ConfinementMode, ConnectorsBinding, DecisionMode,
    DeploySpec, EgressAperture, ExposeSpec, HttpBinding, InnerLoop, KubernetesDeploy,
    KubernetesEgress, KubernetesIngress, KubernetesResources, Label, LimitBreachStep, Loops,
    OuterLoop, PolicySpec, RunBudget, RuntimeSpec, SessionSpec, StateWhen, StopPolicy, ToolPolicy,
    WriteMode, WritesSpec,
};

/// The one format a document declares.
pub const FORMAT: &str = "loom.agent/1";

/// Why a document was refused; the message names the member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpecError {
    message: String,
}

impl AgentSpecError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AgentSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AgentSpecError {}

/// Decodes one `loom.agent/1` document, YAML or JSON.
///
/// # Errors
/// The text exceeds its byte bound, is not YAML or JSON, expands past its bound through YAML
/// aliases, declares another format, or does not decode into [`AgentSpec`]: a member the domain
/// does not declare, a required member missing, a value of the wrong type, or an enum spelling it
/// does not declare. The message names the member.
pub fn parse(text: &str) -> Result<AgentSpec, AgentSpecError> {
    if u64::try_from(text.len()).map_or(true, |length| length > crate::models::MAX_BYTES) {
        return Err(AgentSpecError::new("the document exceeds its byte bound"));
    }
    if flow_openers(text) > MAX_FLOW_OPENERS {
        return Err(AgentSpecError::new(format!(
            "the document opens more than {MAX_FLOW_OPENERS} flow collections"
        )));
    }
    crate::models::bound_aliases(text).map_err(|error| AgentSpecError::new(error.to_string()))?;
    let value: Value = serde_yaml_ng::from_str(text)
        .map_err(|error| AgentSpecError::new(format!("invalid YAML or JSON: {error}")))?;
    untagged(&value, "")?;
    spec(&value)
}

/// How many `[` and `{` a document may hold. The YAML reader's cost grows with the square of its
/// flow nesting, so a deeply nested document is refused before it is read. Every such byte
/// counts, quoted or in a comment too, so nothing can hide an opener; a real spec holds a few
/// dozen.
pub const MAX_FLOW_OPENERS: usize = 4096;

fn flow_openers(text: &str) -> usize {
    text.bytes()
        .filter(|byte| matches!(byte, b'[' | b'{'))
        .count()
}

/// Refuses a YAML tag on any value: the domain declares none, and nothing is ignored.
fn untagged(value: &Value, at: &str) -> Result<(), AgentSpecError> {
    match value {
        Value::Tagged(_) => Err(AgentSpecError::new(format!(
            "`{}` carries a YAML tag",
            if at.is_empty() { "the document" } else { at }
        ))),
        Value::Sequence(items) => items
            .iter()
            .enumerate()
            .try_for_each(|(index, item)| untagged(item, &format!("{at}[{index}]"))),
        Value::Mapping(mapping) => mapping.iter().try_for_each(|(key, item)| {
            let name = key.as_str().map_or_else(|| "?".to_owned(), str::to_owned);
            untagged(key, &join(at, &name))?;
            untagged(item, &join(at, &name))
        }),
        _ => Ok(()),
    }
}

/// The members of one mapping, every one declared.
struct Members<'a> {
    at: String,
    members: BTreeMap<&'a str, &'a Value>,
}

impl<'a> Members<'a> {
    fn of(value: &'a Value, at: &str, declared: &[&str]) -> Result<Self, AgentSpecError> {
        let mapping = value
            .as_mapping()
            .ok_or_else(|| AgentSpecError::new(format!("`{at}` is not a mapping")))?;
        let mut members = BTreeMap::new();
        for (key, value) in mapping {
            let key = key.as_str().ok_or_else(|| {
                AgentSpecError::new(format!("`{at}` holds a key that is not text"))
            })?;
            if !declared.contains(&key) {
                return Err(AgentSpecError::new(format!(
                    "unknown member `{}`",
                    join(at, key)
                )));
            }
            members.insert(key, value);
        }
        Ok(Self {
            at: at.to_owned(),
            members,
        })
    }

    fn path(&self, name: &str) -> String {
        join(&self.at, name)
    }

    fn required(&self, name: &str) -> Result<(&'a Value, String), AgentSpecError> {
        let at = self.path(name);
        match self.members.get(name) {
            Some(value) if !value.is_null() => Ok((value, at)),
            _ => Err(AgentSpecError::new(format!("`{at}` is missing"))),
        }
    }

    fn optional(&self, name: &str) -> Option<(&'a Value, String)> {
        self.members
            .get(name)
            .filter(|value| !value.is_null())
            .map(|value| (*value, self.path(name)))
    }

    fn get<T>(
        &self,
        name: &str,
        decode: impl Fn(&Value, &str) -> Result<T, AgentSpecError>,
    ) -> Result<T, AgentSpecError> {
        let (value, at) = self.required(name)?;
        decode(value, &at)
    }

    fn maybe<T>(
        &self,
        name: &str,
        decode: impl Fn(&Value, &str) -> Result<T, AgentSpecError>,
    ) -> Result<Option<T>, AgentSpecError> {
        self.optional(name)
            .map(|(value, at)| decode(value, &at))
            .transpose()
    }

    fn list<T>(
        &self,
        name: &str,
        decode: impl Fn(&Value, &str) -> Result<T, AgentSpecError>,
    ) -> Result<Vec<T>, AgentSpecError> {
        let Some((value, at)) = self.optional(name) else {
            return Ok(Vec::new());
        };
        let items = value
            .as_sequence()
            .ok_or_else(|| AgentSpecError::new(format!("`{at}` is not a list")))?;
        items
            .iter()
            .enumerate()
            .map(|(index, item)| decode(item, &format!("{at}[{index}]")))
            .collect()
    }
}

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}.{name}")
    }
}

fn text(value: &Value, at: &str) -> Result<String, AgentSpecError> {
    match value.as_str() {
        Some(text) if !text.trim().is_empty() => Ok(text.to_owned()),
        Some(_) => Err(AgentSpecError::new(format!("`{at}` is empty"))),
        None => Err(AgentSpecError::new(format!("`{at}` is not text"))),
    }
}

/// The name of an environment variable, never its value: `[A-Za-z_][A-Za-z0-9_]*`. A refusal
/// does not repeat the text, which may be a secret written in the wrong place.
fn variable_name(value: &Value, at: &str) -> Result<String, AgentSpecError> {
    let name = text(value, at)?;
    let mut bytes = name.bytes();
    let valid = bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(name)
    } else {
        Err(AgentSpecError::new(format!(
            "`{at}` is not an environment variable name"
        )))
    }
}

fn port(value: &Value, at: &str) -> Result<i64, AgentSpecError> {
    let port = integer(value, at)?;
    if (1..=65_535).contains(&port) {
        Ok(port)
    } else {
        Err(AgentSpecError::new(format!(
            "`{at}` is not a port from 1 to 65535"
        )))
    }
}

fn integer(value: &Value, at: &str) -> Result<i64, AgentSpecError> {
    value
        .as_i64()
        .ok_or_else(|| AgentSpecError::new(format!("`{at}` is not an integer")))
}

fn boolean(value: &Value, at: &str) -> Result<bool, AgentSpecError> {
    value
        .as_bool()
        .ok_or_else(|| AgentSpecError::new(format!("`{at}` is not a boolean")))
}

fn decimal(value: &Value, at: &str) -> Result<Decimal, AgentSpecError> {
    let refused = || AgentSpecError::new(format!("`{at}` is not a decimal"));
    let spelled = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => return Err(refused()),
    };
    let digits = spelled.strip_prefix('-').unwrap_or(&spelled);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() || !all_digits(whole) || !all_digits(fraction) {
        return Err(refused());
    }
    if digits.contains('.') && fraction.is_empty() {
        return Err(refused());
    }
    Ok(Decimal(spelled))
}

fn timestamp(value: &Value, at: &str) -> Result<Timestamp, AgentSpecError> {
    text(value, at).map(Timestamp)
}

fn variant<T: Copy>(value: &Value, at: &str, declared: &[(&str, T)]) -> Result<T, AgentSpecError> {
    let spelled = text(value, at)?;
    declared
        .iter()
        .find(|(name, _)| *name == spelled)
        .map(|(_, variant)| *variant)
        .ok_or_else(|| {
            let names: Vec<_> = declared.iter().map(|(name, _)| *name).collect();
            AgentSpecError::new(format!(
                "`{at}` is `{spelled}`, not one of {}",
                names.join(", ")
            ))
        })
}

fn source(value: &Value, at: &str) -> Result<SourceName, AgentSpecError> {
    text(value, at).map(SourceName)
}

fn operation(value: &Value, at: &str) -> Result<OperationId, AgentSpecError> {
    text(value, at).map(OperationId)
}

fn spec(value: &Value) -> Result<AgentSpec, AgentSpecError> {
    let members = Members::of(
        value,
        "",
        &[
            "format", "agent", "model", "session", "loops", "states", "bindings", "channel",
            "writes", "policy", "expose", "runtime", "deploy",
        ],
    )?;
    let format = members.get("format", text)?;
    if format != FORMAT {
        return Err(AgentSpecError::new(format!(
            "`format` is `{format}`, not `{FORMAT}`"
        )));
    }
    Ok(AgentSpec {
        format,
        agent: members.get("agent", identity)?,
        model: members.get("model", |value, at| {
            crate::models::decode_config(value)
                .map_err(|error| AgentSpecError::new(format!("`{at}`: {error}")))
        })?,
        session: members.maybe("session", session)?,
        loops: members.maybe("loops", loops)?,
        states: members.list("states", state)?,
        bindings: members.list("bindings", binding)?,
        channel: members.maybe("channel", channel)?,
        writes: members.maybe("writes", writes)?,
        policy: members.maybe("policy", |value, at| {
            let members = Members::of(value, at, &["capabilities"])?;
            Ok(PolicySpec {
                capabilities: members.list("capabilities", text)?,
            })
        })?,
        expose: members.maybe("expose", expose)?,
        runtime: members.maybe("runtime", runtime)?,
        deploy: members.maybe("deploy", deploy)?,
    })
}

fn identity(value: &Value, at: &str) -> Result<AgentIdentity, AgentSpecError> {
    let members = Members::of(value, at, &["id", "name", "brief"])?;
    Ok(AgentIdentity {
        id: AgentId(members.get("id", text)?),
        name: members.get("name", text)?,
        brief: members.get("brief", text)?,
    })
}

fn session(value: &Value, at: &str) -> Result<SessionSpec, AgentSpecError> {
    let members = Members::of(
        value,
        at,
        &["kind", "budget", "stop", "confinement", "tools"],
    )?;
    Ok(SessionSpec {
        kind: members.get("kind", |value, at| {
            variant(
                value,
                at,
                &[
                    ("ClaudeCode", SessionKind::ClaudeCode),
                    ("Codex", SessionKind::Codex),
                    ("Harness", SessionKind::Harness),
                ],
            )
        })?,
        budget: members.get("budget", run_budget)?,
        stop: members.get("stop", stop_policy)?,
        confinement: members.get("confinement", confinement)?,
        tools: members.get("tools", tool_policy)?,
    })
}

fn confinement(value: &Value, at: &str) -> Result<ConfinementMode, AgentSpecError> {
    variant(value, at, &[("none", ConfinementMode::None)])
}

fn tool_policy(value: &Value, at: &str) -> Result<ToolPolicy, AgentSpecError> {
    let members = Members::of(value, at, &["allow", "deny", "egress", "decisions"])?;
    Ok(ToolPolicy {
        allow: members.list("allow", text)?,
        deny: members.list("deny", text)?,
        egress: members.list("egress", |value, at| {
            let members = Members::of(value, at, &["name", "host", "port", "max_bytes"])?;
            Ok(EgressAperture {
                name: members.get("name", text)?,
                host: members.get("host", text)?,
                port: members.get("port", port)?,
                max_bytes: members.maybe("max_bytes", integer)?,
            })
        })?,
        decisions: members.get("decisions", |value, at| {
            variant(
                value,
                at,
                &[
                    ("frame", DecisionMode::Frame),
                    ("ask", DecisionMode::Ask),
                    ("observe", DecisionMode::Observe),
                ],
            )
        })?,
    })
}

const BUDGET_LIMITS: [(&str, BudgetLimit); 8] = [
    ("MaxSteps", BudgetLimit::MaxSteps),
    ("MaxTrials", BudgetLimit::MaxTrials),
    ("MaxTokensIn", BudgetLimit::MaxTokensIn),
    ("MaxTokensOut", BudgetLimit::MaxTokensOut),
    ("MaxDurationMs", BudgetLimit::MaxDurationMs),
    ("MaxCostMicroUsd", BudgetLimit::MaxCostMicroUsd),
    ("MaxTokens", BudgetLimit::MaxTokens),
    ("Deadline", BudgetLimit::Deadline),
];

const BREACH_STEPS: [(&str, BreachStep); 2] = [
    ("DiscardStep", BreachStep::DiscardStep),
    ("KeepStep", BreachStep::KeepStep),
];

fn run_budget(value: &Value, at: &str) -> Result<RunBudget, AgentSpecError> {
    let members = Members::of(
        value,
        at,
        &[
            "max_steps",
            "max_tokens_in",
            "max_tokens_out",
            "max_duration_ms",
            "max_cost_micro_usd",
            "max_tokens",
            "deadline",
            "per_action",
        ],
    )?;
    Ok(RunBudget {
        max_steps: members.maybe("max_steps", integer)?,
        max_tokens_in: members.maybe("max_tokens_in", integer)?,
        max_tokens_out: members.maybe("max_tokens_out", integer)?,
        max_duration_ms: members.maybe("max_duration_ms", integer)?,
        max_cost_micro_usd: members.maybe("max_cost_micro_usd", integer)?,
        max_tokens: members.maybe("max_tokens", integer)?,
        deadline: members.maybe("deadline", timestamp)?,
        per_action: members.list("per_action", |value, at| {
            let members = Members::of(
                value,
                at,
                &[
                    "action",
                    "max_trials",
                    "max_tokens_in",
                    "max_tokens_out",
                    "max_duration_ms",
                    "max_cost_micro_usd",
                ],
            )?;
            Ok(ActionBudget {
                action: members.get("action", text)?,
                max_trials: members.maybe("max_trials", integer)?,
                max_tokens_in: members.maybe("max_tokens_in", integer)?,
                max_tokens_out: members.maybe("max_tokens_out", integer)?,
                max_duration_ms: members.maybe("max_duration_ms", integer)?,
                max_cost_micro_usd: members.maybe("max_cost_micro_usd", integer)?,
            })
        })?,
    })
}

fn stop_policy(value: &Value, at: &str) -> Result<StopPolicy, AgentSpecError> {
    let members = Members::of(
        value,
        at,
        &[
            "approval_gate",
            "idle_bound",
            "on_breach",
            "breach_by_limit",
        ],
    )?;
    Ok(StopPolicy {
        approval_gate: members.get("approval_gate", boolean)?,
        idle_bound: members.maybe("idle_bound", integer)?,
        on_breach: members.get("on_breach", |value, at| variant(value, at, &BREACH_STEPS))?,
        breach_by_limit: members.list("breach_by_limit", |value, at| {
            let members = Members::of(value, at, &["limit", "step"])?;
            Ok(LimitBreachStep {
                limit: members.get("limit", |value, at| variant(value, at, &BUDGET_LIMITS))?,
                step: members.get("step", |value, at| variant(value, at, &BREACH_STEPS))?,
            })
        })?,
    })
}

fn loops(value: &Value, at: &str) -> Result<Loops, AgentSpecError> {
    let members = Members::of(value, at, &["outer", "inner"])?;
    Ok(Loops {
        outer: members.get("outer", |value, at| {
            let members = Members::of(
                value,
                at,
                &[
                    "poll_interval_seconds",
                    "sources",
                    "min_age_minutes",
                    "lookback_minutes",
                ],
            )?;
            Ok(OuterLoop {
                poll_interval_seconds: members.get("poll_interval_seconds", integer)?,
                sources: members.list("sources", source)?,
                min_age_minutes: members.get("min_age_minutes", integer)?,
                lookback_minutes: members.maybe("lookback_minutes", integer)?,
            })
        })?,
        inner: members.get("inner", |value, at| {
            let members = Members::of(
                value,
                at,
                &[
                    "protocol",
                    "step_budget",
                    "model_budget",
                    "classify_threshold",
                ],
            )?;
            Ok(InnerLoop {
                protocol: members.get("protocol", text)?,
                step_budget: members.get("step_budget", integer)?,
                model_budget: members.get("model_budget", integer)?,
                classify_threshold: members.maybe("classify_threshold", decimal)?,
            })
        })?,
    })
}

fn state(value: &Value, at: &str) -> Result<AgentState, AgentSpecError> {
    let members = Members::of(value, at, &["name", "when", "actions", "sources"])?;
    Ok(AgentState {
        name: members.get("name", text)?,
        when: members.get("when", |value, at| {
            let members = Members::of(value, at, &["intent"])?;
            Ok(StateWhen {
                intent: members.get("intent", |value, at| {
                    variant(
                        value,
                        at,
                        &[
                            ("Ask", Intent::Ask),
                            ("Request", Intent::Request),
                            ("Task", Intent::Task),
                            ("Find", Intent::Find),
                        ],
                    )
                })?,
            })
        })?,
        actions: members.list("actions", text)?,
        sources: members.list("sources", source)?,
    })
}

fn binding(value: &Value, at: &str) -> Result<Binding, AgentSpecError> {
    let kind = value
        .as_mapping()
        .and_then(|mapping| mapping.get("kind"))
        .ok_or_else(|| AgentSpecError::new(format!("`{}` is missing", join(at, "kind"))))?;
    match text(kind, &join(at, "kind"))?.as_str() {
        "connectors" => {
            let members = Members::of(
                value,
                at,
                &[
                    "kind",
                    "name",
                    "adapter",
                    "connection",
                    "list",
                    "search",
                    "get",
                    "write",
                ],
            )?;
            Ok(Binding::Connectors(ConnectorsBinding {
                name: members.get("name", source)?,
                adapter: members.get("adapter", |value, at| text(value, at).map(AdapterAlias))?,
                connection: members
                    .get("connection", |value, at| text(value, at).map(ConnectionId))?,
                list: members.maybe("list", operation)?,
                search: members.maybe("search", operation)?,
                get: members.maybe("get", operation)?,
                write: members.maybe("write", operation)?,
            }))
        }
        "http" => {
            let members = Members::of(
                value,
                at,
                &[
                    "kind",
                    "name",
                    "base_url",
                    "allow_paths",
                    "method",
                    "bearer_env",
                    "max_bytes",
                ],
            )?;
            Ok(Binding::Http(HttpBinding {
                name: members.get("name", source)?,
                base_url: members.get("base_url", text)?,
                allow_paths: members.list("allow_paths", text)?,
                method: members.get("method", text)?,
                bearer_env: members.maybe("bearer_env", variable_name)?,
                max_bytes: members.get("max_bytes", integer)?,
            }))
        }
        other => Err(AgentSpecError::new(format!(
            "`{}` is `{other}`, not one of connectors, http",
            join(at, "kind")
        ))),
    }
}

fn channel(value: &Value, at: &str) -> Result<ChannelSpec, AgentSpecError> {
    let members = Members::of(value, at, &["binding", "allow", "bot_user_id"])?;
    Ok(ChannelSpec {
        binding: members.get("binding", source)?,
        allow: members.list("allow", text)?,
        bot_user_id: members.get("bot_user_id", text)?,
    })
}

fn writes(value: &Value, at: &str) -> Result<WritesSpec, AgentSpecError> {
    let members = Members::of(value, at, &["mode", "max_per_hour"])?;
    Ok(WritesSpec {
        mode: members.get("mode", |value, at| {
            variant(
                value,
                at,
                &[
                    ("auto", WriteMode::Auto),
                    ("approval", WriteMode::Approval),
                    ("none", WriteMode::None),
                ],
            )
        })?,
        max_per_hour: members.get("max_per_hour", integer)?,
    })
}

fn expose(value: &Value, at: &str) -> Result<ExposeSpec, AgentSpecError> {
    let members = Members::of(value, at, &["a2a"])?;
    Ok(ExposeSpec {
        a2a: members.maybe("a2a", |value, at| {
            let members = Members::of(value, at, &["port", "path", "card"])?;
            Ok(A2aExposure {
                port: members.get("port", port)?,
                path: members.get("path", text)?,
                card: members.get("card", |value, at| {
                    let members = Members::of(value, at, &["name", "description", "skills"])?;
                    Ok(A2aCard {
                        name: members.get("name", text)?,
                        description: members.get("description", text)?,
                        skills: members.list("skills", text)?,
                    })
                })?,
            })
        })?,
    })
}

fn runtime(value: &Value, at: &str) -> Result<RuntimeSpec, AgentSpecError> {
    let members = Members::of(value, at, &["state_dir", "connectors", "confinement"])?;
    Ok(RuntimeSpec {
        state_dir: members.get("state_dir", text)?,
        connectors: members.get("connectors", |value, at| {
            let members = Members::of(
                value,
                at,
                &["program", "config", "state_dir", "timeout_seconds"],
            )?;
            Ok(ConnectorsCliConfig {
                program: members.get("program", text)?,
                config: members.maybe("config", text)?,
                state_dir: members.maybe("state_dir", text)?,
                timeout_seconds: members.maybe("timeout_seconds", integer)?,
            })
        })?,
        confinement: members.get("confinement", confinement)?,
    })
}

fn deploy(value: &Value, at: &str) -> Result<DeploySpec, AgentSpecError> {
    let members = Members::of(value, at, &["kubernetes"])?;
    Ok(DeploySpec {
        kubernetes: members.maybe("kubernetes", kubernetes)?,
    })
}

fn kubernetes(value: &Value, at: &str) -> Result<KubernetesDeploy, AgentSpecError> {
    let members = Members::of(
        value,
        at,
        &[
            "namespace",
            "service_account",
            "image_repository",
            "resources",
            "egress",
            "ingress",
        ],
    )?;
    Ok(KubernetesDeploy {
        namespace: members.get("namespace", text)?,
        service_account: members.get("service_account", text)?,
        image_repository: members.get("image_repository", text)?,
        resources: members.get("resources", |value, at| {
            let members = Members::of(value, at, &["cpu", "memory"])?;
            Ok(KubernetesResources {
                cpu: members.get("cpu", text)?,
                memory: members.get("memory", text)?,
            })
        })?,
        egress: members.list("egress", |value, at| {
            let members = Members::of(
                value,
                at,
                &["name", "host", "namespace", "selector", "port"],
            )?;
            Ok(KubernetesEgress {
                name: members.get("name", text)?,
                host: members.maybe("host", text)?,
                namespace: members.maybe("namespace", text)?,
                selector: members.list("selector", |value, at| {
                    let members = Members::of(value, at, &["name", "value"])?;
                    Ok(Label {
                        name: members.get("name", text)?,
                        value: members.get("value", text)?,
                    })
                })?,
                port: members.get("port", port)?,
            })
        })?,
        ingress: members.list("ingress", |value, at| {
            let members = Members::of(value, at, &["name", "from_namespace", "port"])?;
            Ok(KubernetesIngress {
                name: members.get("name", text)?,
                from_namespace: members.get("from_namespace", text)?,
                port: members.get("port", port)?,
            })
        })?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, roles: []}\n";

    #[test]
    fn a_spec_with_an_unknown_member_is_refused_in_yaml_and_json() {
        assert!(parse(MINIMAL).is_ok());
        let yaml = format!(
            "{MINIMAL}loops:\n  outer: {{poll_interval_seconds: 60, min_age_minutes: 0, cadence: 5}}\n  inner: {{protocol: inbound-answer@1, step_budget: 8, model_budget: 16}}\n"
        );
        let refused = parse(&yaml).expect_err("an unknown member decodes");
        assert_eq!(refused.to_string(), "unknown member `loops.outer.cadence`");
        let json = r#"{"format": "loom.agent/1", "agent": {"id": "a", "name": "A", "brief": "b", "owner": "x"}, "model": {"routes": "routes.yaml", "roles": []}}"#;
        let refused = parse(json).expect_err("an unknown member decodes");
        assert_eq!(refused.to_string(), "unknown member `agent.owner`");
    }
}
