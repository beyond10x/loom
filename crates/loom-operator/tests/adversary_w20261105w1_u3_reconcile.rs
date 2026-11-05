//! Adversary pass on unit U3 of wave 2026-11-05-w1 (`story:schedule-run-deadline`): the
//! reconcile of a scheduled `Command` resource with `spec.schedule.runDeadlineSeconds`, against
//! the fake API server of `tests/adversary_w20261102w2_b_reconcile.rs` (kube's mock client over
//! `tower-test`, in-process). Nothing reaches a cluster.

#![allow(dead_code, unused_imports)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::reconcile::{Context, reconcile};
use b10x_loom_operator::resource::LoomAgent;
use b10x_loom_resource::ResourceEvent;
use b10x_loom_resource::model::LoomAgentLastRunObserved;
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
const JOBS: &str = "/apis/batch/v1/namespaces/agents/jobs";
const CRON_JOB: &str = "/apis/batch/v1/namespaces/agents/cronjobs/platform-operator";
const PODS: &str = "/api/v1/namespaces/agents/pods";
const SCHEDULE: &str = "  schedule: {cron: '*/20 * * * *'}\n";
const NOW: &str = "2026-10-26T12:00:30Z";

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

/// The fake API server of `tests/reconcile.rs`, reduced to what these cases steer: applies answer
/// the object with a uid naming its path (a Job gets its selector), reads the last object at the
/// path or 404, lists by label selector, and a `Foreground` delete leaves the object terminating
/// while `lingering`.
#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    lingering: bool,
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
            Method::PATCH => {
                let mut object = body.clone();
                object["metadata"]["uid"] = json!(format!("uid-of-{path}"));
                if path.contains("/jobs/") {
                    object["spec"]["selector"] = json!({"matchLabels": {
                        "batch.kubernetes.io/controller-uid": format!("uid-of-{path}"),
                    }});
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

    fn holds(&self, path: &str) -> bool {
        self.server
            .lock()
            .expect("the fake")
            .objects
            .contains_key(path)
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

fn command_agent(generation: i64) -> LoomAgent {
    let rest = format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [run]}}\n"
    );
    agent(generation, &takt_spec(), &rest)
}

fn scheduled_agent(generation: i64, schedule: &str) -> LoomAgent {
    let rest = format!(
        "{DEFAULT_ROUTES}  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [run]}}\n{schedule}"
    );
    agent(generation, &takt_spec(), &rest)
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

fn paths<'a>(seen: &'a [Seen], method: &Method) -> Vec<&'a str> {
    seen.iter()
        .filter(|seen| seen.method == *method)
        .map(|seen| seen.path.as_str())
        .collect()
}

fn applies(seen: &[Seen]) -> Vec<&str> {
    seen.iter()
        .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        .map(|seen| seen.path.as_str())
        .collect()
}

fn last_published(statuses: &Arc<Mutex<Vec<Value>>>) -> Value {
    statuses
        .lock()
        .expect("the statuses")
        .last()
        .cloned()
        .expect("a status patch")
}

fn condition(status: &Value, kind: &str) -> (String, String) {
    let found = status["conditions"]
        .as_array()
        .expect("conditions")
        .iter()
        .find(|condition| condition["type"] == kind)
        .unwrap_or_else(|| panic!("{kind} in {status}"));
    (
        found["status"].as_str().expect("status").to_owned(),
        found["reason"].as_str().expect("reason").to_owned(),
    )
}

fn succeeded() -> Value {
    json!({"succeeded": 1, "conditions": [{"type": "Complete", "status": "True"}]})
}

fn failed() -> Value {
    json!({"failed": 1, "conditions": [{"type": "Failed", "status": "True"}]})
}

fn cron_uid() -> String {
    format!("uid-of-{CRON_JOB}")
}

/// A Job named `name` that the CronJob of uid `owner` made at `created`, with `status`.
fn cron_run(name: &str, owner: &str, created: &str, status: Value) -> (String, Value) {
    let path = format!("{JOBS}/{name}");
    let mut job = json!({
        "apiVersion": "batch/v1",
        "kind": "Job",
        "metadata": {
            "name": name,
            "namespace": "agents",
            "uid": format!("uid-of-{path}"),
            "creationTimestamp": created,
            "labels": {
                "app.kubernetes.io/instance": NAME,
                "app.kubernetes.io/managed-by": "b10x-loom-operator",
                "app.kubernetes.io/name": "loom-agent",
            },
            "ownerReferences": [{
                "apiVersion": "batch/v1", "kind": "CronJob", "name": NAME, "uid": owner,
                "controller": true,
            }],
        },
        "spec": {
            "selector": {"matchLabels": {
                "batch.kubernetes.io/controller-uid": format!("uid-of-{path}"),
            }},
            "template": {"spec": {"containers": [], "restartPolicy": "Never"}},
        },
    });
    if !status.is_null() {
        job["status"] = status;
    }
    (path, job)
}

/// The pod `name` of the Job at `job`, its program container in `state`.
fn cron_pod(name: &str, job: &str, state: Value) -> (String, Value) {
    let path = format!("{PODS}/{name}");
    let pod = json!({
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": name,
            "namespace": "agents",
            "uid": format!("uid-of-{path}"),
            "labels": {"batch.kubernetes.io/controller-uid": format!("uid-of-{job}")},
            "ownerReferences": [{
                "apiVersion": "batch/v1", "kind": "Job", "name": name,
                "uid": format!("uid-of-{job}"), "controller": true,
            }],
        },
        "status": {"containerStatuses": [{
            "name": "loom-agent", "image": "registry.example/takt:0.12.0", "imageID": "",
            "ready": false, "restartCount": 0, "state": state,
        }]},
    });
    (path, pod)
}

