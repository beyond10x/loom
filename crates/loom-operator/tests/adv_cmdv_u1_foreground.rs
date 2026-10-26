//! Adversary cases for `story:command-variant-runs-once` (wave 2026-10-26-cmdv, unit u1, pass 2),
//! against a fake API server that answers a delete as the API server does: a `Foreground` delete
//! of an object that is there always answers it, terminating (`deletionTimestamp` and the
//! `foregroundDeletion` finalizer), never a `Status`, until the garbage collector removes it; a
//! delete whose uid precondition names another object answers 409; one of an object that is not
//! there answers 404.

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
const COMMAND_CHILDREN: [&str; 5] = [
    "/api/v1/namespaces/agents/serviceaccounts/platform-operator",
    "/api/v1/namespaces/agents/configmaps/platform-operator-spec",
    JOB,
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator",
    "/api/v1/namespaces/agents/services/platform-operator",
];

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
    run: Option<Value>,
    /// Paths whose object was replaced under its name by another one: a delete preconditioned on
    /// the uid read answers 409.
    replaced: BTreeSet<String>,
    /// Paths whose object someone else deleted between the controller's read and its delete.
    vanishing: BTreeSet<String>,
}

fn status(code: u16, reason: &str) -> Value {
    json!({"kind": "Status", "apiVersion": "v1", "metadata": {},
           "status": if code < 300 { "Success" } else { "Failure" },
           "reason": reason, "code": code, "message": reason})
}

impl Server {
    fn answer(&mut self, method: &Method, path: &str, query: &str, body: &Value) -> (u16, Value) {
        let selector = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("labelSelector="))
            .map(|s| s.replace("%3D", "=").replace("%2F", "/"));
        if let (&Method::GET, Some(selector)) = (method, selector) {
            let (key, value) = selector.split_once('=').expect("one label");
            let collection = format!("{path}/");
            let items: Vec<Value> = self
                .objects
                .iter()
                .filter(|(at, _)| {
                    at.strip_prefix(&collection)
                        .is_some_and(|name| !name.contains('/'))
                })
                .filter(|(_, object)| object["metadata"]["labels"][key] == value)
                .map(|(_, object)| object.clone())
                .collect();
            return (
                200,
                json!({"apiVersion": "v1", "kind": "List", "metadata": {"resourceVersion": "1"},
                       "items": items}),
            );
        }
        match *method {
            Method::PATCH if path.ends_with("/status") => (
                200,
                json!({"apiVersion": "loom.beyond10x.dev/v1alpha1", "kind": "LoomAgent",
                       "metadata": body["metadata"], "spec": {"agent": {}, "routes": {}},
                       "status": body["status"]}),
            ),
            Method::PATCH => {
                let mut object = body.clone();
                object["metadata"]["uid"] = json!(format!("uid-of-{path}"));
                if path.contains("/jobs/")
                    && let Some(run) = &self.run
                {
                    object["status"] = run.clone();
                }
                self.objects.insert(path.to_owned(), object.clone());
                (200, object)
            }
            Method::GET => match self.objects.get(path) {
                Some(object) => (200, object.clone()),
                None => (404, status(404, "NotFound")),
            },
            Method::DELETE => {
                if self.vanishing.remove(path) {
                    self.objects.remove(path);
                }
                if self.replaced.contains(path) {
                    return (409, status(409, "Conflict"));
                }
                let Some(object) = self.objects.get_mut(path) else {
                    return (404, status(404, "NotFound"));
                };
                if body["preconditions"]["uid"] != object["metadata"]["uid"] {
                    return (409, status(409, "Conflict"));
                }
                if body["propagationPolicy"] == "Foreground" {
                    object["metadata"]["deletionTimestamp"] = json!("2026-10-26T00:00:00Z");
                    object["metadata"]["finalizers"] = json!(["foregroundDeletion"]);
                    (200, object.clone())
                } else {
                    self.objects.remove(path);
                    (200, status(200, "Success"))
                }
            }
            _ => (405, json!({})),
        }
    }
}

struct Fake {
    server: Arc<Mutex<Server>>,
}

impl Fake {
    fn with<T>(&self, f: impl FnOnce(&mut Server) -> T) -> T {
        f(&mut self.server.lock().expect("the fake"))
    }

    fn take(&self) -> Vec<Seen> {
        self.with(|server| std::mem::take(&mut server.seen))
    }

    /// The garbage collector removes `path` once its dependents are gone.
    fn collect(&self, path: &str) {
        self.with(|server| server.objects.remove(path));
    }

