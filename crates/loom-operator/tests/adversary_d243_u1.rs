//! Adversary pass 1 on unit u1 of wave 2026-10-30-w1 (`story:boundary-container-projected-token`,
//! `story:credential-free-network-policy`, epic `epic:credential-free-agent-pod`).
//!
//! The author of a `LoomAgent` is the adversary: the cases drive the operator with resources such
//! an author may write, under the controller configuration README.md § credential-free documents
//! (the gateway, the Connectors wire and its git path in `allowedBoundaryUpstreams`), and hold the
//! result to the stories' and the epic's outcome text and to the relay the sidecar runs
//! (`b10x-loom-boundary`, the crate `b10x-loom agent boundary` builds its relays with).

use std::fs;
use std::path::PathBuf;

use b10x_loom_boundary::{Relay as BoundaryRelay, Upstream as BoundaryUpstream};
use b10x_loom_operator::config::Config;
use b10x_loom_operator::render::{RenderError, render_document};
use serde_yaml_ng::Value;

/// The model gateway, as README.md's credential-free example names it.
const MODEL: &str = "http://llm-gateway.llm.svc:8080";
/// The Connectors wire, as README.md's credential-free example names it.
const CONNECTORS: &str = "http://connectors.connectors.svc:8080";
/// The git smart-HTTP path on the Connectors instance, as README.md's example names it.
const GIT: &str = "http://connectors.connectors.svc/git";

/// The configuration of `tests/render.rs`, with the three upstreams README.md's example lists.
const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets, takt-forge]
allowedEgressHosts: [chat.example, models.example]
allowedClaimNames: [takt-workspace]
";

fn config() -> Config {
    Config::parse(&format!(
        "{CONFIG}allowedBoundaryUpstreams: ['{MODEL}', '{CONNECTORS}', '{GIT}']\n"
    ))
    .expect("the test configuration")
}

fn example() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read_to_string(root.join("examples/agents/operator.yaml")).expect("the example spec")
}

