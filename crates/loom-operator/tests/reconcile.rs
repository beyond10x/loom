//! Acceptance for `story:loom-operator`: the reconcile loop against a fake API server, kube's mock
//! client over `tower-test`, in-process. The fake answers server-side applies with the object
//! applied and a uid naming its path (a Deployment with the availability the test sets), a read
//! with the last object applied at that path or 404, a list by label selector with the objects
//! of that collection whose labels match, and a delete with a `Status`. Nothing reaches a
//! cluster.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::reconcile::{Context, reconcile};
use b10x_loom_operator::resource::LoomAgent;
use b10x_loom_resource::ResourceEvent;
use b10x_loom_resource::model::{
    DeploymentAvailability, LoomAgentApplied, LoomAgentDegraded, LoomAgentDeleted,
    LoomAgentObserved, LoomAgentRecovered, LoomAgentRefused, LoomAgentRunning, LoomAgentState,
    ResourceName,
};
use http::{Method, Request, Response, StatusCode};
use kube::Client;
use kube::client::Body;
use kube::runtime::controller::Action;
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
const UID: &str = "00000000-0000-4000-8000-000000000001";

const STATUS: &str =
    "/apis/loom.beyond10x.dev/v1alpha1/namespaces/agents/loomagents/platform-operator/status";

/// Every child the example renders, in the order they are applied.
const CHILDREN: [&str; 5] = [
    "/api/v1/namespaces/agents/serviceaccounts/platform-operator",
    "/api/v1/namespaces/agents/configmaps/platform-operator-spec",
    "/apis/apps/v1/namespaces/agents/deployments/platform-operator",
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator",
    "/api/v1/namespaces/agents/services/platform-operator",
];

/// The collection of each child kind, in the order the controller lists them.
const COLLECTIONS: [&str; 5] = [
    "/api/v1/namespaces/agents/serviceaccounts",
    "/api/v1/namespaces/agents/configmaps",
    "/apis/apps/v1/namespaces/agents/deployments",
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies",
    "/api/v1/namespaces/agents/services",
];

/// One request the fake server answered.
#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
    query: String,
    body: Value,
}

impl Seen {
    /// The label selector of a list, decoded: `None` for any other request.
    fn selector(&self) -> Option<String> {
        self.query
            .split('&')
            .find_map(|pair| pair.strip_prefix("labelSelector="))
            .map(decoded)
    }

    /// A read of one object by name.
    fn read(&self) -> bool {
        self.method == Method::GET && self.selector().is_none()
    }
}

/// `text`, percent-encoded as a query parameter, decoded.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                at += 1;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).expect("UTF-8")
}

/// The fake API server's state: what it saw, the last object applied at each path, and the
/// availability a Deployment it is applied answers with (`None`: no status yet).
#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    available: Option<bool>,
}

impl Server {
    fn answer(
        &mut self,
        method: &Method,
        path: &str,
        query: &str,
        body: &Value,
    ) -> (StatusCode, Value) {
        let selector = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("labelSelector="))
            .map(decoded);
        if let (&Method::GET, Some(selector)) = (method, selector) {
            let wanted: Vec<(&str, &str)> = selector
                .split(',')
                .filter_map(|pair| pair.split_once('='))
                .collect();
            let collection = format!("{path}/");
            let items: Vec<Value> = self
                .objects
                .iter()
                .filter(|(at, _)| {
                    at.strip_prefix(&collection)
                        .is_some_and(|name| !name.contains('/'))
                })
                .filter(|(_, object)| {
                    wanted
                        .iter()
                        .all(|(key, value)| object["metadata"]["labels"][*key] == *value)
                })
                .map(|(_, object)| object.clone())
                .collect();
            return (
                StatusCode::OK,
                json!({"apiVersion": "v1", "kind": "List", "metadata": {"resourceVersion": "1"},
                       "items": items}),
            );
        }
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
                if path.contains("/deployments/")
                    && let Some(available) = self.available
                {
                    object["status"] = json!({"conditions": [{
                        "type": "Available",
                        "status": if available { "True" } else { "False" },
                        "reason": "Fake",
                    }]});
                }
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

/// A handle on the fake server a test reads and steers.
struct Fake {
    server: Arc<Mutex<Server>>,
}

impl Fake {
    fn set_available(&self, available: Option<bool>) {
        self.server.lock().expect("the fake").available = available;
    }