    fn set_run(&self, run: Option<Value>) {
        self.with(|server| {
            if let (Some(job), Some(status)) = (server.objects.get_mut(JOB), &run) {
                job["status"] = status.clone();
            }
            server.run = run;
        });
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
            let query = parts.uri.query().unwrap_or_default().to_owned();
            let (code, answer) = {
                let mut server = shared.lock().expect("the fake");
                let answer = server.answer(&parts.method, &path, &query, &body);
                server.seen.push(Seen {
                    method: parts.method,
                    path,
                    body,
                });
                answer
            };
            let response = Response::builder()
                .status(StatusCode::from_u16(code).expect("a status code"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&answer).expect("JSON")))
                .expect("a response");
            send.send_response(response);
        }
    });
    (Client::new(service, "agents"), Fake { server })
}

fn command_agent(generation: i64) -> LoomAgent {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec = fs::read_to_string(root.join("examples/agents/operator.yaml"))
        .expect("the example spec")
        .replace("protocol: operator-reply@1", "protocol: takt-delivery@1");
    let mut text = format!(
        "apiVersion: loom.beyond10x.dev/v1alpha1\nkind: LoomAgent\nmetadata:\n  name: {NAME}\n  \
         namespace: agents\n  uid: {UID}\n  generation: {generation}\nspec:\n  agent:\n"
    );
    for line in spec.lines() {
        text.push_str("    ");
        text.push_str(line);
        text.push('\n');
    }
    text.push_str(DEFAULT_ROUTES);
    text.push_str(&format!(
        "  image: {{repository: registry.example/takt, tag: '0.12.0'}}\n  \
         command: {{args: [run, '{generation}']}}\n"
    ));
    serde_yaml_ng::from_str(&text).expect("a LoomAgent")
}

fn context(client: Client) -> Arc<Context> {
    Arc::new(Context::new(client, Config::parse(CONFIG).expect("config")).expect("the context"))
}

async fn reconciled(generation: i64, ctx: &Arc<Context>) -> Action {
    reconcile(Arc::new(command_agent(generation)), Arc::clone(ctx))
        .await
        .expect("the reconcile succeeds")
}

fn applies(seen: &[Seen]) -> Vec<&str> {
    seen.iter()
        .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        .map(|seen| seen.path.as_str())
        .collect()
}

fn deletes(seen: &[Seen]) -> Vec<&str> {
    seen.iter()
        .filter(|seen| seen.method == Method::DELETE)
        .map(|seen| seen.path.as_str())
        .collect()
}

/// The status the resource reads after `seen`: the last one published, else `before`.
fn status_after(seen: &[Seen], before: &Value) -> Value {
    seen.iter()
        .rev()
        .find(|seen| seen.method == Method::PATCH && seen.path == STATUS)
        .map_or_else(|| before.clone(), |seen| seen.body["status"].clone())
}

