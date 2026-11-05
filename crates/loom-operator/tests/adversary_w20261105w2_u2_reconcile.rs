//! Adversary pass on unit U2 of wave 2026-11-05-w2 (`story:job-available-after-cronjob`): what a
//! single-run `Command` resource's `Available` reads while its Job waits for a CronJob of its
//! name to go, against the fake API server of `tests/adversary_w20261105w1_u4_reconcile.rs`
//! (kube's mock client over `tower-test`, in-process, copied unchanged). Nothing reaches a
//! cluster.
#![allow(dead_code)]

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
const ACCOUNT: &str = "/api/v1/namespaces/agents/serviceaccounts/platform-operator";
const CONFIG_MAP: &str = "/api/v1/namespaces/agents/configmaps/platform-operator-spec";
const PREDECESSOR: &str = "uid-of-the-predecessor";
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

/// `Available` as `(status, reason, message)` of the last status published.
fn available(statuses: &Arc<Mutex<Vec<Value>>>) -> (String, String, String) {
    condition(&last_published(statuses), "Available")
}

/// The message `Available` reads while `workload` waits for the CronJob of the name.
fn waiting_for_the_cron_job(workload: &str) -> String {
    format!(
        "waiting for CronJob platform-operator to be deleted before the {workload} is created; no \
         run starts until then"
    )
}

/// An object at `path` named `name` whose controller is a deleted `LoomAgent` of the resource's
/// name: what a predecessor leaves until the garbage collector removes it.
fn left_by_the_predecessor(name: &str, path: &str) -> Value {
    json!({"metadata": {
        "name": name, "namespace": "agents",
        "uid": format!("uid-of-{path}"),
        "ownerReferences": [{
            "apiVersion": "loom.beyond10x.dev/v1alpha1", "kind": "LoomAgent",
            "name": NAME, "uid": PREDECESSOR, "controller": true,
        }],
    }})
}

/// Every child a deleted scheduled predecessor left: its ServiceAccount, ConfigMap, CronJob and
/// NetworkPolicy, as they stand between the predecessor's (background) deletion and the garbage
/// collector's removal of its dependents.
fn put_a_scheduled_predecessors_children(fake: &Fake) {
    for (name, path) in [
        (NAME, ACCOUNT),
        ("platform-operator-spec", CONFIG_MAP),
        (NAME, CRON_JOB),
        (NAME, NETWORK_POLICY),
    ] {
        fake.put(path, left_by_the_predecessor(name, path));
    }
}

fn applied(seen: &[Seen]) -> Vec<String> {
    seen.iter()
        .filter(|seen| seen.method == Method::PATCH && seen.path != STATUS)
        .map(|seen| seen.path.clone())
        .collect()
}

fn deleted(seen: &[Seen]) -> Vec<String> {
    seen.iter()
        .filter(|seen| seen.method == Method::DELETE)
        .map(|seen| seen.path.clone())
        .collect()
}

/// RED. `story:job-available-after-cronjob`, acceptance: "During the switch `Available` reads
/// False with the reason `WorkloadTerminating`, naming the CronJob". The docs
/// (`docs/design/loom-agent-crd-deployment.md`, `AGENTS.md`) extend that to "one created again
/// under the name of a scheduled predecessor". The unit's own predecessor test puts only the
/// predecessor's CronJob; a real predecessor leaves its ServiceAccount, ConfigMap and
/// NetworkPolicy beside it. Those are in the survey, so the reconcile takes the predecessor path
/// (`reconcile.rs`, `if !predecessors.is_empty()`), deletes them and publishes `deleted`. The
/// survey of a single-run resource never reads a CronJob (`wanted` names the Job), so `deleted`
/// never holds one, `switching` stays empty, and `Available` reads Unknown `RunNotFinished`
/// while the predecessor's CronJob is untouched and no Job exists. The scheduled counterpart
/// (the GREEN case below) reads `WorkloadTerminating` in the same round.
#[tokio::test]
async fn adv_w2u2_a_scheduled_predecessors_children_read_workload_terminating_in_the_first_round() {
    let (client, fake, statuses) = fake();
    put_a_scheduled_predecessors_children(&fake);
    let ctx = context(client);

    let action = reconciled(command_agent(1), &ctx).await;
    assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    let seen = fake.take();
    assert!(applied(&seen).is_empty(), "{seen:?}");
    assert!(
        fake.holds_live(CRON_JOB),
        "the predecessor's CronJob is still there"
    );
    assert!(!fake.holds(JOB), "no Job exists");
    let (status, reason, message) = available(&statuses);
    assert_eq!(
        (status.as_str(), reason.as_str()),
        ("False", "WorkloadTerminating"),
        "the Job waits for the predecessor's CronJob and no run has started, yet Available \
         reads {status} {reason}: {message}"
    );
    assert!(message.contains("CronJob platform-operator"), "{message}");
}

