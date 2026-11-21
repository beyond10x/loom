//! Security review, wave 2026-11-21-w6 unit U2 (`story:boundary-admits-declared-http-egress`).
//!
//! Integrity conformance checks of admission step 5 beside `spec.boundary`: a namespace egress
//! entry is admitted only when it names the namespace of no upstream the configuration holds, has
//! a selector, is tied to a GET `http` binding without a bearer in its namespace and equals one
//! `allowedBoundaryHttpEgress` entry; the rendered peer is no wider than that namespace, selector
//! and port; and the configuration refuses what is out of shape.

use std::fs;
use std::path::PathBuf;

use b10x_loom_operator::admission::Refusal;
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_yaml_ng::Value;

const MODEL: &str = "http://llm-gateway.llm.svc:8080";
const CONNECTORS: &str = "http://connectors.connectors.svc:8080";
const MODEL_ROUTE: &str = "http://model-route.agents.svc:8080/v1";

/// The base configuration: a model route, a model upstream (declared by the resources below) and
/// a Connectors upstream (never declared by them).
fn config_text(model_route: &str, connectors_url: &str, http_egress: &[&str]) -> String {
    format!(
        "namespace: agents
defaultImage: {{repository: registry.example/loom-agent, tag: '0.40.0'}}
defaultResources: {{cpu: 100m, memory: 128Mi}}
defaultCommandResources: {{cpu: 100m, memory: 128Mi}}
modelRoute: {{url: '{model_route}', protocol: messages, model: small}}
allowedBoundaryUpstreams:
- {{kind: model, url: '{MODEL}', audience: llm-gateway}}
- {{kind: connectors, url: '{connectors_url}', audience: connectors}}
allowedBoundaryHttpEgress: [{}]
",
        http_egress.join(", ")
    )
}

fn config(http_egress: &[&str]) -> Config {
    Config::parse(&config_text(MODEL_ROUTE, CONNECTORS, http_egress)).expect("the configuration")
}

/// A GET `http` binding `name` at `url` with no bearer.
fn get(name: &str, url: &str) -> String {
    format!(
        "{{name: {name}, kind: http, base_url: '{url}', allow_paths: [/api/v1/query], method: GET, \
         max_bytes: 262144}}"
    )
}

/// A `Command` resource with a model-only boundary, `bindings` and `egress` (flow lists' items).
fn resource(bindings: &[String], egress: &[&str]) -> String {
    format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata: {{name: nightly, namespace: agents}}
spec:
  agent:
    format: loom.agent/1
    agent: {{id: nightly, name: Nightly, brief: Runs the nightly check.}}
    model: {{routes: routes.yaml, roles: [{{role: agent, model: small}}]}}
    bindings: [{}]
    deploy:
      kubernetes:
        namespace: agents
        service_account: nightly
        resources: {{cpu: 500m, memory: 512Mi}}
        egress: [{}]
  routes: {{}}
  command: {{args: [--once]}}
  boundary:
    model: {{upstream: '{MODEL}', audience: llm-gateway}}
",
        bindings.join(", "),
        egress.join(", ")
    )
}

fn outcome(resource: &str, config: &Config) -> Result<String, Refusal> {
    match render_document(resource, config) {
        Ok(rendered) => Ok(rendered),
        Err(RenderError::Refused(refusal)) => Err(refusal),
        Err(other) => panic!("not rendered: {other}"),
    }
}

fn refused(resource: &str, config: &Config, case: &str) -> Refusal {
    match outcome(resource, config) {
        Ok(_) => panic!("{case}: admitted"),
        Err(refusal) => refusal,
    }
}

fn documents(rendered: &str) -> Vec<Value> {
    rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("YAML"))
        .collect()
}

