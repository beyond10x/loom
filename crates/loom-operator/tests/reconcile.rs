//! Acceptance for `story:loom-operator`: the reconcile loop against a fake API server, kube's mock
//! client over `tower-test`, in-process. The fake answers server-side applies with the object
//! applied and a uid naming its path (a Deployment with the availability the test sets), a read
//! with the last object applied at that path or 404, a list by label selector with the objects
//! of that collection whose labels match (a Job applied gets the selector the API server would give
//! it), a delete with a `Status`, and a read of a path the test forbids with 403. Nothing reaches a
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
    DeploymentAvailability, LoomAgentApplied, LoomAgentCompleted, LoomAgentDegraded,
    LoomAgentDeleted, LoomAgentFailed, LoomAgentObserved, LoomAgentRecovered, LoomAgentRefused,
    LoomAgentRunObserved, LoomAgentRunning, LoomAgentState, ResourceName,
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
const COLLECTIONS: [&str; 6] = [
    "/api/v1/namespaces/agents/serviceaccounts",
    "/api/v1/namespaces/agents/configmaps",
    "/apis/apps/v1/namespaces/agents/deployments",
    "/apis/batch/v1/namespaces/agents/jobs",
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

/// The fake API server's state: what it saw, the last object applied at each path, a Job with
/// the selector the API server gives it (its own uid as `batch.kubernetes.io/controller-uid`), the
/// availability a Deployment it is applied answers with (`None`: no status yet), and the status
/// a Job it is applied answers with (`None`: none yet), and whether a `Foreground` delete leaves
/// the object terminating, as the API server does while its dependents go, instead of removing it,
/// and the path a read or list of is answered 403 (`None`: none).
#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    available: Option<bool>,
    run: Option<Value>,
    lingering: bool,
    forbidden: Option<String>,
}

impl Server {
    fn answer(
        &mut self,
        method: &Method,
        path: &str,
        query: &str,
        body: &Value,
    ) -> (StatusCode, Value) {
        if *method == Method::GET && self.forbidden.as_deref() == Some(path) {
            return (
                StatusCode::FORBIDDEN,
                json!({
                    "kind": "Status", "apiVersion": "v1", "metadata": {},
                    "status": "Failure", "reason": "Forbidden", "code": 403,
                    "message": "forbidden",
                }),
            );
        }
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
                if path.contains("/jobs/") {
                    object["spec"]["selector"] = json!({"matchLabels": {
                        "batch.kubernetes.io/controller-uid": format!("uid-of-{path}"),
                    }});
                }
                if path.contains("/deployments/")
                    && let Some(available) = self.available
                {
                    object["status"] = json!({"conditions": [{
                        "type": "Available",
                        "status": if available { "True" } else { "False" },
                        "reason": "Fake",
                    }]});
                }
                if path.contains("/jobs/")
                    && let Some(run) = &self.run
                {
                    object["status"] = run.clone();
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
            Method::DELETE if self.lingering && body["propagationPolicy"] == "Foreground" => {
                match self.objects.get_mut(path) {
                    Some(object) => {
                        object["metadata"]["deletionTimestamp"] = json!("2026-10-26T00:00:00Z");
                        object["metadata"]["finalizers"] = json!(["foregroundDeletion"]);
                        (StatusCode::OK, object.clone())
                    }
                    None => (
                        StatusCode::NOT_FOUND,
                        json!({
                            "kind": "Status", "apiVersion": "v1", "metadata": {},
                            "status": "Failure", "reason": "NotFound", "code": 404,
                            "message": "not found",
                        }),
                    ),
                }
            }
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

    /// From now on a `Foreground` delete leaves the object terminating until [`Fake::remove`].
    fn set_lingering(&self, lingering: bool) {
        self.server.lock().expect("the fake").lingering = lingering;
    }

    /// The status a Job answers its apply with from now on, and the Job held at `path`, if any,
    /// takes it too, as the Job controller would write it.
    fn set_run(&self, path: &str, run: Option<Value>) {
        let mut server = self.server.lock().expect("the fake");
        if let (Some(job), Some(status)) = (server.objects.get_mut(path), &run) {
            job["status"] = status.clone();
        }
        server.run = run;
    }

    /// From now on a read or list of `path` is answered 403, as for a Role without the read.
    fn set_forbidden(&self, path: &str) {
        self.server.lock().expect("the fake").forbidden = Some(path.to_owned());
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
    assert_eq!(status["variant"], "AgentServe");
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

#[tokio::test]
async fn a_command_resource_is_admitted_running_with_its_variant_and_no_init_container() {
    let (client, fake) = fake();
    let ctx = context(client);
    let spec = example().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let rest = format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{}}\n"
    );
    reconciled(agent(1, &spec, &rest), &ctx).await;

    let seen = fake.take();
    assert_eq!(applies(&seen), COMMAND_CHILDREN);
    let job = seen
        .iter()
        .find(|seen| seen.method == Method::PATCH && seen.path == JOB)
        .expect("the Job is applied");
    let pod = &job.body["spec"]["template"]["spec"];
    assert!(pod.get("initContainers").is_none(), "{pod}");
    assert!(pod["containers"][0].get("command").is_none(), "{pod}");
    assert!(pod["containers"][0].get("args").is_none(), "{pod}");

    let status = published(&seen);
    assert_eq!(status["state"], "Running");
    assert_eq!(status["variant"], "Command");
    assert_eq!(status["check"]["exitCode"], 0);
    assert_eq!(
        status["check"]["message"],
        "admitted platform-operator for its own command: a loom.agent/1 document with no \
         credential, at /config/agent.yaml"
    );
    assert_eq!(
        conditions(&status),
        [
            triple("Checked", "True", "SpecAdmitted"),
            triple("Available", "Unknown", "RunNotFinished"),
            triple("Completed", "Unknown", "RunNotFinished"),
        ]
    );

    let refused = example().replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    reconciled(agent(2, &refused, &rest), &ctx).await;
    let seen = fake.take();
    assert!(applies(&seen).is_empty(), "no child: {seen:?}");
    let status = published(&seen);
    assert_eq!(status["state"], "Refused");
    assert_eq!(status["variant"], "Command");
    assert!(
        status["check"]["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("refused: SecretLiteral at spec.agent.")),
        "{status}"
    );
}

/// The Job of the resource `platform-operator`.
const JOB: &str = "/apis/batch/v1/namespaces/agents/jobs/platform-operator";

/// Every child a `Command` resource over the example renders, in the order they are applied.
const COMMAND_CHILDREN: [&str; 5] = [CHILDREN[0], CHILDREN[1], JOB, CHILDREN[3], CHILDREN[4]];

/// The example as a `Command` resource at `generation`, running `args`.
fn command_agent(generation: i64, args: &str) -> LoomAgent {
    let spec = example().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let rest = format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [{args}]}}\n"
    );
    agent(generation, &spec, &rest)
}