    /// Every request seen since the last call, emptied.
    fn take(&self) -> Vec<Seen> {
        std::mem::take(&mut self.server.lock().expect("the fake").seen)
    }

    /// Stores `object` at `path`, as if someone else had created it.
    fn put(&self, path: &str, object: Value) {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .insert(path.to_owned(), object);
    }

    fn holds(&self, path: &str) -> bool {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .contains_key(path)
    }

    /// Removes the object at `path`, as if someone else had deleted it.
    fn remove(&self, path: &str) {
        self.server.lock().expect("the fake").objects.remove(path);
    }

    /// Every object the fake holds, by path.
    fn objects(&self) -> BTreeMap<String, Value> {
        self.server.lock().expect("the fake").objects.clone()
    }
}

/// An object at `path` named `name` whose controller is `controller` (kind, name, uid), or none.
fn object(name: &str, path: &str, controller: Option<(&str, &str, &str)>) -> Value {
    let mut metadata =
        json!({"name": name, "namespace": "agents", "uid": format!("uid-of-{path}")});
    if let Some((kind, owner, uid)) = controller {
        metadata["ownerReferences"] = json!([{
            "apiVersion": if kind == "LoomAgent" { "loom.beyond10x.dev/v1alpha1" } else { "apps/v1" },
            "kind": kind, "name": owner, "uid": uid, "controller": true,
        }]);
    }
    json!({"metadata": metadata})
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
            let query = parts.uri.query().unwrap_or_default().to_owned();
            let (status, answer) = {
                let mut server = shared.lock().expect("the fake");
                let answer = server.answer(&parts.method, &path, &query, &body);
                server.seen.push(Seen {
                    method: parts.method,
                    path,
                    query,
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

/// The example with its ServiceAccount named `account`.
fn named(account: &str) -> String {
    let spec = example().replace(
        "service_account: platform-operator",
        &format!("service_account: {account}"),
    );
    assert!(spec.contains(&format!("service_account: {account}")));
    spec
}

/// `spec` with a password in a binding's URL: refused `SecretLiteral`.
fn with_secret_literal(spec: &str) -> String {
    let refused = spec.replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    assert_ne!(refused, spec);
    refused
}

/// The paths of the requests in `seen` with `method`, in order.
fn paths<'a>(seen: &'a [Seen], method: &Method) -> Vec<&'a str> {
    seen.iter()
        .filter(|seen| seen.method == *method)
        .map(|seen| seen.path.as_str())
        .collect()
}

/// `spec_agent` wrapped as the `LoomAgent` `platform-operator` at `generation`, with `rest`
/// after `spec.agent`.
fn agent(generation: i64, spec_agent: &str, rest: &str) -> LoomAgent {
    let mut text = format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata:\n  name: {NAME}\n  \
         namespace: agents\n  uid: {UID}\n  generation: {generation}\nspec:\n  agent:\n"
    );
    for line in spec_agent.lines() {
        text.push_str("    ");
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(rest);
    serde_yaml_ng::from_str(&text).expect("a LoomAgent")
}

fn context(client: Client) -> Arc<Context> {
    let config = Config::parse(CONFIG).expect("the test configuration");
    Arc::new(Context::new(client, config).expect("the context"))
}

async fn reconciled(resource: LoomAgent, ctx: &Arc<Context>) -> Action {
    reconcile(Arc::new(resource), Arc::clone(ctx))
        .await
        .expect("the reconcile succeeds")
}

fn applies(seen: &[Seen]) -> Vec<&str> {
    seen.iter()
        .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        .map(|seen| seen.path.as_str())
        .collect()
}

/// The status the last status patch published.
fn published(seen: &[Seen]) -> Value {
    let patch = seen
        .iter()
        .rev()
        .find(|seen| seen.method == Method::PATCH && seen.path == STATUS)
        .expect("a status patch");
    assert!(patch.query.contains("fieldManager=b10x-loom-operator"));
    assert!(patch.query.contains("force=true"));
    assert_eq!(patch.body["kind"], "LoomAgent");
    patch.body["status"].clone()
}

fn conditions(status: &Value) -> Vec<(String, String, String)> {
    status["conditions"]
        .as_array()
        .expect("conditions")
        .iter()
        .map(|condition| {
            (
                condition["type"].as_str().expect("type").to_owned(),
                condition["status"].as_str().expect("status").to_owned(),
                condition["reason"].as_str().expect("reason").to_owned(),
            )
        })
        .collect()
}

fn triple(kind: &str, status: &str, reason: &str) -> (String, String, String) {
    (kind.to_owned(), status.to_owned(), reason.to_owned())
}

fn name() -> ResourceName {
    ResourceName(NAME.to_owned())
}

#[tokio::test]
async fn a_created_resource_is_checked_its_children_applied_and_its_status_running() {
    let (client, fake) = fake();
    let ctx = context(client);
    let action = reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));

    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN);
    for apply in seen.iter().filter(|seen| seen.method == Method::PATCH) {
        assert!(
            apply.query.contains("fieldManager=b10x-loom-operator"),
            "{apply:?}"
        );
        assert!(apply.query.contains("force=true"), "{apply:?}");
        if !apply.path.ends_with("/status") {
            assert_eq!(apply.body["metadata"]["ownerReferences"][0]["uid"], UID);
        }
    }
    let reads: Vec<&str> = seen
        .iter()
        .filter(|seen| seen.read())
        .map(|seen| seen.path.as_str())
        .collect();
    assert_eq!(
        reads, CHILDREN,
        "each child is read once, to take over none"
    );
    assert!(
        seen[..CHILDREN.len()].iter().all(Seen::read),
        "every read comes before the first apply: {seen:?}"
    );
    let first_apply = CHILDREN.len();
    let last_apply = first_apply + CHILDREN.len();
    assert_eq!(applies(&seen[first_apply..last_apply]), CHILDREN);
    let lists: Vec<(&str, String)> = seen
        .iter()
        .filter_map(|seen| Some((seen.path.as_str(), seen.selector()?)))
        .collect();
    assert_eq!(
        lists,
        COLLECTIONS.map(|path| (
            path,
            "app.kubernetes.io/instance=platform-operator".to_owned()
        )),
        "each child kind is listed by the instance label, to delete what this generation does \
         not render"
    );
    assert!(
        seen[last_apply..last_apply + COLLECTIONS.len()]
            .iter()
            .all(|seen| seen.selector().is_some()),
        "the lists follow the last apply: {seen:?}"
    );
    assert!(
        seen.iter()
            .all(|seen| seen.method == Method::PATCH || seen.method == Method::GET),
        "nothing is deleted: {seen:?}"
    );

