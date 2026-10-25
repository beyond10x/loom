//! The checks a `LoomAgent` passes before any child is rendered, in this order; the first refusal
//! is the answer:
//!
//! 1. `spec.agent`, as YAML, through `b10x_loom_agent::check_text` against the plugin catalog:
//!    the check `b10x-loom agent check` runs, in-process (`Malformed`, `SecretLiteral`,
//!    `UnknownProtocol`, `UnknownAction`, `UnboundSource`, `HttpMethod`, `SendWithoutWrites`,
//!    `EmptyChannelAllow`).
//! 2. `spec.routes` walked for a credential by the same library (`SecretLiteral`); a routes
//!    document that is not a mapping is `Malformed`.
//! 3. `spec.connectorsConfig` walked for a credential, each word of the TOML as one value
//!    (`SecretLiteral`).
//! 4. An empty `spec.routes` while the controller configures no model route (`NoModelRoute`).
//! 5. A `spec.secretRef` outside the configuration's `allowedSecretNames` (`SecretNotAllowed`).
//! 6. A `deploy.kubernetes.namespace` other than the resource's (`NamespaceMismatch`).
//! 7. A `deploy.kubernetes.egress` host outside `allowedEgressHosts` (`EgressHostNotAllowed`).
//!
//! A refusal reads `refused: <reason> at <member>: <detail>`, as `b10x-loom agent check` prints
//! it, the member spelt from the resource (`spec.agent.bindings[1].base_url`); its detail never
//! repeats a value of the resource. An admitted spec reads as the check's line, `checked <agent>:
//! <n> states, <m> bindings, protocol <protocol>`.

use b10x_loom_agent::{Checked, RefusalReason, check_text, spelling};
use loom_sdk::agent::{AgentSpec, parse};
use serde_yaml_ng::{Mapping, Value};

use crate::config::{Config, ModelRoute};
use crate::resource::LoomAgentSpec;

/// The exit code a refusal is recorded with, as `b10x-loom agent check` exits on one.
pub const REFUSED: i64 = 3;

/// The name of the endpoint the default routes document declares.
pub const MODEL_ROUTE_ENDPOINT: &str = "model-route";

/// Why a resource is refused: the reason, the member of the resource and what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The reason, in CamelCase: a `loom.agent.RefusalReason` or one of the controller's.
    pub reason: String,
    /// The member, spelt from the resource: `spec.agent.deploy.kubernetes.namespace`.
    pub member: String,
    /// What is wrong, never repeating a value.
    pub detail: String,
}

impl Refusal {
    fn new(reason: &str, member: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            reason: reason.to_owned(),
            member: member.into(),
            detail: detail.into(),
        }
    }

    /// The line the refusal is recorded as in `status.check.message`.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "refused: {} at {}: {}",
            self.reason, self.member, self.detail
        )
    }
}

/// An admitted resource: its decoded spec, the three files of its ConfigMap and the check's
/// answer.
#[derive(Debug, Clone)]
pub struct Admitted {
    /// The decoded `loom.agent/1` document.
    pub spec: AgentSpec,
    /// `agent.yaml`: `spec.agent` as YAML.
    pub agent: String,
    /// `routes.yaml`: `spec.routes` as YAML, or the default routes document.
    pub routes: String,
    /// `connectors.toml`: `spec.connectorsConfig`, empty when absent.
    pub connectors: String,
    /// Whether `routes.yaml` is the default routes document, which opens the model route.
    pub default_routes: bool,
    /// What the check found.
    pub checked: Checked,
}

impl Admitted {
    /// The line the check is recorded as in `status.check.message`.
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "checked {}: {} states, {} bindings, protocol {}",
            self.checked.agent,
            self.checked.states,
            self.checked.bindings,
            self.checked.protocol.as_deref().unwrap_or("none")
        )
    }
}

/// `member` of a document at `at` in the resource.
fn within(at: &str, member: &str) -> String {
    if member == "document" {
        at.to_owned()
    } else if member.starts_with('[') {
        format!("{at}{member}")
    } else {
        format!("{at}.{member}")
    }
}

/// The first credential `text`, a YAML document, holds, as a refusal at `at`.
fn credential(text: &str, at: &str) -> Option<Refusal> {
    match check_text(text, &|_| None) {
        Err(refused) if refused.reason == RefusalReason::SecretLiteral => Some(Refusal::new(
            spelling(refused.reason),
            within(at, &refused.member),
            refused.detail,
        )),
        _ => None,
    }
}