/// A Job's status once its one pod ended with exit code 0.
fn succeeded() -> Value {
    json!({"succeeded": 1, "conditions": [{"type": "Complete", "status": "True"}]})
}

/// A Job's status once its one pod ended otherwise; `backoffLimit: 0` retries none.
fn failed() -> Value {
    json!({"failed": 1, "conditions": [{"type": "Failed", "status": "True"}]})
}

/// `story:command-variant-runs-once`: once the Job of the current generation has ended, no
/// reconcile applies, creates or deletes a child, and a Job someone deletes afterwards is not
/// created again; the status reads Completed or Failed.
#[tokio::test]
async fn a_finished_job_is_not_restarted() {
    for (run, state, reason) in [
        (succeeded(), "Completed", "RunSucceeded"),
        (failed(), "Failed", "RunFailed"),
    ] {
        let (client, fake) = fake();
        let ctx = context(client);
        reconciled(command_agent(1, "run"), &ctx).await;
        let seen = fake.take();
        assert_eq!(applies(&seen), COMMAND_CHILDREN, "{state}");
        let job = seen
            .iter()
            .find(|seen| seen.method == Method::PATCH && seen.path == JOB)
            .expect("the Job is applied");
        assert_eq!(job.body["kind"], "Job");
        assert_eq!(job.body["spec"]["backoffLimit"], 0);
        assert_eq!(
            job.body["spec"]["template"]["spec"]["restartPolicy"],
            "Never"
        );
        let status = published(&seen);
        assert_eq!(status["state"], "Running");
        assert_eq!(
            conditions(&status),
            [
                triple("Checked", "True", "SpecAdmitted"),
                triple("Available", "Unknown", "RunNotFinished"),
                triple("Completed", "Unknown", "RunNotFinished"),
            ]
        );

        fake.set_run(JOB, Some(run));
        reconciled(command_agent(1, "run"), &ctx).await;
        let seen = fake.take();
        assert!(applies(&seen).is_empty(), "{state}: no apply: {seen:?}");
        assert!(
            seen.iter().all(|seen| seen.method != Method::DELETE),
            "{state}: nothing is deleted: {seen:?}"
        );
        let status = published(&seen);
        assert_eq!(status["state"], state);
        assert_eq!(status["observedGeneration"], 1);
        assert_eq!(
            conditions(&status),
            [
                triple("Checked", "True", "SpecAdmitted"),
                triple("Available", "False", reason),
                triple(
                    "Completed",
                    if state == "Completed" {
                        "True"
                    } else {
                        "False"
                    },
                    reason
                ),
            ]
        );

        for _ in 0..2 {
            let action = reconciled(command_agent(1, "run"), &ctx).await;
            assert_eq!(action, Action::requeue(Duration::from_secs(300)));
            let seen = fake.take();
            assert!(
                seen.iter()
                    .all(|seen| seen.path == STATUS && seen.method == Method::PATCH),
                "{state}: a finished run reads, applies and deletes no child: {seen:?}"
            );
            assert_eq!(published(&seen)["state"], state);
            fake.remove(JOB);
        }
        assert!(
            !fake.holds(JOB),
            "{state}: the Job deleted by someone is not created again"
        );

        let events = ctx.events();
        assert_eq!(events.len(), 4, "{state}: {events:?}");
        assert_eq!(
            events[2],
            ResourceEvent::RunObserved(LoomAgentRunObserved {
                name: name(),
                generation: 1
            })
        );
        assert_eq!(
            events[3],
            if state == "Completed" {
                ResourceEvent::Completed(LoomAgentCompleted {
                    name: name(),
                    generation: 1,
                    exit_code: None,
                })
            } else {
                ResourceEvent::Failed(LoomAgentFailed {
                    name: name(),
                    generation: 1,
                    exit_code: None,
                })
            }
        );
    }
}