fn of_kind(rendered: &str, kind: &str) -> Value {
    documents(rendered)
        .into_iter()
        .find(|document| document["kind"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("a {kind}"))
}

fn egress_rules(rendered: &str) -> Vec<Value> {
    of_kind(rendered, "NetworkPolicy")["spec"]["egress"]
        .as_sequence()
        .expect("egress rules")
        .clone()
}

/// Brief invariant 2: an entry naming an allowed upstream's namespace in another spelling is
/// never admitted, even with a binding and an allow-list entry for that spelling.
#[test]
fn an_upstream_namespace_in_any_spelling_is_never_admitted() {
    for (spelt, binding_ns) in [
        ("LLM", "llm"),
        ("llm.", "llm"),
        ("' llm'", "llm"),
        ("'llm '", "llm"),
        ("\u{217C}lm", "llm"),
        ("Connectors", "connectors"),
        ("connectors.", "connectors"),
    ] {
        let entry = format!(
            "{{name: e, namespace: {spelt}, selector: [{{name: app, value: read}}], port: 3100}}"
        );
        let allowed =
            format!("{{namespace: {spelt}, selector: [{{name: app, value: read}}], port: 3100}}");
        let config = config(&[&allowed]);
        let refusal = refused(
            &resource(
                &[get("read", &format!("http://read.{binding_ns}.svc:3100"))],
                &[&entry],
            ),
            &config,
            spelt,
        );
        assert_eq!(
            (refusal.reason.as_str(), refusal.member.as_str()),
            (
                "BoundaryEgressNamespace",
                "spec.agent.deploy.kubernetes.egress[0].namespace"
            ),
            "{spelt}: {}",
            refusal.message()
        );
    }
}

/// Brief invariant 2: every spelling of an `allowedBoundaryUpstreams` URL that names a cluster
/// service closes its namespace.
#[test]
fn an_upstream_url_in_any_cluster_service_spelling_closes_its_namespace() {
    for url in [
        "http://gw.store.svc",
        "http://gw.store.svc:8080/path",
        "http://gw.store.svc.cluster.local.:8080",
        "http://GW.STORE.SVC:8080",
        "https://gw.store.svc",
        "http://user@gw.store.svc:8080",
    ] {
        let config = Config::parse(&config_text(
            MODEL_ROUTE,
            url,
            &["{namespace: store, selector: [{name: app, value: read}], port: 3100}"],
        ))
        .expect("the configuration");
        let refusal = refused(
            &resource(
                &[get("read", "http://read.store.svc:3100")],
                &["{name: e, namespace: store, selector: [{name: app, value: read}], port: 3100}"],
            ),
            &config,
            url,
        );
        assert_eq!(
            refusal.reason,
            "BoundaryEgressNamespace",
            "{url}: {}",
            refusal.message()
        );
    }
}

/// Brief invariant 2, the open-door question: an `allowedBoundaryUpstreams` URL whose host is a
/// short service name (`gw.store`, which in-cluster DNS resolves to the Service `gw` of the
/// namespace `store`) names that namespace all the same. Pass 2: the configuration read refuses
/// it, naming the member; and admission, given a configuration that holds it anyway (built
/// without the read), still admits no entry in a namespace it cannot rule out.
#[test]
fn an_upstream_named_by_a_short_service_name_still_closes_its_namespace() {
    let allowed = ["{namespace: store, selector: [{name: app, value: read}], port: 3100}"];
    let error = Config::parse(&config_text(MODEL_ROUTE, "http://gw.store:8080", &allowed))
        .expect_err("the configuration read refuses a short service name");
    assert!(
        error
            .0
            .starts_with("allowedBoundaryUpstreams[1].url: \"http://gw.store:8080\""),
        "{error}"
    );
    let mut config = config(&allowed);
    config.allowed_boundary_upstreams[1].url = "http://gw.store:8080".to_owned();
    let answer = outcome(
        &resource(
            &[get("read", "http://read.store.svc:3100")],
            &["{name: e, namespace: store, selector: [{name: app, value: read}], port: 3100}"],
        ),
        &config,
    );
    match answer {
        Err(refusal) => assert_eq!(
            refusal.reason,
            "BoundaryEgressNamespace",
            "{}",
            refusal.message()
        ),
        Ok(_) => panic!(
            "an entry in the namespace of the allowedBoundaryUpstreams URL http://gw.store:8080 is \
             admitted: condition 1 reads no namespace from a short service name"
        ),
    }
}

/// Brief invariant 2, "every other upstream the configuration can hold": the configuration's
/// `modelRoute` is an in-cluster endpoint that "needs no credential from the agent (a signing
/// proxy holds it)" (`config.rs`), which a boundary pod is given no rule to. An entry naming its
/// namespace, selecting its pods on its port and tied to a GET binding at it is admitted, so the
/// agent container reaches a credential-holding proxy past the boundary.
#[test]
fn the_model_route_namespace_is_not_opened_beside_a_boundary() {
    let config =
        config(&["{namespace: agents, selector: [{name: app, value: model-route}], port: 8080}"]);
    let answer = outcome(
        &resource(
            &[get("route", "http://model-route.agents.svc:8080")],
            &[
                "{name: route, namespace: agents, selector: [{name: app, value: model-route}], port: 8080}",
            ],
        ),
        &config,
    );
    match answer {
        Err(refusal) => assert_eq!(
            refusal.reason,
            "BoundaryEgressNamespace",
            "{}",
            refusal.message()
        ),
        Ok(rendered) => panic!(
            "an entry opening the modelRoute's namespace (agents) on its port is admitted beside a \
             boundary: {:?}",
            egress_rules(&rendered)
        ),
    }
}

/// Pass 2, point 1: every spelling of a `modelRoute` URL the configuration read accepts closes
/// the namespace admission reads from it, in every spelling of the entry's namespace, with the
/// same reason, member and detail as an upstream's namespace; and the resource's own
/// `spec.routes` does not change the model route.
#[test]
fn the_model_route_namespace_in_any_spelling_is_closed() {
    let allowed = |namespace: &str| {
        format!("{{namespace: {namespace}, selector: [{{name: app, value: read}}], port: 3100}}")
    };
    for url in [
        "http://mr.store.svc",
        "http://mr.store.svc:8080/v1",
        "http://mr.store.svc.cluster.local.:8080/v1",
        "http://MR.STORE.SVC:8080/v1",
        "https://mr.store.svc/v1",
        "http://user@mr.store.svc:8080/v1",
    ] {
        for namespace in ["store", "STORE", "store."] {
            let config = Config::parse(&config_text(url, CONNECTORS, &[&allowed(namespace)]))
                .unwrap_or_else(|error| panic!("{url}: {error}"));
            let entry = format!(
                "{{name: e, namespace: {namespace}, selector: [{{name: app, value: read}}], port: \
                 3100}}"
            );
            let text = resource(&[get("read", "http://read.store.svc:3100")], &[&entry]).replace(
                "  routes: {}\n",
                "  routes:\n    format: llm.routes/1\n    endpoints:\n      lab: {base_url: \
                 'http://lab.example/v1', protocol: messages, auth: {kind: anonymous}}\n    \
                 routes: {small: lab/small}\n",
            );
            let refusal = refused(&text, &config, &format!("{namespace} under {url}"));
            assert_eq!(
                refusal.message(),
                "refused: BoundaryEgressNamespace at spec.agent.deploy.kubernetes.egress[0].namespace: \
                 a resource with spec.boundary reaches only DNS and its boundary's upstreams, so its \
                 NetworkPolicy opens no namespace",
                "{namespace} under {url}"
            );
        }
    }
}

/// Brief invariant 1, condition 2: an absent or empty selector is refused at `selector` even
/// with a binding in that namespace.
#[test]
fn an_entry_without_a_selector_is_refused_at_its_selector() {
    for entry in [
        "{name: e, namespace: monitoring, port: 3100}",
        "{name: e, namespace: monitoring, selector: [], port: 3100}",
    ] {
        let refusal = refused(
            &resource(&[get("logs", "http://logs.monitoring.svc:3100")], &[entry]),
            &config(&["{namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}"]),
            entry,
        );
        assert_eq!(
            (refusal.reason.as_str(), refusal.member.as_str()),
            (
                "BoundaryEgressNamespace",
                "spec.agent.deploy.kubernetes.egress[0].selector"
            ),
            "{entry}: {}",
            refusal.message()
        );
    }
}

/// Brief invariant 1, condition 3: each entry needs its own tie; a binding in one namespace ties
/// no entry of another.
#[test]
fn each_entry_needs_a_binding_in_its_own_namespace() {
    let refusal = refused(
        &resource(
            &[get("logs", "http://logs.monitoring.svc:3100")],
            &[
                "{name: a, namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}",
                "{name: b, namespace: observability, selector: [{name: app, value: x}], port: 80}",
            ],
        ),
        &config(&[
            "{namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}",
            "{namespace: observability, selector: [{name: app, value: x}], port: 80}",
        ]),
        "two entries",
    );
    assert_eq!(
        (refusal.reason.as_str(), refusal.member.as_str()),
        (
            "BoundaryEgressUnbound",
            "spec.agent.deploy.kubernetes.egress[1]"
        ),
        "{}",
        refusal.message()
    );
}

/// Brief invariant 3: a binding whose `base_url` is no plain cluster service in the entry's
/// namespace, a non-GET binding (a `Command` resource skips the document check), a bearer and a
/// Connectors binding tie nothing.
#[test]
fn a_binding_out_of_shape_ties_no_entry() {
    let entry =
        "{name: e, namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}";
    let config =
        config(&["{namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}"]);
    let mut bindings: Vec<(String, String)> = [
        "http://logs.monitoring.svc.evil.example:3100",
        "http://logs.monitoring.svc.cluster.local.evil.example",
        "http://monitoring.svc.cluster.local:3100",
        "http://logs.monitoring.svc.:3100",
        "http://10.0.0.1:3100",
        "http://LOGS.MONITORING.SVC:3100",
        "https://logs.monitoring.svc",
        "http://logs.monitoring.svc.cluster.example:3100",
        "http://%6cogs.monitoring.svc:3100",
        "http://.monitoring.svc:3100",
        "http://logs.monitoring.svc:3100/../x",
        "http://logs.monitoring.svc:3100/a@b",
        "http://logs.monitoring.svc:3100#f",
        "http://logs.other.svc:3100",
    ]
    .iter()
    .map(|url| ((*url).to_owned(), get("logs", url)))
    .collect();
    bindings.push((
        "POST".to_owned(),
        "{name: logs, kind: http, base_url: 'http://logs.monitoring.svc:3100', allow_paths: \
         [/api/v1/push], method: POST, max_bytes: 262144}"
            .to_owned(),
    ));
    bindings.push((
        "bearer".to_owned(),
        "{name: logs, kind: http, base_url: 'http://logs.monitoring.svc:3100', allow_paths: \
         [/api/v1/query], method: GET, max_bytes: 262144, bearer_env: LOGS_TOKEN}"
            .to_owned(),
    ));
    for (case, binding) in bindings {
        let refusal = refused(&resource(&[binding], &[entry]), &config, &case);
        println!("{case}: {}", refusal.message());
        assert!(
            refusal.reason != "BoundaryEgressNotAllowed",
            "{case}: reached condition 4, so condition 3 held: {}",
            refusal.message()
        );
    }
}

/// Brief invariant 4: one allowed entry must equal namespace, selector and port at once; a label
/// value in another case, a namespace in another case and values of two entries do not match.
#[test]
fn the_allow_list_matches_one_entry_exactly() {
    let allowed = [
        "{namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}",
        "{namespace: monitoring, selector: [{name: app, value: metrics}], port: 9090}",
        "{namespace: Observability, selector: [{name: app, value: logs}], port: 3100}",
    ];
    for (entry, binding) in [
        (
            "{name: e, namespace: monitoring, selector: [{name: app, value: Logs}], port: 3100}",
            "http://logs.monitoring.svc:3100",
        ),
        (
            "{name: e, namespace: monitoring, selector: [{name: app, value: logs}], port: 9090}",
            "http://logs.monitoring.svc:9090",
        ),
        (
            "{name: e, namespace: monitoring, selector: [{name: App, value: logs}], port: 3100}",
            "http://logs.monitoring.svc:3100",
        ),
        (
            "{name: e, namespace: observability, selector: [{name: app, value: logs}], port: 3100}",
            "http://logs.observability.svc:3100",
        ),
    ] {
        let refusal = refused(
            &resource(&[get("b", binding)], &[entry]),
            &config(&allowed),
            entry,
        );
        assert_eq!(
            (refusal.reason.as_str(), refusal.member.as_str()),
            (
                "BoundaryEgressNotAllowed",
                "spec.agent.deploy.kubernetes.egress[0]"
            ),
            "{entry}: {}",
            refusal.message()
        );
    }
}

/// Brief invariants 5 and 6: an admitted entry (here with a label repeated, which the set
/// comparison folds) renders exactly one rule more, a peer of its namespace and exactly its
/// labels on one TCP port; the other rules and the pod are what they are without it.
#[test]
fn an_admitted_entry_renders_one_peer_no_wider_and_changes_nothing_else() {
    let config =
        config(&["{namespace: monitoring, selector: [{name: app, value: logs}], port: 3100}"]);
    let binding = get("logs", "http://logs.monitoring.svc:3100");
    let without = outcome(&resource(std::slice::from_ref(&binding), &[]), &config)
        .unwrap_or_else(|refusal| panic!("without: {}", refusal.message()));
    let with = outcome(
        &resource(
            std::slice::from_ref(&binding),
            &["{name: logs, namespace: monitoring, selector: [{name: app, value: logs}, {name: app, \
               value: logs}], port: 3100}"],
        ),
        &config,
    )
    .unwrap_or_else(|refusal| panic!("with: {}", refusal.message()));
    let before = egress_rules(&without);
    let after = egress_rules(&with);
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    let expected: Value = serde_yaml_ng::from_str(
        "to:\n- namespaceSelector: {matchLabels: {kubernetes.io/metadata.name: monitoring}}\n  \
         podSelector: {matchLabels: {app: logs}}\nports:\n- {port: 3100, protocol: TCP}\n",
    )
    .expect("a rule");
    let added: Vec<&Value> = after.iter().filter(|rule| !before.contains(rule)).collect();
    assert_eq!(added, [&expected], "{after:?}");
    for rule in &after {
        assert!(
            rule["ports"]
                .as_sequence()
                .is_some_and(|ports| !ports.is_empty()),
            "{rule:?}"
        );
        for peer in rule["to"].as_sequence().into_iter().flatten() {
            assert!(peer["ipBlock"].is_null(), "{rule:?}");
            assert!(!peer["namespaceSelector"].is_null(), "{rule:?}");
        }
    }
    let template = |rendered: &str| of_kind(rendered, "Job")["spec"]["template"].clone();
    assert_eq!(template(&without), template(&with), "the pod is unchanged");
    for kind in ["ServiceAccount", "ConfigMap"] {
        let strip = |rendered: &str| {
            let mut document = of_kind(rendered, kind);
            if let Some(data) = document["data"].as_mapping_mut() {
                data.remove("agent.yaml");
            }
            document
        };
        assert_eq!(strip(&without), strip(&with), "{kind}");
    }
}

/// Brief invariant 8: the base version of
/// `adversary_d243_u1::an_egress_namespace_entry_does_not_open_an_upstream_namespace_on_another_port`
/// (an `AgentServe` resource, `routes: {}`, a Connectors-only boundary) returned on any refusal.
/// Its resource is refused at step 4, before step 5, which this unit does not change, so at the
/// base the case never reached the egress check.
#[test]
fn the_base_adversary_case_was_refused_before_the_egress_check() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let example =
        fs::read_to_string(root.join("examples/agents/operator.yaml")).expect("the example");
    let mut agent = String::new();
    for line in example.lines() {
        if line.contains("{name: chat-api,") || line.contains("{name: model-api,") {
            continue;
        }
        if line.contains("{name: logs, namespace: monitoring,") {
            agent.push_str("      - {name: connectors-store, namespace: connectors, port: 5432}\n");
        }
        agent.push_str(line);
        agent.push('\n');
    }
    let mut text = String::from(
        "apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata:\n  name: \
         platform-operator\n  namespace: agents\n  uid: 00000000-0000-4000-8000-000000000001\n  \
         generation: 1\nspec:\n  agent:\n",
    );
    for line in agent.lines() {
        if !line.is_empty() {
            text.push_str("    ");
            text.push_str(line);
        }
        text.push('\n');
    }
    text.push_str(&format!(
        "  routes: {{}}\n  boundary:\n    connectors: {{upstream: '{CONNECTORS}', audience: \
         connectors, operations: [merge_request.get]}}\n"
    ));
    let config = Config::parse(&format!(
        "namespace: agents\ndefaultImage: {{repository: registry.example/loom-agent, tag: \
         '0.40.0'}}\ndefaultResources: {{cpu: 100m, memory: 128Mi}}\ndefaultCommandResources: \
         {{cpu: 100m, memory: 128Mi}}\nmodelRoute: {{url: '{MODEL_ROUTE}', protocol: messages, \
         model: small-model}}\nallowedSecretNames: [platform-operator-secrets, takt-forge]\n\
         allowedEgressHosts: [chat.example, models.example]\nallowedClaimNames: \
         [takt-workspace]\nallowedBoundaryUpstreams:\n- {{kind: model, url: '{MODEL}', audience: \
         llm-gateway}}\n- {{kind: connectors, url: '{CONNECTORS}', audience: connectors}}\n- \
         {{kind: git, url: 'http://connectors.connectors.svc/git', audience: connectors-git}}\n"
    ))
    .expect("the adversary configuration");
    let refusal = refused(&text, &config, "the base adversary case");
    assert_eq!(refusal.reason, "NoModelRoute", "{}", refusal.message());
}

