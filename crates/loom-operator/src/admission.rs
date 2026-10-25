//! The checks a `LoomAgent` passes before any child is rendered, in this order; the first refusal
//! is the answer:
//!
//! 1. `spec.agent`, as YAML, through `b10x_loom_agent::check_text` against the plugin catalog:
//!    the check `b10x-loom agent check` runs, in-process (`Malformed`, `SecretLiteral`,
//!    `UnknownProtocol`, `UnknownAction`, `UnboundSource`, `HttpMethod`, `SendWithoutWrites`,
//!    `EmptyChannelAllow`).
//! 2. `spec.routes` walked for a credential by the same library (`SecretLiteral`); a routes
//!    document that is not a mapping is `Malformed`, and so is one llm's strict `llm.routes/1`
//!    parser (`llm_models::routes::Routes::parse`, the reader of the pod's `routes.yaml`)
//!    refuses: an unknown field, an `auth.secret` that is a value and not an `env:`, `file:` or
//!    `keychain:` reference, a route to an undeclared endpoint and the like.
//! 3. `spec.connectorsConfig` decoded as TOML (`Malformed` when it is not TOML) and every key and
//!    string of the decoded table walked for a credential (`SecretLiteral`), so an escape such as
//!    `x` hides nothing.
//! 4. An empty `spec.routes` while the controller configures no model route (`NoModelRoute`).
//! 5. A `spec.secretRef` outside the configuration's `allowedSecretNames` (`SecretNotAllowed`).
//! 6. A `deploy.kubernetes.namespace` other than the resource's (`NamespaceMismatch`).
//! 7. A `deploy.kubernetes.egress` host outside `allowedEgressHosts` (`EgressHostNotAllowed`).
//!
//! The walk of steps 2 and 3 reads a text that is not a `loom.agent/1` document only within the
//! agent decoder's bounds, [`MAX_WALKED_OPENERS`] `[` or `{` bytes and [`MAX_WALKED_BYTES`]; a
//! text beyond either is refused `TooLarge` rather than admitted unread.
//!
//! A refusal reads `refused: <reason> at <member>: <detail>`, as `b10x-loom agent check` prints
//! it, the member spelt from the resource (`spec.agent.bindings[1].base_url`); its detail never
//! repeats a value of the resource. An admitted spec reads as the check's line, `checked <agent>:
//! <n> states, <m> bindings, protocol <protocol>`.

use b10x_loom_agent::{Checked, RefusalReason, check_text, spelling};
use llm_models::routes::Routes;
use loom_sdk::agent::{AgentSpec, parse};
use serde_yaml_ng::{Mapping, Value};

use crate::config::{Config, ModelRoute};
use crate::resource::LoomAgentSpec;

/// The exit code a refusal is recorded with, as `b10x-loom agent check` exits on one.
pub const REFUSED: i64 = 3;

/// The most `[` and `{` bytes a walked routes or connectors text may hold: the agent decoder's
/// bound, beyond which `check_text` does not walk a text that is not a `loom.agent/1` document.
pub const MAX_WALKED_OPENERS: usize = loom_sdk::agent::MAX_FLOW_OPENERS;

/// The longest walked routes or connectors text, in bytes: the agent decoder's byte bound, beyond
/// which `check_text` does not walk a text that is not a `loom.agent/1` document.
pub const MAX_WALKED_BYTES: usize = 2 * 1024 * 1024;

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

/// The first credential `text`, a YAML document standing at `at` in the resource, holds, as a
/// refusal; `TooLarge` when the text is beyond what the walk reads.
fn credential(text: &str, at: &str) -> Result<(), Refusal> {
    let openers = text
        .bytes()
        .filter(|byte| matches!(byte, b'[' | b'{'))
        .count();
    if openers > MAX_WALKED_OPENERS {
        return Err(Refusal::new(
            "TooLarge",
            at,
            format!(
                "it holds more than {MAX_WALKED_OPENERS} `[` or `{{` bytes, more than the \
                 credential walk reads"
            ),
        ));
    }
    if text.len() > MAX_WALKED_BYTES {
        return Err(Refusal::new(
            "TooLarge",
            at,
            format!(
                "it is longer than {MAX_WALKED_BYTES} bytes as YAML, more than the credential \
                 walk reads"
            ),
        ));
    }
    match check_text(text, &|_| None) {
        Err(refused) if refused.reason == RefusalReason::SecretLiteral => Err(Refusal::new(
            spelling(refused.reason),
            within(at, &refused.member),
            refused.detail,
        )),
        _ => Ok(()),
    }
}