/// The pods of the namespace `agents`.
const PODS: &str = "/api/v1/namespaces/agents/pods";

/// The selector the fake gives the resource's Job, as the API server would.
const JOB_SELECTOR: &str = "batch.kubernetes.io/controller-uid=uid-of-/apis/batch/v1/namespaces/agents/jobs/platform-operator";

/// The pod `name` of the Job whose uid is `job`, labelled with the resource Job's selector, its
/// program container in `state`.
fn pod(name: &str, job: &str, state: Value) -> (String, Value) {
    let path = format!("{PODS}/{name}");
    let pod = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": name,
            "namespace": "agents",
            "uid": format!("uid-of-{path}"),
            "labels": {"batch.kubernetes.io/controller-uid": format!("uid-of-{JOB}")},
            "ownerReferences": [{
                "apiVersion": "batch/v1", "kind": "Job", "name": NAME, "uid": job,
                "controller": true,
            }],
        },
        "status": {"containerStatuses": [{
            "name": "loom-agent", "image": "registry.example/takt:0.12.0", "imageID": "",
            "ready": false, "restartCount": 0, "state": state,
        }]},
    });
    (path, pod)
}

/// A container state: terminated with `exit_code` at `at`, for `reason`.
fn terminated(exit_code: i64, at: &str, reason: &str) -> Value {
    json!({"terminated": {"exitCode": exit_code, "finishedAt": at, "reason": reason}})
}

/// The lists of pods among `seen`, by their label selector.
fn pod_lists(seen: &[Seen]) -> Vec<String> {
    seen.iter()
        .filter(|seen| seen.method == Method::GET && seen.path == PODS)
        .map(|seen| seen.selector().expect("a list by label selector"))
        .collect()
}

/// Runs the `Command` resource until its Job reports `run` with the pods `pods` present, and
/// answers the status published once the run ended and the events recorded.
async fn ended_with(run: Value, pods: Vec<(String, Value)>) -> (Value, Vec<ResourceEvent>) {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    let seen = fake.take();
    assert!(pod_lists(&seen).is_empty(), "an active run reads no pod");
    assert!(
        published(&seen).get("run").is_none(),
        "no run before it ends"
    );
    for (path, pod) in pods {
        fake.put(&path, pod);
    }
    fake.set_run(JOB, Some(run));
    reconciled(command_agent(1, "run"), &ctx).await;
    let seen = fake.take();
    assert_eq!(
        pod_lists(&seen),
        [JOB_SELECTOR],
        "the Job's pods are read once"
    );
    (published(&seen), ctx.events())
}

/// The message of the condition `kind` in `status`.
fn message<'a>(status: &'a Value, kind: &str) -> &'a str {
    status["conditions"]
        .as_array()
        .expect("conditions")
        .iter()
        .find(|condition| condition["type"] == kind)
        .and_then(|condition| condition["message"].as_str())
        .expect("the condition")
}

/// `story:command-run-exit-status`: a run that succeeded reports exit code 0 and when the
/// program ended, in `status.run` and the condition `Completed`.
#[tokio::test]
async fn a_completed_run_reports_exit_code_0() {
    let own = format!("uid-of-{JOB}");
    let (status, events) = ended_with(
        succeeded(),
        vec![pod(
            "platform-operator-x1",
            &own,
            terminated(0, "2026-10-26T12:00:00Z", "Completed"),
        )],
    )
    .await;
    assert_eq!(status["state"], "Completed");
    assert_eq!(
        status["run"],
        json!({"exitCode": 0, "finishedAt": "2026-10-26T12:00:00Z", "reason": "Completed"})
    );
    assert_eq!(
        conditions(&status)[2],
        triple("Completed", "True", "RunSucceeded")
    );
    assert_eq!(
        message(&status, "Completed"),
        "the program exited with code 0 at 2026-10-26T12:00:00Z (Completed)"
    );
    assert_eq!(
        events.last(),
        Some(&ResourceEvent::Completed(LoomAgentCompleted {
            name: name(),
            generation: 1,
            exit_code: Some(0),
        }))
    );
}