fn reason(status: &Value, kind: &str) -> String {
    status["conditions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|condition| condition["type"] == kind)
        .map(|condition| condition["reason"].as_str().unwrap_or_default().to_owned())
        .unwrap_or_default()
}

fn job_generation(fake: &Fake) -> Value {
    fake.with(|server| {
        server.objects[JOB]["spec"]["template"]["metadata"]["annotations"]
            ["loom.beyond10x.dev/generation"]
            .clone()
    })
}

/// Generation 1 run to its end: Completed, the Job still there.
async fn finished_generation_one(fake: &Fake, ctx: &Arc<Context>) -> Value {
    reconciled(1, ctx).await;
    fake.set_run(Some(
        json!({"succeeded": 1, "conditions": [{"type": "Complete", "status": "True"}]}),
    ));
    reconciled(1, ctx).await;
    let status = status_after(&fake.take(), &Value::Null);
    assert_eq!(status["state"], "Completed");
    fake.set_run(None);
    status
}

/// `a_new_generation_replaces_the_job`, re-pinned in 1b09f360 to one reconcile, lost its status
/// assertions and its later reconcile. Against the API server's real `Foreground` answer the
/// replacement always takes at least two reconciles; this walks the whole of it and asserts the
/// status at each step, the reconcile after the new Job exists (no delete, no resend), and that
/// the new generation's run ends as its own.
#[tokio::test]
async fn adv_cmdv_u1_a_new_generation_replaces_the_job_over_a_real_foreground_delete() {
    let (client, fake) = fake();
    let ctx = context(client);
    let mut status = finished_generation_one(&fake, &ctx).await;

    for round in 0..2 {
        let action = reconciled(2, &ctx).await;
        let seen = fake.take();
        assert_eq!(action, Action::requeue(Duration::from_secs(30)), "{round}");
        assert!(applies(&seen).is_empty(), "{round}: {seen:?}");
        assert_eq!(deletes(&seen), [JOB], "{round}");
        status = status_after(&seen, &status);
        assert_eq!(status["state"], "Running", "{round}");
        assert_eq!(status["observedGeneration"], 2, "{round}");
        assert_eq!(reason(&status, "Available"), "RunNotFinished", "{round}");
        assert_eq!(
            job_generation(&fake),
            "1",
            "{round}: the old Job still terminates"
        );
    }

    fake.collect(JOB);
    let action = reconciled(2, &ctx).await;
    let seen = fake.take();
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
    assert_eq!(applies(&seen), COMMAND_CHILDREN);
    assert!(deletes(&seen).is_empty(), "{seen:?}");
    assert_eq!(job_generation(&fake), "2");
    status = status_after(&seen, &status);
    assert_eq!(status["state"], "Running");
    assert_eq!(status["observedGeneration"], 2);

    let action = reconciled(2, &ctx).await;
    let seen = fake.take();
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
    assert!(
        deletes(&seen).is_empty(),
        "the new Job is not deleted again: {seen:?}"
    );
    assert!(
        !applies(&seen).contains(&JOB),
        "the new generation's Job is not sent again: {seen:?}"
    );
    status = status_after(&seen, &status);
    assert_eq!(status["state"], "Running");

    fake.set_run(Some(json!({"failed": 1})));
    reconciled(2, &ctx).await;
    let seen = fake.take();
    status = status_after(&seen, &status);
    assert_eq!(status["state"], "Failed");
    assert_eq!(status["observedGeneration"], 2);
    assert!(applies(&seen).is_empty(), "{seen:?}");
}

/// `delete_uid` answers a 409 "not gone": another object took the name between the read and the
/// delete. The Job must then not be created (nor the ConfigMap applied) in that reconcile.
#[tokio::test]
async fn adv_cmdv_u1_a_conflicting_delete_of_the_old_job_is_not_gone() {
    let (client, fake) = fake();
    let ctx = context(client);
    finished_generation_one(&fake, &ctx).await;
    fake.with(|server| server.replaced.insert(JOB.to_owned()));

    let action = reconciled(2, &ctx).await;
    let seen = fake.take();
    assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    assert_eq!(deletes(&seen), [JOB]);
    assert!(
        applies(&seen).is_empty(),
        "nothing applied on a 409: {seen:?}"
    );
}

/// `delete_uid` answers a 404 "gone": someone else deleted the old Job between the read and the
/// delete, so the new generation's Job is created in that same reconcile.
#[tokio::test]
async fn adv_cmdv_u1_an_old_job_already_gone_at_the_delete_is_gone() {
    let (client, fake) = fake();
    let ctx = context(client);
    finished_generation_one(&fake, &ctx).await;
    fake.with(|server| server.vanishing.insert(JOB.to_owned()));

    let action = reconciled(2, &ctx).await;
    let seen = fake.take();
    assert_eq!(deletes(&seen), [JOB]);
    assert_eq!(applies(&seen), COMMAND_CHILDREN, "{seen:?}");
    assert_eq!(job_generation(&fake), "2");
    assert_eq!(action, Action::requeue(Duration::from_secs(300)));
}

/// A controller restart while the old Job still terminates: the empty record passes Pending and
/// Running again, and the new Job still waits for the old one.
#[tokio::test]
async fn adv_cmdv_u1_a_restart_while_the_old_job_terminates_still_waits() {
    let (client, fake) = fake();
    let ctx = context(client);
    finished_generation_one(&fake, &ctx).await;
    reconciled(2, &ctx).await;
    fake.take();

    let restarted = context(Client::new(
        {
            // The same fake, a new controller: reuse its server through a second client.
            let (service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
            let shared = Arc::clone(&fake.server);
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
                    let (code, answer) = {
                        let mut server = shared.lock().expect("the fake");
                        let answer = server.answer(&parts.method, &path, &query, &body);
                        server.seen.push(Seen {
                            method: parts.method,
                            path,
                            body,
                        });
                        answer
                    };
                    send.send_response(
                        Response::builder()
                            .status(StatusCode::from_u16(code).expect("a status code"))
                            .header("content-type", "application/json")
                            .body(Body::from(serde_json::to_vec(&answer).expect("JSON")))
                            .expect("a response"),
                    );
                }
            });
            service
        },
        "agents",
    ));
    let action = reconciled(2, &restarted).await;
    let seen = fake.take();
    assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    assert!(applies(&seen).is_empty(), "{seen:?}");
    assert_eq!(job_generation(&fake), "1");
}