/// GREEN probe, the counterpart of the RED case above: the same predecessor children before a
/// scheduled resource read `WorkloadTerminating` in the first round, naming what was deleted.
#[tokio::test]
async fn adv_w2u2_the_scheduled_counterpart_reads_workload_terminating_in_the_first_round() {
    let (client, fake, statuses) = fake();
    put_a_scheduled_predecessors_children(&fake);
    let ctx = context(client);

    reconciled(scheduled_agent(1), &ctx).await;
    let (status, reason, message) = available(&statuses);
    assert_eq!(
        (status.as_str(), reason.as_str()),
        ("False", "WorkloadTerminating"),
        "{message}"
    );
    assert!(
        message.contains("before the CronJob is created"),
        "{message}"
    );
}

/// GREEN probe. Schedule removed, given back and removed again while the CronJob's foreground
/// deletion never completes: each round names the CronJob with the workload it holds back, no
/// child is applied, `observedGeneration` follows each generation, and once the CronJob is gone
/// the Job is created and `Available` follows its run.
#[tokio::test]
async fn adv_w2u2_switching_back_and_forth_names_the_cronjob_each_way() {
    let (client, fake, statuses) = fake();
    let ctx = context(client);
    reconciled(scheduled_agent(1), &ctx).await;
    assert!(fake.holds_live(CRON_JOB));
    fake.take();
    fake.set_lingering(true);

    for (generation, scheduled, workload) in [
        (2, false, "Job"),
        (3, true, "CronJob"),
        (4, false, "Job"),
        (4, false, "Job"),
    ] {
        let resource = if scheduled {
            scheduled_agent(generation)
        } else {
            command_agent(generation)
        };
        let action = reconciled(resource, &ctx).await;
        assert_eq!(
            action,
            Action::requeue(Duration::from_secs(30)),
            "{generation}"
        );
        let seen = fake.take();
        assert!(applied(&seen).is_empty(), "{generation}: {seen:?}");
        assert!(!fake.holds(JOB), "{generation}");
        let status = last_published(&statuses);
        assert_eq!(status["observedGeneration"], generation, "{status}");
        assert_eq!(status["state"], "Running", "{status}");
        assert_eq!(
            condition(&status, "Available"),
            (
                "False".to_owned(),
                "WorkloadTerminating".to_owned(),
                waiting_for_the_cron_job(workload)
            ),
            "{generation}"
        );
    }

    fake.remove(CRON_JOB);
    reconciled(command_agent(4), &ctx).await;
    assert!(fake.holds(JOB) && !fake.holds(CRON_JOB));
    let (status, reason, _) = available(&statuses);
    assert_eq!(
        (status.as_str(), reason.as_str()),
        ("Unknown", "RunNotFinished")
    );
}

/// GREEN probe. An operator restarted mid-switch (a new `Context`, nothing remembered) surveys
/// afresh and still reads the wait, deleting the CronJob again, then the Job's run once the
/// CronJob is gone.
#[tokio::test]
async fn adv_w2u2_a_restart_mid_switch_still_reads_the_wait() {
    let (client, fake, statuses) = fake();
    let first = context(client.clone());
    reconciled(scheduled_agent(1), &first).await;
    fake.set_lingering(true);
    reconciled(command_agent(2), &first).await;
    assert_eq!(available(&statuses).2, waiting_for_the_cron_job("Job"));
    fake.take();

    let restarted = context(client);
    for round in 0..2 {
        reconciled(command_agent(2), &restarted).await;
        let seen = fake.take();
        assert_eq!(deleted(&seen), [CRON_JOB], "{round}");
        assert!(applied(&seen).is_empty(), "{round}: {seen:?}");
        let status = last_published(&statuses);
        assert_eq!(status["observedGeneration"], 2);
        assert_eq!(
            condition(&status, "Available"),
            (
                "False".to_owned(),
                "WorkloadTerminating".to_owned(),
                waiting_for_the_cron_job("Job")
            ),
            "{round}"
        );
        assert_eq!(
            condition(&status, "Completed").1,
            "RunNotFinished",
            "{round}"
        );
    }

    fake.remove(CRON_JOB);
    reconciled(command_agent(2), &restarted).await;
    assert!(fake.holds(JOB));
    assert_eq!(available(&statuses).1, "RunNotFinished");
}