/// `story:command-run-exit-status`: a run that failed reports the program's exit code and when
/// it ended, in `status.run` and the condition `Completed`.
#[tokio::test]
async fn a_failed_run_reports_its_exit_code() {
    let own = format!("uid-of-{JOB}");
    let (status, events) = ended_with(
        failed(),
        vec![pod(
            "platform-operator-x1",
            &own,
            terminated(3, "2026-10-26T12:05:00Z", "Error"),
        )],
    )
    .await;
    assert_eq!(status["state"], "Failed");
    assert_eq!(
        status["run"],
        json!({"exitCode": 3, "finishedAt": "2026-10-26T12:05:00Z", "reason": "Error"})
    );
    assert_eq!(
        conditions(&status)[2],
        triple("Completed", "False", "RunFailed")
    );
    assert_eq!(
        message(&status, "Completed"),
        "the program exited with code 3 at 2026-10-26T12:05:00Z (Error)"
    );
    assert_eq!(
        events.last(),
        Some(&ResourceEvent::Failed(LoomAgentFailed {
            name: name(),
            generation: 1,
            exit_code: Some(3),
        }))
    );
}

/// `story:command-run-exit-status`: a pod the Job's selector matches but another Job controls
/// does not speak for the run, even when it ended later; the Job's own pod does.
#[tokio::test]
async fn a_pod_of_another_job_does_not_speak_for_the_run() {
    let own = format!("uid-of-{JOB}");
    let (status, _) = ended_with(
        failed(),
        vec![
            pod(
                "platform-operator-own",
                &own,
                terminated(2, "2026-10-26T12:00:00Z", "Error"),
            ),
            pod(
                "platform-operator-other",
                "uid-of-another-job",
                terminated(0, "2026-10-26T13:00:00Z", "Completed"),
            ),
        ],
    )
    .await;
    assert_eq!(
        status["run"],
        json!({"exitCode": 2, "finishedAt": "2026-10-26T12:00:00Z", "reason": "Error"})
    );

    let (status, _) = ended_with(
        failed(),
        vec![pod(
            "platform-operator-other",
            "uid-of-another-job",
            terminated(0, "2026-10-26T13:00:00Z", "Completed"),
        )],
    )
    .await;
    assert_eq!(status["state"], "Failed");
    assert_eq!(
        status["run"],
        json!({}),
        "no exit code from another Job's pod"
    );
}

/// `story:command-run-exit-status`: a Job that failed while no program container's end can be
/// read (its pod deleted before the controller read it, or a pod that ended in phase `Failed`
/// before its container ran) still reports failed, with no exit code and the Job's reason in the
/// condition's message.
#[tokio::test]
async fn a_failed_run_without_a_terminated_container_reports_the_jobs_reason() {
    let own = format!("uid-of-{JOB}");
    let (path, mut never_ran) = pod(
        "platform-operator-x1",
        &own,
        json!({"waiting": {"reason": "ContainerCreating"}}),
    );
    never_ran["status"]["phase"] = json!("Failed");
    for pods in [vec![], vec![(path, never_ran)]] {
        let gone = json!({"failed": 1, "conditions": [{
            "type": "Failed", "status": "True", "reason": "BackoffLimitExceeded",
        }]});
        let (status, events) = ended_with(gone, pods).await;
        assert_eq!(status["state"], "Failed");
        assert_eq!(status["run"], json!({"reason": "BackoffLimitExceeded"}));
        assert_eq!(
            conditions(&status)[2],
            triple("Completed", "False", "RunFailed")
        );
        assert_eq!(
            message(&status, "Completed"),
            "the Job's run failed with no exit code read from its pod (BackoffLimitExceeded)"
        );
        assert_eq!(
            events.last(),
            Some(&ResourceEvent::Failed(LoomAgentFailed {
                name: name(),
                generation: 1,
                exit_code: None,
            }))
        );
    }
}

/// `story:command-run-exit-status`: a list of pods the API server refuses (403, a Role without
/// the read) leaves the exit code absent and the `Completed` condition says the pod could not be
/// read; the run still ends Failed.
#[tokio::test]
async fn a_refused_pod_list_leaves_the_exit_code_absent() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.take();
    fake.set_forbidden(PODS);
    let (path, own) = pod(
        "platform-operator-x1",
        &format!("uid-of-{JOB}"),
        terminated(1, "2026-10-26T12:00:00Z", "Error"),
    );
    fake.put(&path, own);
    fake.set_run(JOB, Some(failed()));
    let action = reconciled(command_agent(1, "run"), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
    let status = published(&fake.take());
    assert_eq!(status["state"], "Failed");
    assert_eq!(status["run"], json!({}));
    assert_eq!(
        message(&status, "Completed"),
        "the Job's run failed with no exit code read from its pod; its pod could not be read (the \
         list was answered 403)"
    );
}

