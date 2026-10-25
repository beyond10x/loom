//! Adversary cases for `story:loom-operator` (wave 2026-10-25-operator, unit u1).
//!
//! Each `adv_*` case drives the controller's own contract (the story's Outcome and Design, design
//! § 4, § 5.2 and § 7, `src/admission.rs`'s module documentation) through `reconcile` over a fake
//! API server, kube's mock client over `tower-test`, in-process, as `tests/reconcile.rs` does.
//! Every token below is a clearly fake one. Nothing reaches a cluster or a network.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use b10x_loom_operator::config::Config;
use b10x_loom_operator::reconcile::{Context, reconcile};
use b10x_loom_operator::render::render_document;
use b10x_loom_operator::resource::LoomAgent;
use http::{Method, Request, Response, StatusCode};
use kube::Client;
use kube::client::Body;
use serde_json::{Value, json};

const CONFIG: &str = "namespace: agents
defaultImage: {repository: registry.example/loom-agent, tag: '0.40.0'}
defaultResources: {cpu: 100m, memory: 128Mi}
modelRoute: {url: 'http://model-route.agents.svc:8080/v1', protocol: messages, model: small-model}
allowedSecretNames: [platform-operator-secrets]
allowedEgressHosts: [chat.example, models.example]
";

const DEFAULT_ROUTES: &str = "  routes: {}\n  secretRef: {name: platform-operator-secrets}\n";

const NAME: &str = "platform-operator";
const UID_A: &str = "00000000-0000-4000-8000-00000000000a";
const UID_B: &str = "00000000-0000-4000-8000-00000000000b";

const STATUS: &str =
    "/apis/loom.beyond10x.dev/v1alpha1/namespaces/agents/loomagents/platform-operator/status";

const CHILDREN: [&str; 5] = [
    "/api/v1/namespaces/agents/serviceaccounts/platform-operator",
    "/api/v1/namespaces/agents/configmaps/platform-operator-spec",
    "/apis/apps/v1/namespaces/agents/deployments/platform-operator",
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator",
    "/api/v1/namespaces/agents/services/platform-operator",
];

const DEPLOYMENT: &str = "/apis/apps/v1/namespaces/agents/deployments/platform-operator";

#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
    body: Value,
}

#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    /// Paths whose next DELETE answers 500, once.
    fail_delete_once: BTreeSet<String>,
}

impl Server {
    fn answer(&mut self, method: &Method, path: &str, body: &Value) -> (StatusCode, Value) {
        match *method {
            Method::PATCH if path.ends_with("/status") => (
                StatusCode::OK,
                json!({
                    "apiVersion": "loom.beyond10x.dev/v1alpha1",
                    "kind": "LoomAgent",
                    "metadata": body["metadata"],
                    "spec": {"agent": {}, "routes": {}},
                    "status": body["status"],
                }),
            ),
            Method::PATCH => {
                let mut object = body.clone();
                object["metadata"]["uid"] = json!(format!("uid-of-{path}"));
                self.objects.insert(path.to_owned(), object.clone());
                (StatusCode::OK, object)
            }
            Method::GET => match self.objects.get(path) {
                Some(object) => (StatusCode::OK, object.clone()),
                None => (
                    StatusCode::NOT_FOUND,
                    json!({
                        "kind": "Status", "apiVersion": "v1", "metadata": {},
                        "status": "Failure", "reason": "NotFound", "code": 404,
                        "message": "not found",
                    }),
                ),
            },
            Method::DELETE => {
                if self.fail_delete_once.remove(path) {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json!({
                            "kind": "Status", "apiVersion": "v1", "metadata": {},
                            "status": "Failure", "reason": "InternalError", "code": 500,
                            "message": "the fake fails this delete once",
                        }),
                    );
                }
                self.objects.remove(path);
                (
                    StatusCode::OK,
                    json!({
                        "kind": "Status", "apiVersion": "v1", "metadata": {},
                        "status": "Success", "code": 200,
                    }),
                )
            }
            _ => (StatusCode::METHOD_NOT_ALLOWED, json!({})),
        }
    }
}

struct Fake {
    server: Arc<Mutex<Server>>,
}

impl Fake {
    fn take(&self) -> Vec<Seen> {
        std::mem::take(&mut self.server.lock().expect("the fake").seen)
    }

    fn holds(&self, path: &str) -> bool {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .contains_key(path)
    }

    fn put(&self, path: &str, object: Value) {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .insert(path.to_owned(), object);
    }

    fn fail_delete_once(&self, path: &str) {
        self.server
            .lock()
            .expect("the fake")
            .fail_delete_once
            .insert(path.to_owned());
    }
}