fn terminated(exit_code: i64, at: &str, reason: &str) -> Value {
    json!({"terminated": {"exitCode": exit_code, "finishedAt": at, "reason": reason}})
}

fn last_runs(events: &[ResourceEvent]) -> Vec<LoomAgentLastRunObserved> {
    events
        .iter()
        .filter_map(|event| match event {
            ResourceEvent::LastRunObserved(observed) => Some(observed.clone()),
            _ => None,
        })
        .collect()
}

/// Puts a finished run of the resource's CronJob, its Job and its pod, and answers the Job's path.
fn finished_run(fake: &Fake, name: &str, created: &str, exit_code: i64, ended: &str) -> String {
    let (path, job) = cron_run(
        name,
        &cron_uid(),
        created,
        if exit_code == 0 {
            succeeded()
        } else {
            failed()
        },
    );
    fake.put(&path, job);
    let (pod_path, pod) = cron_pod(
        &format!("{name}-pod"),
        &path,
        terminated(
            exit_code,
            ended,
            if exit_code == 0 { "Completed" } else { "Error" },
        ),
    );
    fake.put(&pod_path, pod);
    path
}

const DEADLINE: &str = "  schedule: {cron: '0 3 * * *', runDeadlineSeconds: 600}\n";

/// The Job status Kubernetes writes for a run it ended at `activeDeadlineSeconds`.
fn deadline_exceeded() -> Value {
    json!({
        "failed": 1,
        "conditions": [
            {"type": "FailureTarget", "status": "True", "reason": "DeadlineExceeded",
             "message": "Job was active longer than specified deadline"},
            {"type": "Failed", "status": "True", "reason": "DeadlineExceeded",
             "message": "Job was active longer than specified deadline"},
        ],
    })
}

/// RED. `docs/design/loom-agent-crd-deployment.md` (§ 12, Lifecycle and status): "A run ended at
/// `runDeadlineSeconds` is a failed run: its Job is `Failed` with the reason `DeadlineExceeded`,
/// so `lastRun` is `Failed` with that reason (no exit code when the pod is already gone)". The
/// Job controller ends such a run by deleting its pod, whose program container is then terminated
/// by SIGTERM (exit 143, reason `Error`); while that pod object still exists the controller reads
/// its container's end and reports the container's reason, so a run ended at its deadline reads
/// as one that crashed with `Error`, and nothing in `status.lastRun` says the deadline ended it.
#[tokio::test]
async fn adv_u3_a_deadline_ended_run_whose_pod_remains_reports_deadline_exceeded() {
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1, DEADLINE), &ctx).await;
    let (path, job) = cron_run(
        "platform-operator-29370180",
        &cron_uid(),
        "2026-11-05T03:00:00Z",
        deadline_exceeded(),
    );
    fake.put(&path, job);
    let (pod_path, pod) = cron_pod(
        "platform-operator-29370180-pod",
        &path,
        terminated(143, "2026-11-05T03:10:01Z", "Error"),
    );
    fake.put(&pod_path, pod);
    reconciled(scheduled_agent(1, DEADLINE), &ctx).await;
    let status = last_published(&statuses);
    assert_eq!(status["state"], "Running", "{status}");
    assert_eq!(
        condition(&status, "LastRunSucceeded"),
        ("False".to_owned(), "RunFailed".to_owned())
    );
    assert_eq!(status["lastRun"]["phase"], "Failed", "{status}");
    assert_eq!(
        status["lastRun"]["reason"], "DeadlineExceeded",
        "a run Kubernetes ended at its deadline is reported with that reason: {}",
        status["lastRun"]
    );
}

/// Removing `runDeadlineSeconds` from a scheduled resource applies the CronJob without
/// `activeDeadlineSeconds`, so the server-side apply drops the deadline it owned.
#[tokio::test]
async fn adv_u3_removing_the_run_deadline_applies_the_cronjob_without_one() {
    let (client, fake, _) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1, DEADLINE), &ctx).await;
    fake.take();
    reconciled(
        scheduled_agent(2, "  schedule: {cron: '0 3 * * *'}\n"),
        &ctx,
    )
    .await;
    let seen = fake.take();
    assert!(applies(&seen).contains(&CRON_JOB), "{seen:?}");
    let applied = fake.server.lock().expect("the fake").objects[CRON_JOB].clone();
    assert_eq!(applied["spec"]["schedule"], "0 3 * * *");
    assert!(
        applied["spec"]["jobTemplate"]["spec"]
            .get("activeDeadlineSeconds")
            .is_none(),
        "{applied}"
    );
}

/// A scheduled resource with a run deadline made unscheduled again runs one Job, and that Job
/// carries no deadline: the run deadline is the schedule's, not the resource's.
#[tokio::test]
async fn adv_u3_a_resource_unscheduled_again_runs_a_job_without_a_deadline() {
    let (client, fake, _) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1, DEADLINE), &ctx).await;
    assert!(fake.holds(CRON_JOB));
    for _ in 0..3 {
        reconciled(command_agent(2), &ctx).await;
        if fake.holds(JOB) {
            break;
        }
    }
    assert!(fake.holds(JOB) && !fake.holds(CRON_JOB));
    let job = fake.server.lock().expect("the fake").objects[JOB].clone();
    assert!(job["spec"].get("activeDeadlineSeconds").is_none(), "{job}");
}