/// `story:command-variant-runs-once`: a Job's pod template cannot change, so a new generation
/// deletes the Job of the earlier one (its pod first, propagation `Foreground`) and, once the
/// delete answers it gone, creates its own.
#[tokio::test]
async fn a_new_generation_replaces_the_job() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.set_run(JOB, Some(succeeded()));
    reconciled(command_agent(1, "run"), &ctx).await;
    assert_eq!(published(&fake.take())["state"], "Completed");
    fake.set_run(JOB, None);

    let action = reconciled(command_agent(2, "run, --again"), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
    let seen = fake.take();
    assert_eq!(paths(&seen, &Method::DELETE), [JOB]);
    let delete = seen
        .iter()
        .position(|seen| seen.method == Method::DELETE)
        .expect("a delete");
    assert_eq!(seen[delete].body["propagationPolicy"], "Foreground");
    assert_eq!(
        seen[delete].body["preconditions"]["uid"],
        format!("uid-of-{JOB}")
    );
    let created = seen
        .iter()
        .position(|seen| seen.method == Method::PATCH && seen.path == JOB)
        .expect("the new Job is created");
    assert!(
        delete < created,
        "the old Job is gone before the new one is created: {seen:?}"
    );
    assert_eq!(applies(&seen), COMMAND_CHILDREN);
    let job = &fake.objects()[JOB];
    assert_eq!(
        job["spec"]["template"]["metadata"]["annotations"]["loom.beyond10x.dev/generation"],
        "2"
    );
    assert_eq!(
        job["spec"]["template"]["spec"]["containers"][0]["args"],
        json!(["run", "--again"])
    );
    assert_eq!(published(&seen)["state"], "Running");
    assert_eq!(
        ctx.events()[ctx.events().len() - 3..],
        [
            ResourceEvent::Applied(LoomAgentApplied {
                name: name(),
                generation: 2
            }),
            ResourceEvent::Running(LoomAgentRunning {
                name: name(),
                generation: 2
            }),
            ResourceEvent::RunObserved(LoomAgentRunObserved {
                name: name(),
                generation: 2
            }),
        ]
    );
}

/// Applies nothing while the workload at `path` terminates, asks again after 30 s, and creates the
/// Job once it is gone: the old pod and the new run never share the workspace.
async fn the_job_waits_for(path: &str, ctx: &Arc<Context>, fake: &Fake, generation: i64) {
    for round in 0..2 {
        let action = reconciled(command_agent(generation, "run"), ctx).await;
        assert_eq!(action, Action::requeue(Duration::from_secs(30)), "{round}");
        let seen = fake.take();
        assert!(
            applies(&seen).is_empty(),
            "{round}: nothing applied: {seen:?}"
        );
        let deletes: Vec<&Seen> = seen
            .iter()
            .filter(|seen| seen.method == Method::DELETE)
            .collect();
        assert_eq!(deletes.len(), 1, "{round}: {seen:?}");
        assert_eq!(deletes[0].path, path);
        assert_eq!(deletes[0].body["propagationPolicy"], "Foreground");
        assert!(fake.holds(path), "{round}: still terminating");
        let status = published(&seen);
        assert_eq!(status["state"], "Running", "{round}");
        assert_eq!(status["observedGeneration"], generation, "{round}");
    }

    fake.remove(path);
    let action = reconciled(command_agent(generation, "run"), ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
    let seen = fake.take();
    assert_eq!(applies(&seen), COMMAND_CHILDREN);
    assert!(paths(&seen, &Method::DELETE).is_empty(), "{seen:?}");
    assert_eq!(
        fake.objects()[JOB]["spec"]["template"]["metadata"]["annotations"]["loom.beyond10x.dev/generation"],
        generation.to_string()
    );
}

/// A new generation's Job is not created while the earlier generation's Job, deleted in the
/// foreground, still terminates.
#[tokio::test]
async fn a_new_generation_waits_until_the_old_job_is_gone() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.take();
    fake.set_lingering(true);
    the_job_waits_for(JOB, &ctx, &fake, 2).await;
}

/// A `Command` resource's Job is not created while the Deployment an earlier release made for it,
/// deleted in the foreground, still terminates.
#[tokio::test]
async fn a_job_waits_until_an_earlier_command_deployment_is_gone() {
    let (client, fake) = fake();
    let mut earlier = object(NAME, CHILDREN[2], Some(("LoomAgent", NAME, UID)));
    earlier["metadata"]["labels"] = json!({"app.kubernetes.io/instance": NAME});
    earlier["kind"] = json!("Deployment");
    earlier["apiVersion"] = json!("apps/v1");
    fake.put(CHILDREN[2], earlier);
    fake.set_lingering(true);
    let ctx = context(client);
    the_job_waits_for(CHILDREN[2], &ctx, &fake, 1).await;
}

// Adversary cases for story:command-variant-runs-once (wave 2026-10-26-cmdv, unit u1).

/// The test configuration with the forge Secret allowed as files and the default image at `tag`,
/// as the chart writes it: `agentImage.tag` is the release's version, so a release changes it.
fn adv_config(tag: &str) -> Config {
    let text = CONFIG
        .replace("tag: '0.40.0'", &format!("tag: '{tag}'"))
        .replace(
            "allowedSecretNames: [platform-operator-secrets]",
            "allowedSecretNames: [platform-operator-secrets, takt-forge]",
        );
    Config::parse(&text).expect("the adversary configuration")
}

/// A `Command` resource at `generation` with its forge key as files, which an init container
/// from the configured default image stages (design § 9a).
fn adv_command_with_files(generation: i64) -> LoomAgent {
    let spec = example().replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let rest = format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [run]}}\n  \
         secretFiles: {{name: takt-forge, mountPath: /var/run/secrets/takt}}\n"
    );
    agent(generation, &spec, &rest)
}