/// Brief invariant 4 and the chart: an entry of `allowedBoundaryHttpEgress` out of shape (a
/// selector or a port given as a string, a port beyond 65535 or negative, an unknown member of
/// the list's entries) refuses the configuration; absent, the member reads as an empty list.
#[test]
fn the_configuration_refuses_an_http_egress_entry_out_of_shape() {
    for entry in [
        "{namespace: m, selector: 'app=logs', port: 3100}",
        "{namespace: m, selector: [{name: app, value: logs}], port: '3100'}",
        "{namespace: m, selector: [{name: app, value: logs}], port: 70000}",
        "{namespace: m, selector: [{name: app, value: logs}], port: -1}",
        "{namespace: m, selector: [{name: app, value: logs}], port: 3100, protocol: UDP}",
        "{namespace: m, selector: [{name: app, value: logs}], port: 3100, endPort: 3200}",
        "{namespace: m, selector: {app: logs}, port: 3100}",
    ] {
        assert!(
            Config::parse(&config_text(MODEL_ROUTE, CONNECTORS, &[entry])).is_err(),
            "{entry} is read"
        );
    }
    let text = config_text(MODEL_ROUTE, CONNECTORS, &[]);
    let absent = text.replace("allowedBoundaryHttpEgress: []\n", "");
    assert_ne!(absent, text);
    assert_eq!(
        Config::parse(&absent).expect("absent"),
        Config::parse(&text).expect("empty")
    );
}