/// `text`, the `spec.connectorsConfig` TOML, decoded and written as YAML for the walk.
fn connectors_yaml(text: &str) -> Result<String, Refusal> {
    let table: toml::Table = toml::from_str(text).map_err(|error| {
        let detail = error.span().map_or_else(
            || "it is not a TOML document".to_owned(),
            |span| {
                let line = text
                    .as_bytes()
                    .iter()
                    .take(span.start)
                    .filter(|byte| **byte == b'\n')
                    .count()
                    + 1;
                format!("it is not a TOML document (line {line})")
            },
        );
        Refusal::new(
            spelling(RefusalReason::Malformed),
            "spec.connectorsConfig",
            detail,
        )
    })?;
    yaml(&table, "spec.connectorsConfig")
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
        credential(text, "spec.routes")?;
        if !spec.routes.is_object() {
            return Err(Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.routes",
                "an llm.routes/1 document is a mapping",
            ));
        }
        Routes::parse(text).map_err(|error| {
            Refusal::new(
                spelling(RefusalReason::Malformed),
                "spec.routes",
                error.message,
            )
        })?;
    }

    let connectors = spec.connectors_config.clone().unwrap_or_default();
    credential(&connectors_yaml(&connectors)?, "spec.connectorsConfig")?;

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
        assert_eq!(
            refusal.member,
            "spec.connectorsConfig.connections.ops.token"
        );
        assert!(!refusal.message().contains("xoxb-000-fake"));
        admitted(&spec(Some(
            "[connections.ops]\nadapter = \"chat\"\ntoken = \"env:CHAT_TOKEN\"\n",
        )))
        .expect("a reference is admitted");
    }

    #[test]
    fn a_token_spelt_with_toml_escapes_or_as_a_key_is_refused_as_decoded() {
        for (toml, member) in [
            (
                "[connections.ops]\ntoken = \"\\u0078oxb-000-fake\"\n",
                "spec.connectorsConfig.connections.ops.token",
            ),
            (
                "[connections.ops]\ntoken = \"\"\"\n\\U00000078oxb-000-fake\"\"\"\n",
                "spec.connectorsConfig.connections.ops.token",
            ),
            (
                "[connections]\n\"\\u0078oxb-000-fake\" = 1\n",
                "spec.connectorsConfig.connections",
            ),
        ] {
            let refusal = admitted(&spec(Some(toml))).expect_err(toml);
            assert_eq!(refusal.reason, "SecretLiteral", "{toml}");
            assert_eq!(refusal.member, member, "{toml}");
            assert!(!refusal.message().contains("oxb-000-fake"), "{toml}");
        }
    }

    #[test]
    fn a_connectors_config_that_is_not_toml_is_malformed_without_repeating_it() {
        let refusal = admitted(&spec(Some("[connections.ops]\ntoken = xoxb-000-fake\n")))
            .expect_err("refused");
        assert_eq!(
            refusal.message(),
            "refused: Malformed at spec.connectorsConfig: it is not a TOML document (line 2)"
        );
    }

    #[test]
    fn a_routes_secret_that_is_a_value_is_refused_by_llms_parser_without_repeating_it() {
        let mut literal = spec(None);
        literal.routes["endpoints"]["lab"]["auth"] =
            serde_json::json!({"kind": "bearer", "secret": "not-a-reference-000-fake"});
        let refusal = admitted(&literal).expect_err("refused");
        assert_eq!(refusal.reason, "Malformed");
        assert_eq!(refusal.member, "spec.routes");
        assert!(
            refusal.detail.contains("must be a reference"),
            "{refusal:?}"
        );
        assert!(!refusal.message().contains("not-a-reference-000-fake"));

        let mut reference = spec(None);
        reference.routes["endpoints"]["lab"]["auth"] =
            serde_json::json!({"kind": "bearer", "secret": "env:LAB_KEY"});
        admitted(&reference).expect("a reference is admitted");

        let mut unknown = spec(None);
        unknown.routes["note"] = serde_json::json!("anything");
        assert_eq!(admitted(&unknown).expect_err("refused").reason, "Malformed");
    }

    /// The walk's bound, held at its edge: a token beside exactly [`MAX_WALKED_OPENERS`] braces
    /// is still found, and one more brace refuses the text unwalked.
    #[test]
    fn a_routes_text_beyond_the_walks_bound_is_too_large_and_one_within_it_is_walked() {
        let at = |braces: usize| {
            let mut routes = spec(None);
            routes.routes["note"] = serde_json::json!("{".repeat(braces));
            routes.routes["endpoints"]["lab"]["auth"] =
                serde_json::json!({"kind": "bearer", "secret": "xoxb-000-fake"});
            admitted(&routes).expect_err("refused")
        };
        let within = at(MAX_WALKED_OPENERS);
        assert_eq!(within.reason, "SecretLiteral");
        assert_eq!(within.member, "spec.routes.endpoints.lab.auth.secret");
        let beyond = at(MAX_WALKED_OPENERS + 1);
        assert_eq!(
            beyond.message(),
            "refused: TooLarge at spec.routes: it holds more than 4096 `[` or `{` bytes, more \
             than the credential walk reads"
        );

        let long = |bytes: usize| {
            let mut connectors = String::from("token = \"xoxb-000-fake\"\npad = \"");
            let pad = bytes.saturating_sub(connectors.len() + 2);
            connectors.push_str(&"a".repeat(pad));
            connectors.push_str("\"\n");
            admitted(&spec(Some(&connectors))).expect_err("refused")
        };
        assert_eq!(long(MAX_WALKED_BYTES - 64).reason, "SecretLiteral");
        let beyond = long(MAX_WALKED_BYTES + 64);
        assert_eq!(beyond.reason, "TooLarge");
        assert_eq!(beyond.member, "spec.connectorsConfig");
        assert!(!beyond.message().contains("xoxb-000-fake"));
    }

    #[test]
    fn the_default_routes_read_as_llm_routes() {
        let route = ModelRoute {
            url: "http://model-route.agents.svc:8080/v1".to_owned(),
            protocol: "messages".to_owned(),
            model: "small-model".to_owned(),
        };
        let decoded = parse(
            "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml, \
             roles: [{role: agent, model: small}]}\n",
        )
        .expect("a spec");
        Routes::parse(&default_routes(&route, &decoded)).expect("llm reads the default routes");
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