    let status = published(&seen);
    assert_eq!(status["observedGeneration"], 1);
    assert_eq!(status["state"], "Running");
    assert_eq!(status["check"]["exitCode"], 0);
    assert_eq!(
        status["check"]["message"],
        "checked platform-operator: 3 states, 3 bindings, protocol operator-reply@1"
    );
    assert!(status.get("deployment").is_none());
    assert_eq!(
        conditions(&status),
        [
            triple("Checked", "True", "SpecServeable"),
            triple("Available", "Unknown", "DeploymentNotObserved"),
        ]
    );
    assert_eq!(
        ctx.events(),
        [
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 1
            }),
        ]
    );
}

#[tokio::test]
async fn the_deployment_moves_a_running_resource_to_degraded_and_back() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();

    fake.set_available(Some(false));
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN, "the children are applied again");
    let status = published(&seen);
    assert_eq!(status["state"], "Degraded");
    assert_eq!(status["deployment"], json!({"availability": "Unavailable"}));
    assert_eq!(
        conditions(&status),
        [
            triple("Checked", "True", "SpecServeable"),
            triple("Available", "False", "DeploymentUnavailable"),
        ]
    );

    fake.set_available(Some(true));
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take());
    assert_eq!(status["state"], "Running");
    assert_eq!(status["deployment"], json!({"availability": "Available"}));
    assert_eq!(
        conditions(&status),
        [
            triple("Checked", "True", "SpecServeable"),
            triple("Available", "True", "DeploymentAvailable"),
        ]
    );

    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    assert_eq!(
        ctx.events()[2..],
        [
            ResourceEvent::Degraded(LoomAgentDegraded { name: name() }),
            ResourceEvent::Recovered(LoomAgentRecovered { name: name() }),
            ResourceEvent::Observed(LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Available
            }),
        ]
    );
}