/// The example agent without its two host egress entries, which a boundary refuses; `extra`
/// (egress entries, each a line at the list's indentation) is added before its namespace entries.
fn credential_free_agent(extra: &str) -> String {
    let mut out = String::new();
    for line in example().lines() {
        if line.contains("{name: chat-api,") || line.contains("{name: model-api,") {
            continue;
        }
        if line.contains("{name: logs, namespace: monitoring,") {
            out.push_str(extra);
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// An `AgentServe` resource running `agent`, with `rest` (further `spec` members) after it.
fn serve(agent: &str, rest: &str) -> String {
    let mut out = String::from(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: platform-operator
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000001
  generation: 1
spec:
  agent:
",
    );
    for line in agent.lines() {
        if !line.is_empty() {
            out.push_str("    ");
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str(rest);
    out
}

/// A `Command` resource without `spec.agent`: an image, its arguments, the default routes,
/// then `rest`.
fn command(rest: &str) -> String {
    format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata:
  name: takt
  namespace: agents
  uid: 00000000-0000-4000-8000-000000000002
  generation: 1
spec:
  routes: {{}}
  image: {{repository: registry.example/takt, tag: '0.12.0'}}
  command: {{args: [--iterations, '1']}}
{rest}"
    )
}

fn documents(rendered: &str) -> Vec<Value> {
    rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("each document is YAML"))
        .collect()
}

fn of_kind(rendered: &str, kind: &str) -> Value {
    documents(rendered)
        .into_iter()
        .find(|document| document["kind"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("a {kind} in {rendered}"))
}

/// An egress rule: the namespace its peer selects (none for a rule without a peer) and its ports.
type EgressRule = (Option<String>, Vec<(String, i64)>);

/// Each egress rule of the rendered NetworkPolicy: the namespace its peer selects (none for a
/// rule without a peer) and its ports as `(protocol, port)`.
fn egress(rendered: &str) -> Vec<EgressRule> {
    let policy = of_kind(rendered, "NetworkPolicy");
    policy["spec"]["egress"]
        .as_sequence()
        .expect("egress rules")
        .iter()
        .map(|rule| {
            let namespace = rule["to"].as_sequence().map(|peers| {
                peers[0]["namespaceSelector"]["matchLabels"]["kubernetes.io/metadata.name"]
                    .as_str()
                    .expect("a namespace")
                    .to_owned()
            });
            let ports = rule["ports"]
                .as_sequence()
                .expect("ports")
                .iter()
                .map(|port| {
                    (
                        port["protocol"].as_str().expect("a protocol").to_owned(),
                        port["port"].as_i64().expect("a port number"),
                    )
                })
                .collect();
            (namespace, ports)
        })
        .collect()
}

/// The `<service>.<namespace>.svc[.cluster.local][:port]` authority of `url`, as namespace and
/// TCP port (80 when it names none).
fn namespace_and_port(url: &str) -> (String, i64) {
    let rest = url.strip_prefix("http://").expect("an http URL");
    let authority = rest.split('/').next().expect("an authority");
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse::<i64>().expect("a port")),
        None => (authority, 80),
    };
    let namespace = host.split('.').nth(1).expect("a namespace label");
    (namespace.to_owned(), port)
}

/// The boundary sidecar's arguments in the rendered pod.
fn boundary_args(rendered: &str) -> Vec<String> {
    let documents = documents(rendered);
    let pod = &documents[2]["spec"]["template"]["spec"];
    let sidecar = pod["initContainers"]
        .as_sequence()
        .expect("init containers")
        .iter()
        .find(|container| container["name"].as_str() == Some("boundary"))
        .expect("the boundary sidecar");
    sidecar["args"]
        .as_sequence()
        .expect("args")
        .iter()
        .map(|arg| arg.as_str().expect("a string").to_owned())
        .collect()
}

/// Builds every relay the sidecar's `args` declare with `b10x-loom-boundary`, as `b10x-loom
/// agent boundary` does (`crates/loom-cli/src/main.rs`, `boundary_relays`), and names the first
/// it refuses.
fn relays_from(args: &[String]) -> Result<usize, String> {
    let value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .map(|at| args[at + 1].clone())
    };
    let values = |flag: &str| -> Vec<String> {
        args.windows(2)
            .filter(|pair| pair[0] == flag)
            .map(|pair| pair[1].clone())
            .collect()
    };
    let upstream = |flag: &str, url: &str| {
        BoundaryUpstream::parse(url).map_err(|refused| format!("{flag}: {refused}"))
    };
    let mut built: Vec<BoundaryRelay> = Vec::new();
    if let (Some(url), Some(token)) = (value("--model-upstream"), value("--model-token")) {
        built.push(BoundaryRelay::model(
            upstream("--model-upstream", &url)?,
            token,
        ));
    }
    if let (Some(url), Some(token)) = (value("--connectors-upstream"), value("--connectors-token"))
    {
        built.push(
            BoundaryRelay::connectors(
                upstream("--connectors-upstream", &url)?,
                token,
                values("--operation"),
            )
            .map_err(|refused| format!("--operation: {refused}"))?,
        );
    }
    if let (Some(url), Some(token)) = (value("--git-upstream"), value("--git-token")) {
        built.push(
            BoundaryRelay::git(
                upstream("--git-upstream", &url)?,
                token,
                values("--repository"),
            )
            .map_err(|refused| format!("--repository: {refused}"))?,
        );
    }
    Ok(built.len())
}

/// Epic `credential-free-agent-pod` § Acceptance: the boundary relays Connectors "for declared
/// operations ... only", and README.md says the same of the Connectors relay (8402). The model
/// relay forwards any method on any path under `/v1/` (`crates/loom-boundary/src/route.rs`,
/// `model`), and the Connectors host serves `POST /v1/invoke` beside `/v1alpha2/invoke`
/// (`connectors-host/src/server.rs`). `allowedBoundaryUpstreams` is one list for every kind, so an
/// author may name the Connectors upstream as `spec.boundary.model.upstream` with the Connectors
/// audience: the agent then invokes any operation through 8401, whatever
/// `spec.boundary.connectors.operations` says.
#[test]
fn the_connectors_upstream_cannot_be_declared_as_the_model_relay() {
    let hidden = format!(
        "  boundary:\n    model: {{upstream: '{CONNECTORS}', audience: connectors}}\n    \
         connectors: {{upstream: '{CONNECTORS}', audience: connectors, operations: \
         [merge_request.get]}}\n"
    );
    let alone =
        format!("  boundary:\n    model: {{upstream: '{CONNECTORS}', audience: connectors}}\n");
    for (what, rest) in [
        ("beside a connectors relay declaring one operation", hidden),
        ("as the only relay", alone),
    ] {
        match render_document(&command(&rest), &config()) {
            Err(RenderError::Refused(_)) => {}
            other => panic!(
                "DEFECT: the Connectors upstream is admitted as spec.boundary.model.upstream \
                 ({what}): allowedBoundaryUpstreams is matched without the relay kind \
                 (admission.rs:1258), so the model relay, which forwards every method under \
                 /v1/, reaches Connectors' POST /v1/invoke for undeclared operations; \
                 rendered: {}",
                other.map(|_| "admitted").unwrap_or("not a refusal")
            ),
        }
    }
}

/// `story:credential-free-network-policy` § Outcome: the NetworkPolicy allows "for each declared
/// upstream, ... its namespace on its port, and nothing else"; design D243 § 2: "DNS plus the
/// credential namespace on the declared ports". A namespace egress entry naming the upstream's
/// namespace on another port (a database beside the Connectors instance) opens that port too.
#[test]
fn an_egress_namespace_entry_does_not_open_an_upstream_namespace_on_another_port() {
    let entry = "      - {name: connectors-store, namespace: connectors, port: 5432}\n";
    let agent = credential_free_agent(entry);
    assert!(agent.contains(entry), "the entry sits in the egress list");
    let resource = serve(
        &agent,
        &format!(
            "  routes: {{}}\n  boundary:\n    connectors: {{upstream: '{CONNECTORS}', audience: \
             connectors, operations: [merge_request.get]}}\n"
        ),
    );
    let rendered = match render_document(&resource, &config()) {
        Err(RenderError::Refused(_)) => return,
        Err(other) => panic!("not rendered: {other}"),
        Ok(rendered) => rendered,
    };
    let (namespace, declared) = namespace_and_port(CONNECTORS);
    let opened: Vec<i64> = egress(&rendered)
        .into_iter()
        .filter(|(peer, _)| peer.as_deref() == Some(namespace.as_str()))
        .flat_map(|(_, ports)| ports.into_iter().map(|(_, port)| port))
        .collect();
    assert!(
        opened.iter().all(|port| *port == declared),
        "DEFECT: a credential-free pod's NetworkPolicy opens the upstream namespace {namespace} \
         on {opened:?}, not only on the declared {declared}: the spec's namespace egress entries \
         are rendered beside a boundary unfiltered (render.rs:1043), so the agent container \
         reaches the credential namespace's other services past the boundary"
    );
}

/// `README.md`: "with `boundary.model` the default routes name the model relay"; without it they
/// name the controller's model route, and `story:credential-free-network-policy` drops the
/// model-route rule beside any boundary (render.rs:1076). A credential-free serve agent with
/// Connectors and git but no model relay is admitted with routes no rule of its NetworkPolicy
/// reaches: every model call is dropped, and nothing refuses it.
#[test]
fn default_routes_beside_a_boundary_without_a_model_relay_name_a_reachable_endpoint() {
    let resource = serve(
        &credential_free_agent(""),
        &format!(
            "  routes: {{}}\n  boundary:\n    connectors: {{upstream: '{CONNECTORS}', audience: \
             connectors, operations: [merge_request.get]}}\n    git: {{upstream: '{GIT}', \
             audience: connectors-git, repositories: [agents/takt-target]}}\n"
        ),
    );
    let rendered = match render_document(&resource, &config()) {
        Err(RenderError::Refused(_)) => return,
        Err(other) => panic!("not rendered: {other}"),
        Ok(rendered) => rendered,
    };
    let config_map = of_kind(&rendered, "ConfigMap");
    let routes: Value = serde_yaml_ng::from_str(
        config_map["data"]["routes.yaml"]
            .as_str()
            .expect("routes.yaml"),
    )
    .expect("routes.yaml is YAML");
    assert!(
        routes["routes"]
            .as_mapping()
            .is_some_and(|routes| !routes.is_empty()),
        "the agent's roles are routed: {routes:?}"
    );
    let rules = egress(&rendered);
    for (name, endpoint) in routes["endpoints"].as_mapping().expect("endpoints") {
        let base_url = endpoint["base_url"].as_str().expect("a base_url");
        if base_url.starts_with("http://127.0.0.1:") {
            continue;
        }
        let (namespace, port) = namespace_and_port(base_url);
        let reached = rules.iter().any(|(peer, ports)| {
            peer.as_ref().is_none_or(|peer| *peer == namespace)
                && ports.contains(&("TCP".to_owned(), port))
        });
        assert!(
            reached,
            "DEFECT: the default routes' endpoint {name:?} is {base_url}, and the credential-free \
             NetworkPolicy {rules:?} opens neither {namespace} on {port} nor the port: a boundary \
             without `model` keeps the model route in routes.yaml (admission.rs:1422) while the \
             policy drops its rule (render.rs:1076), so the admitted agent reaches no model"
        );
    }
}

/// AGENTS.md § Operator: the sidecar's flags "are the contract with `b10x-loom agent boundary`".
/// Every boundary admission admits must be a command line the relay crate accepts, else the
/// sidecar exits 2 on every start, its startup probe never passes and the agent never starts,
/// with the resource admitted and no refusal in its status.
#[test]
fn every_admitted_boundary_is_a_command_line_the_relay_accepts() {
    let long_operation = format!("merge_request.{}", "x".repeat(243));
    assert_eq!(long_operation.len(), 257);
    let cases = [
        (
            "README.md's example",
            format!(
                "  boundary:\n    model: {{upstream: '{MODEL}', audience: llm-gateway}}\n    \
                 connectors: {{upstream: '{CONNECTORS}', audience: connectors, operations: \
                 [merge_request.create]}}\n    git: {{upstream: '{GIT}', audience: \
                 connectors-git, repositories: [agents/takt-target]}}\n"
            ),
        ),
        (
            "an operation id of 257 bytes",
            format!(
                "  boundary:\n    connectors: {{upstream: '{CONNECTORS}', audience: connectors, \
                 operations: [{long_operation}]}}\n"
            ),
        ),
        (
            "a repository whose parts are dots and dashes",
            format!(
                "  boundary:\n    git: {{upstream: '{GIT}', audience: connectors-git, \
                 repositories: ['.a-/b.-_', 'a/...']}}\n"
            ),
        ),
    ];
    let mut checked = 0;
    for (what, rest) in cases {
        let rendered = match render_document(&command(&rest), &config()) {
            Err(RenderError::Refused(_)) => continue,
            Err(other) => panic!("{what}: not rendered: {other}"),
            Ok(rendered) => rendered,
        };
        let args = boundary_args(&rendered);
        if let Err(refused) = relays_from(&args) {
            panic!(
                "DEFECT: {what} is admitted, and b10x-loom agent boundary refuses the rendered \
                 command line ({refused}): admission's operation_id (admission.rs:1122) bounds no \
                 length while the relay admits 1 to 256 bytes (loom-boundary/src/lib.rs:247), so \
                 the sidecar exits 2 on every start and the agent never runs"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "at least README.md's example is admitted");
}

/// Attack that held: every upstream spelling admission admits names, for the NetworkPolicy, the
/// namespace and port the relay connects to (`Upstream::parse` lowercases the authority and reads
/// a port with leading zeros as its number), so the policy cannot open one namespace while the
/// boundary dials another.
#[test]
fn an_admitted_upstream_names_the_namespace_and_port_the_relay_dials() {
    for (url, namespace, port) in [
        ("http://gw.llm.svc:08080", "llm", 8080),
        ("http://gw.llm.svc.cluster.local:8080/v1", "llm", 8080),
        ("http://gw.llm.svc", "llm", 80),
        ("http://gw.llm.svc:00080/a/b", "llm", 80),
    ] {
        let config = Config::parse(&format!("{CONFIG}allowedBoundaryUpstreams: ['{url}']\n"))
            .expect("a configuration");
        let rendered = render_document(
            &command(&format!(
                "  boundary:\n    model: {{upstream: '{url}', audience: llm-gateway}}\n"
            )),
            &config,
        )
        .unwrap_or_else(|refused| panic!("{url}: {refused}"));
        let rules = egress(&rendered);
        assert_eq!(
            rules.last(),
            Some(&(
                Some(namespace.to_owned()),
                vec![("TCP".to_owned(), i64::from(port))]
            )),
            "{url}: {rules:?}"
        );
        let relay = BoundaryUpstream::parse(url).expect("the relay parses what admission admits");
        let authority = relay.authority();
        let (host, dialled) = match authority.split_once(':') {
            Some((host, digits)) => (host, digits.parse::<u16>().expect("a port")),
            None => (authority, 80),
        };
        assert_eq!(host.split('.').nth(1), Some(namespace), "{url}");
        assert_eq!(dialled, port, "{url}");
    }
}