/// A Job's pod template cannot change once created: the API server answers an apply that changes
/// it `422 spec.template: field is immutable`. A release moves the chart's `agentImage.tag`, the
/// image of the `stage-secret-files` init container in a `Command` pod, so a controller upgraded
/// while a run is active renders a different template for the same generation. The Job that is
/// running must not be applied again with it, or every reconcile of that run fails until it ends,
/// and the NetworkPolicy behind the Job is not applied meanwhile.
#[tokio::test]
async fn adv_an_active_job_is_not_applied_again_with_a_changed_pod_template() {
    let (client, fake) = fake();
    let before = Arc::new(Context::new(client, adv_config("0.40.0")).expect("the context"));
    reconciled(adv_command_with_files(1), &before).await;
    let seen = fake.take();
    assert!(
        applies(&seen).contains(&JOB),
        "the Job is created: {seen:?}"
    );
    let created = fake.objects()[JOB].clone();
    let old_init = created["spec"]["template"]["spec"]["initContainers"][0]["image"].clone();
    assert_eq!(old_init, "registry.example/loom-agent:0.40.0");

    let (upgraded_client, upgraded) = self::fake();
    for (path, object) in fake.objects() {
        upgraded.put(&path, object);
    }
    let after = Arc::new(Context::new(upgraded_client, adv_config("0.41.0")).expect("the context"));
    reconciled(adv_command_with_files(1), &after).await;
    let seen = upgraded.take();
    let reapplied: Vec<&Seen> = seen
        .iter()
        .filter(|seen| seen.method == Method::PATCH && seen.path == JOB)
        .collect();
    let changed: Vec<Value> = reapplied
        .iter()
        .map(|seen| seen.body["spec"]["template"]["spec"]["initContainers"][0]["image"].clone())
        .filter(|image| *image != old_init)
        .collect();
    assert!(
        changed.is_empty(),
        "an active Job of the current generation is applied again with another pod template \
         (init image {old_init} -> {changed:?}); the API server refuses that as immutable"
    );
    assert!(applies(&seen).contains(&CHILDREN[3]), "{seen:?}");
}

/// CHANGELOG (Breaking): "an existing Command Deployment is deleted as a child this generation does
/// not render". The Deployment an earlier release made for a `Command` resource is found after the
/// upgrade's restart by its label and owner and deleted; the Job is created.
#[tokio::test]
async fn adv_a_command_deployment_of_an_earlier_release_is_deleted() {
    let (client, fake) = fake();
    let mut earlier = object(NAME, CHILDREN[2], Some(("LoomAgent", NAME, UID)));
    earlier["metadata"]["labels"] = json!({"app.kubernetes.io/instance": NAME});
    earlier["kind"] = json!("Deployment");
    earlier["apiVersion"] = json!("apps/v1");
    fake.put(CHILDREN[2], earlier);
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    let seen = fake.take();
    assert_eq!(applies(&seen), COMMAND_CHILDREN);
    assert_eq!(paths(&seen, &Method::DELETE), [CHILDREN[2]]);
    assert!(!fake.holds(CHILDREN[2]), "the earlier Deployment is gone");
    assert!(fake.holds(JOB));
    assert_eq!(published(&seen)["state"], "Running");
}

/// After a restart the controller holds no name it applied, so a Job a `Command` generation left
/// is found only by listing `jobs`: a new generation that is `AgentServe` deletes it.
#[tokio::test]
async fn adv_after_a_restart_an_agent_serve_generation_deletes_the_job_it_finds_by_label() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.take();

    let (fresh_client, fresh) = self::fake();
    for (path, object) in fake.objects() {
        fresh.put(&path, object);
    }
    let restarted = context(fresh_client);
    reconciled(agent(2, &example(), DEFAULT_ROUTES), &restarted).await;
    let seen = fresh.take();
    assert!(applies(&seen).contains(&CHILDREN[2]), "{seen:?}");
    assert_eq!(paths(&seen, &Method::DELETE), [JOB]);
    assert!(!fresh.holds(JOB));
}

/// A Job's run ends by its condition or by its pod count, each alone: the controller reads
/// either (AGENTS.md § Operator: "condition `Complete` or `Failed` true, or `succeeded` or
/// `failed` above 0"), and a condition that is not True ends nothing.
#[tokio::test]
async fn adv_a_run_ends_by_its_count_or_its_condition_alone() {
    for (run, state) in [
        (json!({"succeeded": 1}), "Completed"),
        (
            json!({"conditions": [{"type": "Complete", "status": "True"}]}),
            "Completed",
        ),
        (json!({"failed": 1}), "Failed"),
        (
            json!({"conditions": [{"type": "Failed", "status": "True"}]}),
            "Failed",
        ),
        (
            json!({"active": 1, "conditions": [
                {"type": "Complete", "status": "False"},
                {"type": "Failed", "status": "Unknown"},
            ]}),
            "Running",
        ),
    ] {
        let (client, fake) = fake();
        let ctx = context(client);
        reconciled(command_agent(1, "run"), &ctx).await;
        fake.take();
        fake.set_run(JOB, Some(run.clone()));
        reconciled(command_agent(1, "run"), &ctx).await;
        let seen = fake.take();
        assert_eq!(published(&seen)["state"], state, "{run}");
        assert_eq!(
            applies(&seen).is_empty(),
            state != "Running",
            "{run}: a finished Job is not applied, an active one is: {seen:?}"
        );
    }
}

