//! Adversary pass on unit U4 of wave 2026-11-05-w1 (`story:schedule-available-after-cronjob`):
//! what a scheduled `Command` resource's `Available` reads around its CronJob's creation, against
//! the fake API server of `tests/adversary_w20261102w2_b_reconcile.rs` (kube's mock client over
//! `tower-test`, in-process), with two changes the real API server makes: an apply to an object
//! being deleted keeps its `deletionTimestamp` and finalizers, and an apply to a path in
//! `refusing` is answered 403. Nothing reaches a cluster.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::reconcile::{Context, reconcile};
use b10x_loom_operator::resource::LoomAgent;
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
const JOB: &str = "/apis/batch/v1/namespaces/agents/jobs/platform-operator";
const CRON_JOB: &str = "/apis/batch/v1/namespaces/agents/cronjobs/platform-operator";
const NETWORK_POLICY: &str =
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator";
const RUNNER_ACCOUNT: &str = "/api/v1/namespaces/agents/serviceaccounts/platform-operator-runner";
const SCHEDULE: &str = "  schedule: {cron: '*/20 * * * *'}\n";
const NOW: &str = "2026-10-26T12:00:30Z";

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
}

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

#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    lingering: bool,
    refusing: BTreeSet<String>,
}

fn not_found() -> (StatusCode, Value) {
    (
        StatusCode::NOT_FOUND,
        json!({
            "kind": "Status", "apiVersion": "v1", "metadata": {},
            "status": "Failure", "reason": "NotFound", "code": 404, "message": "not found",
        }),
    )
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
            Method::PATCH if self.refusing.contains(path) => (
                StatusCode::FORBIDDEN,
                json!({
                    "kind": "Status", "apiVersion": "v1", "metadata": {},
                    "status": "Failure", "reason": "Forbidden", "code": 403,
                    "message": "refused by an admission policy",
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
                // The API server never clears a deletion by an apply: the object stays
                // terminating with its finalizers.
                if let Some(old) = self.objects.get(path)
                    && !old["metadata"]["deletionTimestamp"].is_null()
                {
                    object["metadata"]["deletionTimestamp"] =
                        old["metadata"]["deletionTimestamp"].clone();
                    object["metadata"]["finalizers"] = old["metadata"]["finalizers"].clone();
                }
                self.objects.insert(path.to_owned(), object.clone());
                (StatusCode::OK, object)
            }
            Method::GET => match self.objects.get(path) {
                Some(object) => (StatusCode::OK, object.clone()),
                None => not_found(),
            },
            Method::DELETE if self.lingering && body["propagationPolicy"] == "Foreground" => {
                match self.objects.get_mut(path) {
                    Some(object) => {
                        object["metadata"]["deletionTimestamp"] = json!("2026-10-26T00:00:00Z");
                        object["metadata"]["finalizers"] = json!(["foregroundDeletion"]);
                        (StatusCode::OK, object.clone())
                    }
                    None => not_found(),
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

struct Fake {
    server: Arc<Mutex<Server>>,
}

impl Fake {
    fn set_lingering(&self, lingering: bool) {
        self.server.lock().expect("the fake").lingering = lingering;
    }

    fn refuse(&self, path: &str) {
        self.server
            .lock()
            .expect("the fake")
            .refusing
            .insert(path.to_owned());
    }

    fn take(&self) -> Vec<Seen> {
        std::mem::take(&mut self.server.lock().expect("the fake").seen)
    }

    fn put(&self, path: &str, object: Value) {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .insert(path.to_owned(), object);
    }

    fn get(&self, path: &str) -> Option<Value> {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .get(path)
            .cloned()
    }

    fn holds(&self, path: &str) -> bool {
        self.get(path).is_some()
    }

    /// Whether the object at `path` exists and is not being deleted.
    fn holds_live(&self, path: &str) -> bool {
        self.get(path)
            .is_some_and(|object| object["metadata"]["deletionTimestamp"].is_null())
    }

    fn remove(&self, path: &str) {
        self.server.lock().expect("the fake").objects.remove(path);
    }
}

/// The fake server and the bodies of its status patches, in order.
fn fake() -> (Client, Fake, Arc<Mutex<Vec<Value>>>) {
    let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
    let server = Arc::new(Mutex::new(Server::default()));
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let shared = Arc::clone(&server);
    let published = Arc::clone(&statuses);
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
            if parts.method == Method::PATCH && path == STATUS {
                published
                    .lock()
                    .expect("the statuses")
                    .push(body["status"].clone());
            }
            let (status, answer) = {
                let mut server = shared.lock().expect("the fake");
                let answer = server.answer(&parts.method, &path, &query, &body);
                server.seen.push(Seen {
                    method: parts.method,
                    path,
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
    (Client::new(service, "agents"), Fake { server }, statuses)
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn takt_spec() -> String {
    fs::read_to_string(root().join("examples/agents/operator.yaml"))
        .expect("the example spec")
        .replace("protocol: operator-reply@1", "protocol: takt-delivery@1")
}

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

fn rest(schedule: &str) -> String {
    format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [run]}}\n{schedule}"
    )
}

fn command_agent(generation: i64) -> LoomAgent {
    agent(generation, &takt_spec(), &rest(""))
}

fn scheduled_agent(generation: i64) -> LoomAgent {
    agent(generation, &takt_spec(), &rest(SCHEDULE))
}

fn context(client: Client) -> Arc<Context> {
    let config = Config::parse(CONFIG).expect("the test configuration");
    Arc::new(
        Context::new(client, config)
            .expect("the context")
            .with_clock(|| NOW.parse().expect("an instant")),
    )
}

async fn reconciled(resource: LoomAgent, ctx: &Arc<Context>) -> Action {
    reconcile(Arc::new(resource), Arc::clone(ctx))
        .await
        .expect("the reconcile succeeds")
}

fn last_published(statuses: &Arc<Mutex<Vec<Value>>>) -> Value {
    statuses
        .lock()
        .expect("the statuses")
        .last()
        .cloned()
        .expect("a status patch")
}

/// The status, reason and message of the condition `kind` of `status`.
fn condition(status: &Value, kind: &str) -> (String, String, String) {
    let found = status["conditions"]
        .as_array()
        .expect("conditions")
        .iter()
        .find(|condition| condition["type"] == kind)
        .unwrap_or_else(|| panic!("{kind} in {status}"));
    (
        found["status"].as_str().expect("status").to_owned(),
        found["reason"].as_str().expect("reason").to_owned(),
        found["message"].as_str().expect("message").to_owned(),
    )
}

/// RED. `story:schedule-available-after-cronjob`, outcome: "`Available` True `Scheduled` only
/// once the CronJob exists". Removing `spec.schedule` deletes the CronJob in the foreground, and
/// it stays terminating while its runs' pods go; giving the schedule back meanwhile (a revert, a
/// GitOps sync of the earlier manifest) waits for nothing: `workloads_terminating` asks only for
/// a Job and a Deployment of the name, the apply lands on the terminating CronJob, and
/// `Available` reads True `Scheduled` while the only CronJob is being deleted (Kubernetes' CronJob
/// controller starts no run for one with a `deletionTimestamp`, and the garbage collector removes
/// it). The CronJob of the resource's own name that is still terminating is a child the CronJob
/// waits for as much as the Job is.
#[tokio::test]
async fn adv_u4_a_schedule_given_back_while_its_cronjob_terminates_does_not_read_scheduled() {
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1), &ctx).await;
    assert!(fake.holds_live(CRON_JOB));
    assert_eq!(
        condition(&last_published(&statuses), "Available").1,
        "Scheduled"
    );

    fake.set_lingering(true);
    let action = reconciled(command_agent(2), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    assert!(fake.holds(CRON_JOB) && !fake.holds_live(CRON_JOB));
    assert!(!fake.holds(JOB));
    fake.take();

    reconciled(scheduled_agent(3), &ctx).await;
    assert!(
        fake.holds(CRON_JOB) && !fake.holds_live(CRON_JOB),
        "the fake keeps the CronJob terminating, as the API server does"
    );
    let (status, reason, message) = condition(&last_published(&statuses), "Available");
    assert_eq!(
        (status.as_str(), reason.as_str()),
        ("False", "WorkloadTerminating"),
        "the only CronJob is being deleted, yet Available reads: {message}"
    );
    assert!(message.contains("CronJob platform-operator"), "{message}");
}

/// RED. The status is published only at the end of a reconcile, so one that applies the CronJob
/// and then fails (the NetworkPolicy apply refused, here by an admission policy; equally a
/// transient API error on the NetworkPolicy, the Service, the Jobs list of `newest_run` or the
/// sweep) leaves the previous round's `Available` False `WorkloadTerminating` "before the CronJob
/// is created; no run starts until then" in place while the CronJob exists and starts runs. For
/// as long as the failure repeats, every retry applies the CronJob again and publishes nothing.
/// Before the unit the stale status read `Scheduled`, which the CronJob made true.
#[tokio::test]
async fn adv_u4_a_reconcile_failing_after_the_cronjob_is_applied_leaves_workload_terminating() {
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(command_agent(1), &ctx).await;
    assert!(fake.holds(JOB));

    fake.set_lingering(true);
    reconciled(scheduled_agent(2), &ctx).await;
    assert_eq!(
        condition(&last_published(&statuses), "Available").1,
        "WorkloadTerminating"
    );

    fake.remove(JOB);
    fake.refuse(NETWORK_POLICY);
    for round in 0..2 {
        let answer = reconcile(Arc::new(scheduled_agent(2)), Arc::clone(&ctx)).await;
        assert!(answer.is_err(), "{round}: the NetworkPolicy apply is refused");
        assert!(fake.holds_live(CRON_JOB), "{round}: the CronJob is applied");
    }
    let (status, reason, message) = condition(&last_published(&statuses), "Available");
    assert!(
        !message.contains("before the CronJob is created"),
        "the CronJob exists, yet the published status reads {status} {reason}: {message}"
    );
}

/// RED, constructed. The predecessor path publishes every predecessor child it deleted as what
/// the CronJob waits for, whether or not the resource's own CronJob already exists. Here the
/// resource's CronJob runs (generation 1) and generation 2 names a ServiceAccount
/// (`deploy.kubernetes.service_account`) that a deleted predecessor of the same name still
/// holds: `Available` reads False `WorkloadTerminating` "before the CronJob is created; no run
/// starts until then", though the CronJob exists, is not touched, and starts runs.
#[tokio::test]
async fn adv_u4_a_predecessors_account_beside_a_live_cronjob_does_not_read_workload_terminating()
{
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1), &ctx).await;
    assert!(fake.holds_live(CRON_JOB));

    fake.put(
        RUNNER_ACCOUNT,
        json!({"metadata": {
            "name": "platform-operator-runner", "namespace": "agents",
            "uid": format!("uid-of-{RUNNER_ACCOUNT}"),
            "ownerReferences": [{
                "apiVersion": "loom.beyond10x.dev/v1alpha1", "kind": "LoomAgent",
                "name": NAME, "uid": "uid-of-the-predecessor", "controller": true,
            }],
        }}),
    );
    fake.set_lingering(true);
    let renamed = takt_spec().replace(
        "service_account: platform-operator",
        "service_account: platform-operator-runner",
    );
    reconciled(agent(2, &renamed, &rest(SCHEDULE)), &ctx).await;
    assert!(fake.holds_live(CRON_JOB), "the CronJob of generation 1 stays");
    let (status, reason, message) = condition(&last_published(&statuses), "Available");
    assert!(
        !message.contains("before the CronJob is created"),
        "the CronJob exists, yet Available reads {status} {reason}: {message}"
    );
}

/// GREEN probe. A deletion that never completes: every round publishes the same status (no
/// patch churn would follow on a resource that carries it), requeues after 30 s and names both
/// lingering workloads in a fixed order.
#[tokio::test]
async fn adv_u4_a_deletion_that_never_completes_keeps_one_stable_message() {
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(command_agent(1), &ctx).await;
    fake.put(
        "/apis/apps/v1/namespaces/agents/deployments/platform-operator",
        json!({"metadata": {
            "name": NAME, "namespace": "agents",
            "uid": "uid-of-the-deployment",
            "ownerReferences": [{
                "apiVersion": "loom.beyond10x.dev/v1alpha1", "kind": "LoomAgent",
                "name": NAME, "uid": UID, "controller": true,
            }],
        }}),
    );
    fake.set_lingering(true);
    let mut messages = BTreeSet::new();
    for _ in 0..3 {
        let action = reconciled(scheduled_agent(2), &ctx).await;
        assert_eq!(action, Action::requeue(Duration::from_secs(30)));
        assert!(!fake.holds(CRON_JOB));
        let (status, reason, message) = condition(&last_published(&statuses), "Available");
        assert_eq!((status.as_str(), reason.as_str()), ("False", "WorkloadTerminating"));
        messages.insert(message);
    }
    assert_eq!(
        messages.into_iter().collect::<Vec<_>>(),
        ["waiting for Job platform-operator, Deployment platform-operator to be deleted before \
          the CronJob is created; no run starts until then"]
    );
}