fn fake() -> (Client, Fake) {
    let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let server = Arc::new(Mutex::new(Server::default()));
    let shared = Arc::clone(&server);
    tokio::spawn(async move {
        let mut handle = std::pin::pin!(handle);
        while let Some((request, send)) = handle.next_request().await {
            let (parts, body) = request.into_parts();
            let bytes = body.collect_bytes().await.expect("the request body");
            let body: Value = if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).expect("a JSON body")
            };
            let path = parts.uri.path().to_owned();
            let (status, answer) = {
                let mut server = shared.lock().expect("the fake");
                let answer = server.answer(&parts.method, &path, &body);
                server.seen.push(Seen {
                    method: parts.method,
                    path,
                    body,
                });
                answer
            };
            let response = Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&answer).expect("JSON")))
                .expect("a response");
            send.send_response(response);
        }
    });
    (Client::new(service, "agents"), Fake { server })
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> String {
    fs::read_to_string(root().join("examples/agents/operator.yaml")).expect("the example spec")
}

fn refused_example() -> String {
    example().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    )
}

fn resource_text(uid: &str, generation: i64, spec_agent: &str, rest: &str) -> String {
    let mut text = format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata:\n  name: {NAME}\n  \
         namespace: agents\n  uid: {uid}\n  generation: {generation}\nspec:\n  agent:\n"
    );
    for line in spec_agent.lines() {
        if !line.is_empty() {
            text.push_str("    ");
            text.push_str(line);
        }
        text.push('\n');
    }
    text.push_str(rest);
    text
}

fn agent(uid: &str, generation: i64, spec_agent: &str, rest: &str) -> LoomAgent {
    serde_yaml_ng::from_str(&resource_text(uid, generation, spec_agent, rest)).expect("a LoomAgent")
}

fn context(client: Client) -> Arc<Context> {
    let config = Config::parse(CONFIG).expect("the test configuration");
    Arc::new(Context::new(client, config).expect("the context"))
}

fn applies(seen: &[Seen]) -> Vec<&str> {
    seen.iter()
        .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        .map(|seen| seen.path.as_str())
        .collect()
}

fn published(seen: &[Seen]) -> Option<Value> {
    seen.iter()
        .rev()
        .find(|seen| seen.method == Method::PATCH && seen.path == STATUS)
        .map(|patch| patch.body["status"].clone())
}

fn assert_refused_without_children(seen: &[Seen], value: &str) {
    assert!(
        applies(seen).is_empty(),
        "no child may be applied, but these were: {:?}",
        applies(seen)
    );
    let status = published(seen).expect("a status patch");
    assert_eq!(status["state"], "Refused", "{status}");
    assert_eq!(status["check"]["exitCode"], 3, "{status}");
    assert!(!status.to_string().contains(value), "{status}");
}