/// `value`, a member of the resource at `at`, as YAML.
///
/// The value goes through its JSON text: a `serde_json::Value` number does not serialize as a
/// number to another format when `serde_json`'s `arbitrary_precision` is on, as it is in this
/// workspace, and JSON text reads as YAML.
pub fn yaml(value: &impl serde::Serialize, at: &str) -> Result<String, Refusal> {
    let refused = || {
        Refusal::new(
            spelling(RefusalReason::Malformed),
            at,
            "cannot be written as YAML",
        )
    };
    let json = serde_json::to_string(value).map_err(|_| refused())?;
    let value: Value = serde_yaml_ng::from_str(&json).map_err(|_| refused())?;
    serde_yaml_ng::to_string(&value).map_err(|_| refused())
}

/// Whether `routes` asks for the controller's model route: an empty object, or nothing.
fn empty(routes: &serde_json::Value) -> bool {
    match routes {
        serde_json::Value::Null => true,
        serde_json::Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

/// The routes document a resource with an empty `spec.routes` is given: `llm.routes/1`, one
/// anonymous endpoint at the model route, and one route per model a role names, to the model
/// route's model.
#[must_use]
pub fn default_routes(route: &ModelRoute, spec: &AgentSpec) -> String {
    let mut auth = Mapping::new();
    auth.insert("kind".into(), "anonymous".into());
    let mut endpoint = Mapping::new();
    endpoint.insert("base_url".into(), route.url.clone().into());
    endpoint.insert("protocol".into(), route.protocol.clone().into());
    endpoint.insert("auth".into(), Value::Mapping(auth));
    let mut endpoints = Mapping::new();
    endpoints.insert(MODEL_ROUTE_ENDPOINT.into(), Value::Mapping(endpoint));
    let mut routes = Mapping::new();
    for role in &spec.model.roles {
        routes.insert(
            role.model.clone().into(),
            format!("{MODEL_ROUTE_ENDPOINT}/{}", route.model).into(),
        );
    }
    let mut document = Mapping::new();
    document.insert("format".into(), "llm.routes/1".into());
    document.insert("endpoints".into(), Value::Mapping(endpoints));
    document.insert("routes".into(), Value::Mapping(routes));
    serde_yaml_ng::to_string(&Value::Mapping(document)).unwrap_or_default()
}

/// Admits `spec`, a `LoomAgent` in `namespace`, under `config`, checking `spec.agent` against
/// `actions` (the action ids of a catalog protocol by its `name@major`).
///
/// # Errors
/// The first refusal, in the order the module documentation gives.
pub fn admit(
    spec: &LoomAgentSpec,
    namespace: &str,
    config: &Config,
    actions: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Result<Admitted, Refusal> {
    let agent = yaml(&spec.agent, "spec.agent")?;
    let checked = check_text(&agent, actions).map_err(|refused| {
        Refusal::new(
            spelling(refused.reason),
            within("spec.agent", &refused.member),
            refused.detail,
        )
    })?;
    let decoded = parse(&agent).map_err(|error| {
        Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.agent",
            error.to_string(),
        )
    })?;

    let given = (!empty(&spec.routes))
        .then(|| yaml(&spec.routes, "spec.routes"))
        .transpose()?;
    if let Some(text) = &given {
        if let Some(refusal) = credential(text, "spec.routes") {
            return Err(refusal);
        }
        if !spec.routes.is_object() {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.routes",
                "an llm.routes/1 document is a mapping",
            ));
        }
    }

    let connectors = spec.connectors_config.clone().unwrap_or_default();
    let words: Vec<&str> = connectors
        .split(|c: char| c.is_whitespace() || "=\"',[]{}".contains(c))
        .filter(|word| !word.is_empty())
        .collect();
    if let Some(refusal) = credential(&yaml(&words, "spec.connectorsConfig")?, "document") {
        return Err(Refusal::new(
            &refusal.reason,
            "spec.connectorsConfig",
            refusal.detail,
        ));
    }

    let (routes, default_routes) = match (given, &config.model_route) {
        (Some(text), _) => (text, false),
        (None, Some(route)) => (default_routes(route, &decoded), true),
        (None, None) => {
            return Err(Refusal::new(
                "NoModelRoute",
                "spec.routes",
                "it is empty and the controller configures no model route",
            ));
        }
    };

    if let Some(secret) = &spec.secret_ref
        && !config.allowed_secret_names.contains(&secret.name)
    {
        return Err(Refusal::new(
            "SecretNotAllowed",
            "spec.secretRef.name",
            "the controller's allowedSecretNames does not hold it",
        ));
    }

    if let Some(kubernetes) = decoded
        .deploy
        .as_ref()
        .and_then(|deploy| deploy.kubernetes.as_ref())
    {
        if kubernetes.namespace != namespace {
            return Err(Refusal::new(
                "NamespaceMismatch",
                "spec.agent.deploy.kubernetes.namespace",
                "it names another namespace than the resource's",
            ));
        }
        for (index, egress) in kubernetes.egress.iter().enumerate() {
            if let Some(host) = &egress.host
                && !config.allowed_egress_hosts.contains(host)
            {
                return Err(Refusal::new(
                    "EgressHostNotAllowed",
                    format!("spec.agent.deploy.kubernetes.egress[{index}].host"),
                    "the controller's allowedEgressHosts does not hold it",
                ));
            }
        }
    }

    Ok(Admitted {
        spec: decoded,
        agent,
        routes,
        connectors,
        default_routes,
        checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        let mut config = Config::builtin();
        config.namespace = "agents".to_owned();
        config
    }

    fn spec(connectors: Option<&str>) -> LoomAgentSpec {
        LoomAgentSpec {
            agent: serde_json::json!({
                "format": "loom.agent/1",
                "agent": {"id": "a", "name": "A", "brief": "answers"},
                "model": {"routes": "routes.yaml", "roles": [{"role": "agent", "model": "small"}]},
            }),
            routes: serde_json::json!({
                "format": "llm.routes/1",
                "endpoints": {"lab": {"base_url": "http://lab.example/v1", "protocol": "messages",
                                      "auth": {"kind": "anonymous"}}},
                "routes": {"small": "lab/small"},
            }),
            connectors_config: connectors.map(str::to_owned),
            secret_ref: None,
            image: None,
        }
    }

    fn admitted(spec: &LoomAgentSpec) -> Result<Admitted, Refusal> {
        admit(spec, "agents", &config(), &|_| None)
    }

    #[test]
    fn a_spec_without_loops_is_admitted_with_the_checks_line() {
        let admitted = admitted(&spec(None)).expect("admitted");
        assert_eq!(
            admitted.message(),
            "checked a: 0 states, 0 bindings, protocol none"
        );
        assert!(!admitted.default_routes);
        assert_eq!(admitted.connectors, "");
    }

    #[test]
    fn a_token_in_the_connectors_toml_is_refused_without_repeating_it() {
        let refusal = admitted(&spec(Some(
            "[connections.ops]\nadapter = \"chat\"\ntoken = \"xoxb-000-fake\"\n",
        )))
        .expect_err("refused");
        assert_eq!(refusal.reason, "SecretLiteral");
        assert_eq!(refusal.member, "spec.connectorsConfig");
        assert!(!refusal.message().contains("xoxb-000-fake"));
        admitted(&spec(Some(
            "[connections.ops]\nadapter = \"chat\"\ntoken = \"env:CHAT_TOKEN\"\n",
        )))
        .expect("a reference is admitted");
    }

    #[test]
    fn routes_that_are_not_a_mapping_are_malformed() {
        let mut list = spec(None);
        list.routes = serde_json::json!(["lab/small"]);
        let refusal = admitted(&list).expect_err("refused");
        assert_eq!(
            refusal.message(),
            "refused: Malformed at spec.routes: an llm.routes/1 document is a mapping"
        );
    }

    #[test]
    fn the_default_routes_name_each_roles_model_once() {
        let route = ModelRoute {
            url: "http://model-route.agents.svc:8080/v1".to_owned(),
            protocol: "messages".to_owned(),
            model: "small-model".to_owned(),
        };
        let decoded = parse(
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, \
             roles: [{role: agent, model: small}, {role: classifier, model: small}]}\n",
        )
        .expect("a spec");
        assert_eq!(
            default_routes(&route, &decoded),
            "format: llm.routes/1\nendpoints:\n  model-route:\n    base_url: \
             http://model-route.agents.svc:8080/v1\n    protocol: messages\n    auth:\n      \
             kind: anonymous\nroutes:\n  small: model-route/small-model\n"
        );
    }
}