#[tokio::test]
async fn a_secret_literal_in_the_agent_is_refused_before_any_child_is_applied() {
    let (client, fake) = fake();
    let ctx = context(client);
    let spec = example().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    reconciled(agent(1, &spec, DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    let status = published(&seen);
    assert_eq!(status["state"], "Refused");
    assert_eq!(status["check"]["exitCode"], 3);
    let message = status["check"]["message"].as_str().expect("a message");
    assert!(
        message.starts_with("refused: SecretLiteral at spec.agent.bindings[1].base_url: "),
        "{message}"
    );
    assert!(!status.to_string().contains("not-a-real-password"));
    assert_eq!(
        conditions(&status),
        [
            triple("Checked", "False", "SpecRefused"),
            triple("Available", "False", "SpecRefused"),
        ]
    );
    assert_eq!(
        ctx.rows()[0].state,
        LoomAgentState::Refused,
        "the record holds it Refused"
    );
}

#[tokio::test]
async fn a_secret_literal_in_the_routes_is_refused_before_any_child_is_applied() {
    let (client, fake) = fake();
    let ctx = context(client);
    let routes = "  routes:
    format: llm.routes/1
    endpoints:
      lab: {base_url: 'http://lab.example/v1', protocol: messages, auth: {kind: bearer, secret: xoxb-000-fake}}
    routes: {small: lab/small}
";
    reconciled(agent(1, &example(), routes), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    let status = published(&seen);
    assert_eq!(status["state"], "Refused");
    assert_eq!(status["check"]["exitCode"], 3);
    let message = status["check"]["message"].as_str().expect("a message");
    assert!(
        message.starts_with("refused: SecretLiteral at spec.routes.endpoints.lab.auth.secret: "),
        "{message}"
    );
    assert!(!status.to_string().contains("xoxb-000-fake"));
}

#[tokio::test]
async fn a_new_generation_returns_to_pending_and_is_checked_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    fake.set_available(Some(true));
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take());
    assert_eq!(status["deployment"], json!({"availability": "Available"}));

    fake.set_available(None);
    reconciled(agent(2, &example(), DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN);
    let status = published(&seen);
    assert_eq!(status["observedGeneration"], 2);
    assert_eq!(status["state"], "Running");
    assert!(
        status.get("deployment").is_none(),
        "respecify clears what was read of the Deployment"
    );

    let refused = example().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    reconciled(agent(3, &refused, DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    let deleted: Vec<&str> = seen
        .iter()
        .filter(|seen| seen.method == Method::DELETE)
        .map(|seen| seen.path.as_str())
        .collect();
    assert_eq!(
        deleted, CHILDREN,
        "the children of generation 2 are deleted"
    );
    for delete in seen.iter().filter(|seen| seen.method == Method::DELETE) {
        assert_eq!(
            delete.body["preconditions"]["uid"],
            format!("uid-of-{}", delete.path),
            "a delete names the child it read"
        );
    }
    let status = published(&seen);
    assert_eq!(status["observedGeneration"], 3);
    assert_eq!(status["state"], "Refused");

    assert_eq!(
        ctx.events(),
        [
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Observed(LoomAgentObserved {
                name: name(),
                availability: DeploymentAvailability::Available
            }),
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 2
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 2
            }),
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 3
            }),
            ResourceEvent::Refused(LoomAgentRefused {
                name: name(),
                generation: 3,
                message: status["check"]["message"]
                    .as_str()
                    .expect("a message")
                    .to_owned()
            }),
        ]
    );
}