/// Design § 7: secrets are "never in the resource, the ConfigMap or the image; only `env:`/`file:`
/// references", enforced by the controller's admission. An `llm.routes/1` `auth.secret` that is
/// a value and not a reference is the literal secret llm's own parser names ("a literal secret",
/// `SECRET_FORMS`); the controller admits it and writes it into the ConfigMap.
#[tokio::test]
async fn adv_a_routes_secret_that_is_a_value_not_a_reference_is_refused_before_any_child() {
    let (client, fake) = fake();
    let ctx = context(client);
    let routes = "  routes:
    format: llm.routes/1
    endpoints:
      lab: {base_url: 'http://lab.example/v1', protocol: messages, auth: {kind: bearer, secret: not-a-reference-000-fake}}
    routes: {small: lab/small}
";
    reconcile(
        Arc::new(agent(UID_A, 1, &example(), routes)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    assert_refused_without_children(&fake.take(), "not-a-reference-000-fake");
}

/// `src/admission.rs` step 2: `spec.routes` is walked for a credential. The walk is
/// `check_text` over the routes text; a routes text is never a `loom.agent/1` document, so
/// `check_text` walks it only within `credentials::bound`, which answers `None` (no walk) for a
/// text holding more than `MAX_FLOW_OPENERS` (4096) `[` or `{` bytes, anywhere, quoted or not.
#[tokio::test]
async fn adv_a_routes_token_beside_4097_braces_in_a_string_is_refused_before_any_child() {
    let (client, fake) = fake();
    let ctx = context(client);
    let braces = "{".repeat(4097);
    let routes = format!(
        "  routes:
    format: llm.routes/1
    note: '{braces}'
    endpoints:
      lab: {{base_url: 'http://lab.example/v1', protocol: messages, auth: {{kind: bearer, secret: xoxb-000-fake}}}}
    routes: {{small: lab/small}}
"
    );
    reconcile(
        Arc::new(agent(UID_A, 1, &example(), &routes)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    assert_refused_without_children(&fake.take(), "xoxb-000-fake");
}

/// `src/admission.rs` step 3: `spec.connectorsConfig` walked for a credential, "each word of the
/// TOML as one value". A TOML basic string decodes `\uXXXX` escapes, so the connectors process
/// reads `xoxb-000-fake` from `"\u0078oxb-000-fake"`, while the word the walk sees starts with
/// `\u0078`.
#[tokio::test]
async fn adv_a_token_spelt_with_a_toml_unicode_escape_is_refused_before_any_child() {
    let (client, fake) = fake();
    let ctx = context(client);
    let rest = format!(
        "{DEFAULT_ROUTES}  connectorsConfig: |\n    [connections.ops]\n    adapter = \"chat\"\n    \
         token = \"\\u0078oxb-000-fake\"\n"
    );
    reconcile(
        Arc::new(agent(UID_A, 1, &example(), &rest)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    assert_refused_without_children(&fake.take(), "oxb-000-fake");
}

/// A refused resource is deleted and created again under the same name, fixed, at generation 1
/// (a new uid). kube's `Controller` triggers on applied objects of the main resource only, and a
/// refused resource owns no Deployment whose deletion would trigger a reconcile, so the record
/// still holds the old resource when the new one is reconciled: `ApplyLoomAgent` answers
/// `unchanged`, the state stays `Refused`, no child is applied and the old refusal is published
/// as the new resource's status.
#[tokio::test]
async fn adv_a_resource_recreated_under_its_name_after_a_refusal_is_checked_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconcile(
        Arc::new(agent(UID_A, 1, &refused_example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    assert_eq!(
        published(&fake.take()).expect("a status")["state"],
        "Refused"
    );

    reconcile(
        Arc::new(agent(UID_B, 1, &example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    let seen = fake.take();
    assert_eq!(
        applies(&seen),
        CHILDREN,
        "the new resource gets its children"
    );
    let status = published(&seen).expect("a status patch");
    assert_eq!(status["state"], "Running", "{status}");
    assert_eq!(status["check"]["exitCode"], 0, "{status}");
}

/// The same, for a resource that reached generation 3 before it was replaced (`kubectl replace
/// --force`, or a delete and create the watch did not see apart): the new resource's generation 1
/// is `stale` against the record's 3, forever, so it is never checked, never gets a child and never
/// gets a status.
#[tokio::test]
async fn adv_a_resource_recreated_at_a_lower_generation_is_not_stale_forever() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconcile(
        Arc::new(agent(UID_A, 3, &example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    fake.take();

    for _ in 0..2 {
        reconcile(
            Arc::new(agent(UID_B, 1, &example(), DEFAULT_ROUTES)),
            Arc::clone(&ctx),
        )
        .await
        .expect("the reconcile succeeds");
    }
    let seen = fake.take();
    assert_eq!(
        applies(&seen)[..],
        CHILDREN[..],
        "the new resource gets its children"
    );
    for apply in seen.iter().filter(|seen| seen.method == Method::PATCH) {
        if !apply.path.ends_with("/status") {
            assert_eq!(apply.body["metadata"]["ownerReferences"][0]["uid"], UID_B);
        }
    }
    let status = published(&seen).expect("a status patch");
    assert_eq!(status["observedGeneration"], 1, "{status}");
}

/// The story: "a Refused resource has no children (existing ones of an earlier generation are
/// deleted)". The deletion runs only in the reconcile that records the refusal; when one delete
/// fails there, the retry finds the record already `Refused` and deletes nothing, so the earlier
/// generation's Deployment keeps running under a `Refused` status.
#[tokio::test]
async fn adv_a_refusal_whose_child_delete_failed_once_deletes_it_on_the_retry() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconcile(
        Arc::new(agent(UID_A, 1, &example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the reconcile succeeds");
    assert!(fake.holds(DEPLOYMENT));
    fake.take();

    fake.fail_delete_once(DEPLOYMENT);
    let first = reconcile(
        Arc::new(agent(UID_A, 2, &refused_example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await;
    assert!(first.is_err(), "the failed delete fails the reconcile");
    fake.take();

    reconcile(
        Arc::new(agent(UID_A, 2, &refused_example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("the retry succeeds");
    let seen = fake.take();
    assert_eq!(published(&seen).expect("a status")["state"], "Refused");
    assert!(
        !fake.holds(DEPLOYMENT),
        "the Deployment of generation 1 is gone after the retry; requests: {:?}",
        seen.iter()
            .map(|seen| format!("{} {}", seen.method, seen.path))
            .collect::<Vec<_>>()
    );
}

/// Design § 7: the controller's blast radius is its own children. A `deploy.kubernetes.
/// service_account` naming a ServiceAccount the resource does not own (the controller's own,
/// `<release>-operator`, or the namespace's `default`) is force-applied with `controller: true`
/// and `automountServiceAccountToken: false`, so the resource takes it over and its deletion
/// garbage-collects it. The delete path already refuses to touch a child it does not own
/// (`delete_owned`); the apply path does not ask.
#[tokio::test]
async fn adv_a_service_account_the_resource_does_not_own_is_not_taken_over() {
    let (client, fake) = fake();
    let ctx = context(client);
    let theirs = "/api/v1/namespaces/agents/serviceaccounts/loom-agents-operator";
    fake.put(
        theirs,
        json!({
            "apiVersion": "v1", "kind": "ServiceAccount",
            "metadata": {"name": "loom-agents-operator", "namespace": "agents",
                         "uid": "uid-of-the-charts-account",
                         "labels": {"app.kubernetes.io/managed-by": "Helm"}},
        }),
    );
    let spec = example().replace(
        "service_account: platform-operator",
        "service_account: loom-agents-operator",
    );
    let _ = reconcile(
        Arc::new(agent(UID_A, 1, &spec, DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await;
    let seen = fake.take();
    assert!(
        !applies(&seen).contains(&theirs),
        "the ServiceAccount another owner holds is force-applied: {:?}",
        applies(&seen)
    );
}

/// Green probe, design § 5.1 "the loom.agent/1 document, verbatim": the ConfigMap's `agent.yaml`
/// decodes to the same `AgentSpec` as the example, `classify_threshold: 0.5` included.
#[test]
fn adv_the_configmaps_agent_yaml_decodes_to_the_examples_spec() {
    let config = Config::parse(CONFIG).expect("the configuration");
    let rendered = render_document(
        &resource_text(UID_A, 1, &example(), DEFAULT_ROUTES),
        &config,
    )
    .expect("rendered");
    let config_map: serde_yaml_ng::Value = rendered
        .split("\n---\n")
        .map(|document| serde_yaml_ng::from_str(document).expect("YAML"))
        .find(|document: &serde_yaml_ng::Value| document["kind"].as_str() == Some("ConfigMap"))
        .expect("a ConfigMap");
    let written = config_map["data"]["agent.yaml"]
        .as_str()
        .expect("agent.yaml");
    let ours = loom_sdk::agent::parse(written).expect("agent.yaml decodes");
    let theirs = loom_sdk::agent::parse(&example()).expect("the example decodes");
    assert_eq!(format!("{ours:?}"), format!("{theirs:?}"));
}

/// Green probe, design § 5.1 status: every status the controller publishes in Running, Degraded
/// and Refused holds the CRD's `status` enums and required members.
#[tokio::test]
async fn adv_every_published_status_holds_the_crds_status_enums() {
    let crd: serde_yaml_ng::Value = serde_yaml_ng::from_str(
        &fs::read_to_string(root().join("deploy/crd/loomagents.loom.beyond10x.dev.yaml"))
            .expect("the CRD"),
    )
    .expect("YAML");
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["status"];
    let enumeration = |value: &serde_yaml_ng::Value| -> Vec<String> {
        value["enum"]
            .as_sequence()
            .expect("an enum")
            .iter()
            .map(|item| item.as_str().expect("a string").to_owned())
            .collect()
    };
    let states = enumeration(&schema["properties"]["state"]);
    let item = &schema["properties"]["conditions"]["items"]["properties"];
    let kinds = enumeration(&item["type"]);
    let statuses = enumeration(&item["status"]);

    let (client, fake) = fake();
    let ctx = context(client);
    let mut seen = Vec::new();
    reconcile(
        Arc::new(agent(UID_A, 1, &example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("running");
    seen.extend(fake.take());
    reconcile(
        Arc::new(agent(UID_A, 2, &refused_example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect("refused");
    seen.extend(fake.take());
    let statuses_seen: Vec<Value> = seen
        .iter()
        .filter(|seen| seen.method == Method::PATCH && seen.path == STATUS)
        .map(|seen| seen.body["status"].clone())
        .collect();
    assert_eq!(statuses_seen.len(), 2);
    for status in &statuses_seen {
        assert!(status["observedGeneration"].is_i64(), "{status}");
        assert!(states.contains(&status["state"].as_str().expect("state").to_owned()));
        for condition in status["conditions"].as_array().expect("conditions") {
            assert!(kinds.contains(&condition["type"].as_str().expect("type").to_owned()));
            assert!(statuses.contains(&condition["status"].as_str().expect("status").to_owned()));
            assert!(condition["reason"].is_string() && condition["message"].is_string());
        }
    }
}