/// AGENTS.md § Operator: "After a controller restart a finished Job still there is observed, not
/// run again". The record is empty after the restart, so the resource passes Pending and Running
/// again; the finished Job of its generation moves it to Completed without an apply.
#[tokio::test]
async fn adv_after_a_restart_a_finished_job_is_observed_and_not_run_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.set_run(JOB, Some(succeeded()));
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.take();

    let (fresh_client, fresh) = self::fake();
    for (path, object) in fake.objects() {
        fresh.put(&path, object);
    }
    let restarted = context(fresh_client);
    reconciled(command_agent(1, "run"), &restarted).await;
    let seen = fresh.take();
    assert!(applies(&seen).is_empty(), "{seen:?}");
    assert!(paths(&seen, &Method::DELETE).is_empty(), "{seen:?}");
    let status = published(&seen);
    assert_eq!(status["state"], "Completed");
    assert_eq!(status["observedGeneration"], 1);
}

// Adversary cases for story:command-run-exit-status (wave 2026-10-26-cmdv, unit u2).

/// A pod of the resource's Job named `name`, controlled by the Job when `controller`, its
/// container statuses `containers` as written.
fn adv_pod(name: &str, controller: bool, containers: Value) -> (String, Value) {
    let path = format!("{PODS}/{name}");
    let pod = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": name,
            "namespace": "agents",
            "uid": format!("uid-of-{path}"),
            "labels": {"batch.kubernetes.io/controller-uid": format!("uid-of-{JOB}")},
            "ownerReferences": [{
                "apiVersion": "batch/v1", "kind": "Job", "name": NAME,
                "uid": format!("uid-of-{JOB}"), "controller": controller,
            }],
        },
        "status": {"containerStatuses": containers},
    });
    (path, pod)
}

/// A container status of `name` in `state`.
fn adv_container(name: &str, state: Value) -> Value {
    json!({
        "name": name, "image": "registry.example/takt:0.12.0", "imageID": "",
        "ready": false, "restartCount": 0, "state": state,
    })
}

/// A pod may run a container beside the program (a mesh sidecar an admission webhook injects
/// first): only the program container `loom-agent` speaks for the run, even when the sidecar is
/// listed first and terminated later with exit code 0.
#[tokio::test]
async fn adv_a_sidecar_does_not_speak_for_the_program() {
    let (path, pod) = adv_pod(
        "platform-operator-x1",
        true,
        json!([
            adv_container(
                "mesh-proxy",
                terminated(0, "2026-10-26T13:00:00Z", "Completed")
            ),
            adv_container("loom-agent", terminated(3, "2026-10-26T12:00:00Z", "Error")),
        ]),
    );
    let (status, _) = ended_with(failed(), vec![(path, pod)]).await;
    assert_eq!(
        status["run"],
        json!({"exitCode": 3, "finishedAt": "2026-10-26T12:00:00Z", "reason": "Error"}),
        "the program container's end, not the sidecar's"
    );
}

/// Of several pods the Job controls, the one whose program terminated last answers, whatever
/// order the list returns them in.
#[tokio::test]
async fn adv_the_jobs_pod_that_ended_last_answers() {
    let (later_path, later) = adv_pod(
        "platform-operator-a",
        true,
        json!([adv_container(
            "loom-agent",
            terminated(7, "2026-10-26T12:30:00Z", "Error")
        )]),
    );
    let (earlier_path, earlier) = adv_pod(
        "platform-operator-b",
        true,
        json!([adv_container(
            "loom-agent",
            terminated(1, "2026-10-26T12:00:00Z", "Error")
        )]),
    );
    let (status, _) =
        ended_with(failed(), vec![(later_path, later), (earlier_path, earlier)]).await;
    assert_eq!(
        status["run"],
        json!({"exitCode": 7, "finishedAt": "2026-10-26T12:30:00Z", "reason": "Error"})
    );
}

/// A pod that names the Job's uid in an owner reference that is not its controller does not
/// speak for the run.
#[tokio::test]
async fn adv_a_pod_the_job_does_not_control_does_not_speak_for_the_run() {
    let (path, pod) = adv_pod(
        "platform-operator-x1",
        false,
        json!([adv_container(
            "loom-agent",
            terminated(0, "2026-10-26T12:00:00Z", "Completed")
        )]),
    );
    let (status, _) = ended_with(failed(), vec![(path, pod)]).await;
    assert_eq!(status["state"], "Failed");
    assert_eq!(status["run"], json!({}));
}