#[tokio::test]
async fn a_refused_generation_deletes_the_service_account_its_spec_names() {
    let (client, fake) = fake();
    let ctx = context(client);
    let named = example().replace(
        "service_account: platform-operator",
        "service_account: operator-account",
    );
    let account = "/api/v1/namespaces/agents/serviceaccounts/operator-account";
    reconciled(agent(1, &named, DEFAULT_ROUTES), &ctx).await;
    assert!(applies(&fake.take()).contains(&account));

    let refused = named.replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    reconciled(agent(2, &refused, DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    assert!(
        seen.iter()
            .any(|seen| seen.method == Method::DELETE && seen.path == account),
        "the ServiceAccount the spec names is deleted: {seen:?}"
    );
    assert_eq!(published(&seen)["state"], "Refused");
}

#[tokio::test]
async fn an_unchanged_status_is_not_patched_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take());

    let mut current = agent(1, &example(), DEFAULT_ROUTES);
    current.status = Some(serde_json::from_value(status).expect("the published status"));
    reconciled(current, &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN);
    assert!(
        seen.iter().all(|seen| seen.path != STATUS),
        "no status patch: {seen:?}"
    );
}

#[tokio::test]
async fn a_deleted_resource_is_forgotten_and_nothing_is_called() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();

    let mut deleting = agent(1, &example(), DEFAULT_ROUTES);
    deleting.metadata.deletion_timestamp =
        Some(serde_json::from_value(json!("2026-10-25T00:00:00Z")).expect("a timestamp"));
    reconciled(deleting, &ctx).await;
    assert!(
        fake.take().is_empty(),
        "the children go by garbage collection"
    );
    assert!(ctx.rows().is_empty());
    assert_eq!(
        ctx.events().last(),
        Some(&ResourceEvent::Deleted(LoomAgentDeleted { name: name() }))
    );

    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();
    ctx.forget(NAME);
    assert!(
        ctx.rows().is_empty(),
        "gone from the store, gone from the record"
    );
    assert!(fake.take().is_empty());
}

#[tokio::test]
async fn a_child_someone_else_controls_refuses_the_resource_naming_it_and_stays() {
    let config_map = CHILDREN[1];
    for (controller, account, member, kind_and_name) in [
        (
            Some(("Deployment", "other", "uid-of-someone-else")),
            None,
            "metadata.name",
            "ConfigMap platform-operator-spec",
        ),
        (
            None,
            Some("loom-agents-operator"),
            "spec.agent.deploy.kubernetes.service_account",
            "ServiceAccount loom-agents-operator",
        ),
        (
            Some(("LoomAgent", "another-agent", "uid-of-another-agent")),
            Some("another-agent"),
            "spec.agent.deploy.kubernetes.service_account",
            "ServiceAccount another-agent",
        ),
    ] {
        let (client, fake) = fake();
        let ctx = context(client);
        let mut spec = example();
        let path = match account {
            Some(account) => {
                spec = spec.replace(
                    "service_account: platform-operator",
                    &format!("service_account: {account}"),
                );
                format!("/api/v1/namespaces/agents/serviceaccounts/{account}")
            }
            None => config_map.to_owned(),
        };
        let name = path.rsplit('/').next().expect("a name").to_owned();
        fake.put(&path, object(&name, &path, controller));

        reconciled(agent(1, &spec, DEFAULT_ROUTES), &ctx).await;
        let seen = fake.take();
        assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
        assert!(
            seen.iter().all(|seen| seen.method != Method::DELETE),
            "nothing is deleted: {seen:?}"
        );
        assert!(fake.holds(&path));
        let status = published(&seen);
        assert_eq!(status["state"], "Refused");
        assert_eq!(status["check"]["exitCode"], 3);
        assert_eq!(
            status["check"]["message"],
            format!(
                "refused: ChildNotOwned at {member}: the {kind_and_name} exists and this resource \
                 does not control it"
            )
        );
    }
}

#[tokio::test]
async fn a_child_someone_else_creates_under_a_running_resource_is_not_taken_over() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();

    let service = CHILDREN[4];
    fake.put(service, object(NAME, service, None));
    let error = reconcile(
        Arc::new(agent(1, &example(), DEFAULT_ROUTES)),
        Arc::clone(&ctx),
    )
    .await
    .expect_err("the reconcile fails");
    assert_eq!(
        error.to_string(),
        "the Service platform-operator exists and this resource does not control it"
    );
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    assert_eq!(
        fake.server.lock().expect("the fake").objects[service],
        object(NAME, service, None),
        "the Service is left as its owner wrote it"
    );
}

#[tokio::test]
async fn children_a_predecessor_left_are_deleted_and_applied_on_the_next_reconcile() {
    let (client, fake) = fake();
    let ctx = context(client);
    for path in CHILDREN {
        let name = path.rsplit('/').next().expect("a name");
        fake.put(
            path,
            object(
                name,
                path,
                Some(("LoomAgent", NAME, "uid-of-the-predecessor")),
            ),
        );
    }

    let action = reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    let deletes: Vec<&Seen> = seen
        .iter()
        .filter(|seen| seen.method == Method::DELETE)
        .collect();
    assert_eq!(
        deletes
            .iter()
            .map(|seen| seen.path.as_str())
            .collect::<Vec<_>>(),
        CHILDREN
    );
    for delete in deletes {
        assert_eq!(
            delete.body["preconditions"]["uid"],
            format!("uid-of-{}", delete.path)
        );
    }
    assert_eq!(published(&seen)["state"], "Running");

    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN);
    for apply in seen
        .iter()
        .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
    {
        assert_eq!(apply.body["metadata"]["ownerReferences"][0]["uid"], UID);
    }
}