/// A program the kernel killed (exit code 137, `OOMKilled`) whose runtime recorded no
/// `finishedAt` reports the exit code and reason, and no end time.
#[tokio::test]
async fn adv_a_killed_program_without_an_end_time_reports_137() {
    let (path, pod) = adv_pod(
        "platform-operator-x1",
        true,
        json!([adv_container(
            "loom-agent",
            json!({"terminated": {"exitCode": 137, "signal": 9, "reason": "OOMKilled"}})
        )]),
    );
    let (status, events) = ended_with(failed(), vec![(path, pod)]).await;
    assert_eq!(
        status["run"],
        json!({"exitCode": 137, "reason": "OOMKilled"})
    );
    assert_eq!(
        message(&status, "Completed"),
        "the program exited with code 137 (OOMKilled)"
    );
    assert_eq!(
        events.last(),
        Some(&ResourceEvent::Failed(LoomAgentFailed {
            name: name(),
            generation: 1,
            exit_code: Some(137),
        }))
    );
}

/// A new generation clears the run the earlier one recorded: the status publishes no `run`, and
/// `Completed` reads `RunNotFinished` again once the new Job runs.
#[tokio::test]
async fn adv_a_new_generation_clears_the_run() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    let (path, pod) = pod(
        "platform-operator-x1",
        &format!("uid-of-{JOB}"),
        terminated(3, "2026-10-26T12:00:00Z", "Error"),
    );
    fake.put(&path, pod);
    fake.set_run(JOB, Some(failed()));
    reconciled(command_agent(1, "run"), &ctx).await;
    assert_eq!(published(&fake.take())["run"]["exitCode"], 3);
    fake.remove(&path);
    fake.set_run(JOB, None);

    reconciled(command_agent(2, "run, --again"), &ctx).await;
    let status = published(&fake.take());
    assert_eq!(status["state"], "Running");
    assert!(status.get("run").is_none(), "run cleared: {status}");
    assert_eq!(
        conditions(&status)[2],
        triple("Completed", "Unknown", "RunNotFinished")
    );
}

/// After a controller restart a finished Job still there is observed, not run again, and its
/// pod's exit code is read again: the status the restarted controller publishes keeps `run`.
#[tokio::test]
async fn adv_a_restarted_controller_reads_the_finished_runs_exit_code_again() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    let (path, pod) = pod(
        "platform-operator-x1",
        &format!("uid-of-{JOB}"),
        terminated(0, "2026-10-26T12:00:00Z", "Completed"),
    );
    fake.put(&path, pod);
    fake.set_run(JOB, Some(succeeded()));
    reconciled(command_agent(1, "run"), &ctx).await;
    fake.take();

    let (restarted_client, restarted) = self::fake();
    for (at, object) in fake.objects() {
        restarted.put(&at, object);
    }
    let after = context(restarted_client);
    reconciled(command_agent(1, "run"), &after).await;
    let seen = restarted.take();
    assert!(!applies(&seen).contains(&JOB), "not run again: {seen:?}");
    let status = published(&seen);
    assert_eq!(status["state"], "Completed");
    assert_eq!(
        status["run"],
        json!({"exitCode": 0, "finishedAt": "2026-10-26T12:00:00Z", "reason": "Completed"})
    );
}

/// The Job controller counts a pod that is terminating (deleted or evicted while it runs: a node
/// drain, a preemption) as failed at once, `status.failed: 1`, while the program still runs out
/// its grace period; the API server then adds `FailureTarget` and only writes `Failed` once the
/// pod has terminated. The controller moves the resource to Failed on `failed: 1` and reads the
/// pod then, before its container terminated, and a Failed resource is never read again: the
/// program's exit code (143 here, SIGTERM) is lost for good although the pod reports it seconds
/// later and stays readable.
#[tokio::test]
async fn adv_a_run_counted_failed_while_its_pod_terminates_reports_the_exit_code() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(command_agent(1, "run"), &ctx).await;
    let mut seen = fake.take();

    let (path, mut pod) = adv_pod(
        "platform-operator-x1",
        true,
        json!([adv_container(
            "loom-agent",
            json!({"running": {"startedAt": "2026-10-26T11:59:00Z"}})
        )]),
    );
    pod["metadata"]["deletionTimestamp"] = json!("2026-10-26T12:00:00Z");
    fake.put(&path, pod.clone());
    fake.set_run(
        JOB,
        Some(json!({"failed": 1, "conditions": [{
            "type": "FailureTarget", "status": "True", "reason": "BackoffLimitExceeded",
        }]})),
    );
    reconciled(command_agent(1, "run"), &ctx).await;
    seen.extend(fake.take());

    pod["status"]["containerStatuses"] = json!([adv_container(
        "loom-agent",
        terminated(143, "2026-10-26T12:00:20Z", "Error")
    )]);
    fake.put(&path, pod);
    fake.set_run(
        JOB,
        Some(json!({"failed": 1, "conditions": [
            {"type": "FailureTarget", "status": "True", "reason": "BackoffLimitExceeded"},
            {"type": "Failed", "status": "True", "reason": "BackoffLimitExceeded"},
        ]})),
    );
    reconciled(command_agent(1, "run"), &ctx).await;
    seen.extend(fake.take());

    let status = published(&seen);
    assert_eq!(status["state"], "Failed");
    assert_eq!(
        status["run"]["exitCode"], 143,
        "the program's exit code, readable from the Job's own pod once it terminated, never \
         reaches the status: {status}"
    );
}