#[tokio::test]
async fn a_service_account_a_new_generation_no_longer_names_is_deleted_after_the_apply() {
    let (client, fake) = fake();
    let ctx = context(client);
    let old = "/api/v1/namespaces/agents/serviceaccounts/operator-account";
    let new = "/api/v1/namespaces/agents/serviceaccounts/another-account";
    reconciled(agent(1, &named("operator-account"), DEFAULT_ROUTES), &ctx).await;
    assert!(fake.holds(old));
    fake.take();

    reconciled(agent(2, &named("another-account"), DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).contains(&new), "{seen:?}");
    assert_eq!(paths(&seen, &Method::DELETE), [old]);
    let delete = seen
        .iter()
        .position(|seen| seen.method == Method::DELETE)
        .expect("a delete");
    assert_eq!(
        seen[delete].body["preconditions"]["uid"],
        format!("uid-of-{old}")
    );
    let last_apply = seen
        .iter()
        .rposition(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        .expect("an apply");
    assert!(
        last_apply < delete,
        "the old account goes once the new children are applied: {seen:?}"
    );
    assert!(!fake.holds(old));
    assert!(fake.holds(new));
    assert_eq!(published(&seen)["state"], "Running");
}

/// After a restart the controller has applied nothing: the children an earlier generation had
/// are found by the instance label, and only those this resource controls are deleted.
#[tokio::test]
async fn after_a_restart_a_refused_generation_deletes_the_children_it_finds_by_label() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(1, &named("operator-account"), DEFAULT_ROUTES), &ctx).await;
    fake.take();

    let (fresh_client, fresh) = self::fake();
    for (path, object) in fake.objects() {
        fresh.put(&path, object);
    }
    let theirs = "/api/v1/namespaces/agents/configmaps/platform-operator-notes";
    let mut labelled = object("platform-operator-notes", theirs, None);
    labelled["metadata"]["labels"] = json!({"app.kubernetes.io/instance": NAME});
    fresh.put(theirs, labelled);
    let restarted = context(fresh_client);
    reconciled(
        agent(
            2,
            &with_secret_literal(&named("another-account")),
            DEFAULT_ROUTES,
        ),
        &restarted,
    )
    .await;
    let seen = fresh.take();
    assert_eq!(published(&seen)["state"], "Refused");
    assert!(
        seen.iter().all(|seen| !seen.read()),
        "nothing is read by name: {seen:?}"
    );
    assert_eq!(
        paths(&seen, &Method::DELETE),
        [
            "/api/v1/namespaces/agents/serviceaccounts/operator-account",
            CHILDREN[1],
            CHILDREN[2],
            CHILDREN[3],
            CHILDREN[4],
        ]
    );
    assert_eq!(
        fresh.objects().into_keys().collect::<Vec<_>>(),
        [theirs],
        "only the labelled ConfigMap no one controls is left"
    );
}

#[tokio::test]
async fn a_child_not_owned_refusal_is_read_again_and_clears_once_the_child_is_gone() {
    let (client, fake) = fake();
    let ctx = context(client);
    let config_map = CHILDREN[1];
    fake.put(
        config_map,
        object("platform-operator-spec", config_map, None),
    );
    reconciled(agent(1, &example(), DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take());
    assert_eq!(status["state"], "Refused");
    let mut current = agent(1, &example(), DEFAULT_ROUTES);
    current.status = Some(serde_json::from_value(status).expect("the published status"));
    let events = ctx.events().len();

    reconciled(current.clone(), &ctx).await;
    let seen = fake.take();
    assert_eq!(
        seen.iter()
            .filter(|seen| seen.read())
            .map(|seen| seen.path.as_str())
            .collect::<Vec<_>>(),
        CHILDREN,
        "the children are read again"
    );
    assert!(applies(&seen).is_empty(), "no apply call: {seen:?}");
    assert!(
        seen.iter().all(|seen| seen.path != STATUS),
        "the refusal stands: {seen:?}"
    );
    assert_eq!(ctx.events().len(), events, "the record is not touched");

    fake.remove(config_map);
    reconciled(current, &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), CHILDREN);
    let status = published(&seen);
    assert_eq!(status["state"], "Running");
    assert_eq!(status["observedGeneration"], 1);
    assert_eq!(status["check"]["exitCode"], 0);
    assert_eq!(
        ctx.events()[events..],
        [
            ResourceEvent::Deleted(LoomAgentDeleted { name: name() }),
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 1
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 1
            }),
        ]
    );
}

#[tokio::test]
async fn a_refusal_of_the_spec_reads_no_child_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    let refused = with_secret_literal(&example());
    reconciled(agent(1, &refused, DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take());
    let mut current = agent(1, &refused, DEFAULT_ROUTES);
    current.status = Some(serde_json::from_value(status).expect("the published status"));
    let events = ctx.events().len();

    reconciled(current, &ctx).await;
    let seen = fake.take();
    assert!(
        seen.iter()
            .all(|seen| !seen.read() && seen.method == Method::GET),
        "only the lists of what it controls: {seen:?}"
    );
    assert_eq!(ctx.events().len(), events);
}
