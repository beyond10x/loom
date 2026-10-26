//! Adversary cases, pass 2, for `story:loom-operator` (wave 2026-10-25-operator, unit u1), against
//! the correction `5ef99bf8`.
//!
//! Each `adv2_*` case drives the controller's own contract (the story's Outcome and Design, design
//! § 7, `src/admission.rs` and `src/reconcile.rs` module documentation) through `reconcile` over a
//! fake API server, kube's mock client over `tower-test`, in-process, as `tests/reconcile.rs` does.
//! Every token below is a clearly fake one. Nothing reaches a cluster or a network.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use b10x_loom_operator::config::Config;
use b10x_loom_operator::reconcile::{Context, reconcile};
use b10x_loom_operator::resource::LoomAgent;
use b10x_loom_operator::status;
use b10x_loom_resource::model::{
    AgentVariant, CheckResult, DeploymentAvailability, DeploymentStatus, LoomAgentState,
    LoomAgents, ResourceName,
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
const UID_A: &str = "00000000-0000-4000-8000-00000000000a";
const UID_B: &str = "00000000-0000-4000-8000-00000000000b";
const UID_C: &str = "00000000-0000-4000-8000-00000000000c";

const STATUS: &str =
    "/apis/loom.beyond10x.dev/v1alpha1/namespaces/agents/loomagents/platform-operator/status";

const CHILDREN: [&str; 5] = [
    "/api/v1/namespaces/agents/serviceaccounts/platform-operator",
    "/api/v1/namespaces/agents/configmaps/platform-operator-spec",
    "/apis/apps/v1/namespaces/agents/deployments/platform-operator",
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator",
    "/api/v1/namespaces/agents/services/platform-operator",
];
const CONFIG_MAP: &str = "/api/v1/namespaces/agents/configmaps/platform-operator-spec";
const NETWORK_POLICY: &str =
    "/apis/networking.k8s.io/v1/namespaces/agents/networkpolicies/platform-operator";
const ACCOUNT: &str = "/api/v1/namespaces/agents/serviceaccounts/operator-account";

#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
    body: Value,
}

/// What a Deployment the fake is applied answers with: `metadata.generation`,
/// `status.observedGeneration` and the `Available` condition's status.
#[derive(Debug, Clone, Copy)]
struct Rollout {
    generation: i64,
    observed: i64,
    available: bool,
}

#[derive(Debug, Default)]
struct Server {
    seen: Vec<Seen>,
    objects: BTreeMap<String, Value>,
    rollout: Option<Rollout>,
    /// Paths whose next DELETE is raced: 404, the object gone meanwhile (the garbage collector
    /// took it), or 409, the object replaced meanwhile by one with another uid and no controller.
    raced: BTreeMap<String, u16>,
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
                if path.contains("/deployments/")
                    && let Some(rollout) = self.rollout
                {
                    object["metadata"]["generation"] = json!(rollout.generation);
                    object["status"] = json!({
                        "observedGeneration": rollout.observed,
                        "conditions": [{
                            "type": "Available",
                            "status": if rollout.available { "True" } else { "False" },
                            "reason": "Fake",
                        }],
                    });
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
                if let Some(code) = self.raced.remove(path) {
                    if code == 404 {
                        self.objects.remove(path);
                    } else {
                        let name = path.rsplit('/').next().unwrap_or_default();
                        self.objects.insert(
                            path.to_owned(),
                            json!({"metadata": {"name": name, "namespace": "agents",
                                                "uid": "uid-of-a-replacement"}}),
                        );
                    }
                    return (
                        StatusCode::from_u16(code).expect("a status code"),
                        json!({
                            "kind": "Status", "apiVersion": "v1", "metadata": {},
                            "status": "Failure",
                            "reason": if code == 404 { "NotFound" } else { "Conflict" },
                            "code": code,
                            "message": "raced",
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

    fn remove(&self, path: &str) {
        self.server.lock().expect("the fake").objects.remove(path);
    }

    fn set_rollout(&self, rollout: Option<Rollout>) {
        self.server.lock().expect("the fake").rollout = rollout;
    }

    fn race_delete(&self, path: &str, code: u16) {
        self.server
            .lock()
            .expect("the fake")
            .raced
            .insert(path.to_owned(), code);
    }

    /// The objects the fake holds, as a person would `kubectl get` them, for a second controller.
    fn objects(&self) -> BTreeMap<String, Value> {
        self.server.lock().expect("the fake").objects.clone()
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

fn named(account: &str) -> String {
    let spec = example().replace(
        "service_account: platform-operator",
        &format!("service_account: {account}"),
    );
    assert!(spec.contains(&format!("service_account: {account}")));
    spec
}

fn with_secret_literal(spec: &str) -> String {
    let refused = spec.replace(
        "http://logs.monitoring.svc:3100",
        "http://ops:not-a-real-password@logs.monitoring.svc:3100",
    );
    assert_ne!(refused, spec);
    refused
}

fn agent(uid: &str, generation: i64, spec_agent: &str, rest: &str) -> LoomAgent {
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
    serde_yaml_ng::from_str(&text).expect("a LoomAgent")
}

/// `rest` after `spec.agent` with the default routes and `toml` as `spec.connectorsConfig`.
fn with_connectors(toml: &str) -> String {
    let mut rest = DEFAULT_ROUTES.to_owned();
    rest.push_str("  connectorsConfig: |\n");
    for line in toml.lines() {
        rest.push_str("    ");
        rest.push_str(line);
        rest.push('\n');
    }
    rest
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

fn published(seen: &[Seen]) -> Option<Value> {
    seen.iter()
        .rev()
        .find(|seen| seen.method == Method::PATCH && seen.path == STATUS)
        .map(|patch| patch.body["status"].clone())
}

/// Every request body that carries `value`, by method and path.
fn carrying<'a>(seen: &'a [Seen], value: &str) -> Vec<(&'a Method, &'a str)> {
    seen.iter()
        .filter(|seen| seen.body.to_string().contains(value))
        .map(|seen| (&seen.method, seen.path.as_str()))
        .collect()
}

/// Design § 7: secrets are "never in the resource, the ConfigMap or the image". The correction
/// (F3) replaced the word walk over the raw `spec.connectorsConfig` text with a walk over the
/// decoded TOML table; a TOML comment does not decode, so it is not walked, while the ConfigMap
/// still receives the raw text, comment and all. The word walk of `90952cc6` found this token.
/// Commenting out an old token line beside its `env:` replacement is the ordinary way it happens.
#[tokio::test]
async fn adv2_a_token_in_a_toml_comment_never_reaches_the_config_map() {
    let (client, fake) = fake();
    let ctx = context(client);
    let toml = "[connections.ops]\nadapter = \"chat\"\n# token = \"xoxb-000-fake\"\ntoken = \"env:CHAT_TOKEN\"\n";
    reconciled(agent(UID_A, 1, &example(), &with_connectors(toml)), &ctx).await;
    let seen = fake.take();
    assert_eq!(
        carrying(&seen, "xoxb-000-fake"),
        Vec::<(&Method, &str)>::new(),
        "the token reached the API server in these requests; status: {:?}",
        published(&seen)
    );
}

/// The same regression for a token that is not the first word of its TOML string (GitHub's
/// `Authorization: token <pat>` form): the credential walk's prefix test reads only the start of a
/// value, and the decoded walk hands it the whole string, where `90952cc6`'s word walk split it on
/// whitespace first. (`Bearer ` is itself a listed prefix, so it is not this case.)
#[tokio::test]
async fn adv2_a_token_after_a_word_in_a_toml_string_never_reaches_the_config_map() {
    let (client, fake) = fake();
    let ctx = context(client);
    let toml = "[connections.ops]\nadapter = \"chat\"\ntoken = \"env:CHAT_TOKEN\"\nheaders = { authorization = \"token ghp_000fake\" }\n";
    reconciled(agent(UID_A, 1, &example(), &with_connectors(toml)), &ctx).await;
    let seen = fake.take();
    assert_eq!(
        carrying(&seen, "ghp_000fake"),
        Vec::<(&Method, &str)>::new(),
        "the token reached the API server in these requests; status: {:?}",
        published(&seen)
    );
}

/// The story: "a Refused resource has no children (existing ones of an earlier generation are
/// deleted)". The Refused arm deletes the ServiceAccount the *refused* generation would name;
/// the one an earlier generation named and the resource still controls is never deleted, when
/// the refused generation names another account, or does not decode at all and falls back to the
/// resource's name.
#[tokio::test]
async fn adv2_a_refused_generation_deletes_the_service_account_an_earlier_generation_named() {
    for (case, refused) in [
        (
            "renamed and refused",
            with_secret_literal(&named("another-account")),
        ),
        (
            "does not decode",
            format!("{}unknown_member: 1\n", named("operator-account")),
        ),
    ] {
        let (client, fake) = fake();
        let ctx = context(client);
        reconciled(
            agent(UID_A, 1, &named("operator-account"), DEFAULT_ROUTES),
            &ctx,
        )
        .await;
        assert!(
            applies(&fake.take()).contains(&ACCOUNT),
            "{case}: generation 1 applies its ServiceAccount"
        );

        reconciled(agent(UID_A, 2, &refused, DEFAULT_ROUTES), &ctx).await;
        let seen = fake.take();
        assert_eq!(
            published(&seen).expect("a status")["state"],
            "Refused",
            "{case}"
        );
        assert!(
            !fake.holds(CHILDREN[2]),
            "{case}: the Deployment is deleted"
        );
        assert!(
            !fake.holds(ACCOUNT),
            "{case}: the ServiceAccount generation 1 named, controlled by this resource, is still \
             there under a Refused status; requests: {:?}",
            seen.iter()
                .map(|seen| (&seen.method, seen.path.as_str()))
                .collect::<Vec<_>>()
        );
    }
}

/// A `ChildNotOwned` refusal is about the cluster, not the spec: the message tells the operator
/// which object to remove. Once it is removed, the same resource at the same generation is
/// refused forever by this controller (the record holds `Refused`, `ApplyLoomAgent` answers
/// `unchanged`, nothing checks again), while a restarted controller over the same cluster runs it
/// (the Design: "after a restart every resource is ... checked again"). The status is not a
/// function of the cluster: it depends on whether the controller restarted.
#[tokio::test]
async fn adv2_a_child_not_owned_refusal_clears_once_the_child_is_removed() {
    let (client, fake) = fake();
    let ctx = context(client);
    fake.put(
        CONFIG_MAP,
        json!({"metadata": {"name": "platform-operator-spec", "namespace": "agents",
                            "uid": "uid-of-a-leftover"}}),
    );
    reconciled(agent(UID_A, 1, &example(), DEFAULT_ROUTES), &ctx).await;
    let status = published(&fake.take()).expect("a status");
    assert_eq!(status["state"], "Refused");
    assert!(
        status["check"]["message"]
            .as_str()
            .expect("a message")
            .contains("ChildNotOwned")
    );

    fake.remove(CONFIG_MAP);
    let mut current = agent(UID_A, 1, &example(), DEFAULT_ROUTES);
    current.status = Some(serde_json::from_value(status).expect("the published status"));

    let (fresh_client, fresh_fake) = self::fake();
    for (path, object) in fake.objects() {
        fresh_fake.put(&path, object);
    }
    let restarted = context(fresh_client);
    reconciled(current.clone(), &restarted).await;
    let after_restart = published(&fresh_fake.take()).expect("a status after a restart");
    assert_eq!(after_restart["state"], "Running", "a restarted controller");

    reconciled(current, &ctx).await;
    let seen = fake.take();
    assert_eq!(
        applies(&seen),
        CHILDREN,
        "the children are applied once the leftover is gone, as after a restart; status now: {:?}",
        ctx.rows()
            .iter()
            .map(|row| (row.state, row.check.clone()))
            .collect::<Vec<_>>()
    );
}

/// Design § 7: "`allowedEgressHosts` in the controller config refuses a host outside the list"
/// (AGENTS.md: "an empty list admitting none"). An egress entry naming neither `host` nor
/// `namespace` decodes, is not checked against the list, and renders an egress rule with no peer:
/// every destination on its port. The case asserts what a NetworkPolicy may open without a peer:
/// only the ports of allowed host entries and the model route.
#[tokio::test]
async fn adv2_an_egress_entry_naming_no_host_opens_no_port_the_allow_list_did_not() {
    let (client, fake) = fake();
    let ctx = context(client);
    let spec = example().replace(
        "      - {name: chat-api, host: chat.example, port: 443}",
        "      - {name: anywhere, port: 6443}\n      - {name: chat-api, host: chat.example, port: 443}",
    );
    assert!(spec.contains("name: anywhere"));
    reconciled(agent(UID_A, 1, &spec, DEFAULT_ROUTES), &ctx).await;
    let seen = fake.take();
    let allowed: BTreeSet<i64> = [443, 8080].into();
    let open: Vec<i64> = seen
        .iter()
        .filter(|seen| seen.method == Method::PATCH && seen.path == NETWORK_POLICY)
        .flat_map(|seen| {
            seen.body["spec"]["egress"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|rule| rule.get("to").is_none_or(Value::is_null))
        .flat_map(|rule| rule["ports"].as_array().cloned().unwrap_or_default())
        .filter_map(|port| port["port"].as_i64())
        .filter(|port| !allowed.contains(port))
        .collect();
    assert_eq!(
        open,
        Vec::<i64>::new(),
        "egress to every destination on a port no allowed host declares; status: {:?}",
        published(&seen)
    );
}

/// `src/reconcile.rs` `availability`: a Deployment's `Available` condition counts only once its
/// status observed the applied generation. No case of the suite sets a Deployment's
/// `metadata.generation` or `status.observedGeneration`, so the guard is unexercised: here a
/// stale `Available: False` (observed 1 of 2) must not degrade a running resource, and the same
/// condition once observed must.
#[tokio::test]
async fn adv2_an_unobserved_rollout_does_not_degrade_and_an_observed_one_does() {
    let (client, fake) = fake();
    let ctx = context(client);
    reconciled(agent(UID_A, 1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();

    fake.set_rollout(Some(Rollout {
        generation: 2,
        observed: 1,
        available: false,
    }));
    reconciled(agent(UID_A, 1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();
    assert_eq!(ctx.rows()[0].state, LoomAgentState::Running);
    assert!(ctx.rows()[0].deployment.is_none());

    fake.set_rollout(Some(Rollout {
        generation: 2,
        observed: 2,
        available: false,
    }));
    reconciled(agent(UID_A, 1, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();
    assert_eq!(ctx.rows()[0].state, LoomAgentState::Degraded);
}

/// The record keyed by uid (F4): a resource replaced while Degraded, and again at once, each time
/// is checked as created, its predecessor's children deleted, and its own applied next.
#[tokio::test]
async fn adv2_a_resource_replaced_while_degraded_and_again_runs_as_the_newest() {
    let (client, fake) = fake();
    let ctx = context(client);
    fake.set_rollout(Some(Rollout {
        generation: 1,
        observed: 1,
        available: false,
    }));
    reconciled(agent(UID_A, 4, &example(), DEFAULT_ROUTES), &ctx).await;
    reconciled(agent(UID_A, 4, &example(), DEFAULT_ROUTES), &ctx).await;
    fake.take();
    assert_eq!(ctx.rows()[0].state, LoomAgentState::Degraded);

    fake.set_rollout(None);
    for uid in [UID_B, UID_C] {
        let action = reconciled(agent(uid, 1, &example(), DEFAULT_ROUTES), &ctx).await;
        assert_eq!(action, Action::requeue(Duration::from_secs(30)), "{uid}");
        let seen = fake.take();
        assert!(applies(&seen).is_empty(), "{uid}: {seen:?}");
        let status = published(&seen).expect("a status");
        assert_eq!(status["observedGeneration"], 1, "{uid}");
        assert_eq!(status["state"], "Running", "{uid}");
        assert!(status.get("deployment").is_none(), "{uid}");
        for path in CHILDREN {
            assert!(!fake.holds(path), "{uid}: {path}");
        }
        reconciled(agent(uid, 1, &example(), DEFAULT_ROUTES), &ctx).await;
        let seen = fake.take();
        assert_eq!(applies(&seen), CHILDREN, "{uid}");
        for apply in seen
            .iter()
            .filter(|seen| seen.method == Method::PATCH && !seen.path.ends_with("/status"))
        {
            assert_eq!(
                apply.body["metadata"]["ownerReferences"][0]["uid"], uid,
                "{uid}"
            );
        }
        assert_eq!(ctx.rows().len(), 1);
    }
}

/// `src/reconcile.rs` `delete_uid`: "one that is gone or was replaced meanwhile is left alone".
/// A predecessor's children are being removed by the garbage collector while the controller
/// deletes them, so a delete answered 404 is ordinary, and one answered 409 (the uid
/// precondition failed: replaced meanwhile) is the precondition doing its work. Neither may fail
/// the reconcile. No case of the suite has the fake answer a DELETE with anything but 200, so
/// neither arm is exercised.
#[tokio::test]
async fn adv2_a_predecessors_child_gone_or_replaced_meanwhile_does_not_fail_the_reconcile() {
    for code in [404, 409] {
        let (client, fake) = fake();
        let ctx = context(client);
        for path in CHILDREN {
            let name = path.rsplit('/').next().expect("a name");
            fake.put(
                path,
                json!({"metadata": {
                    "name": name, "namespace": "agents", "uid": format!("uid-of-{path}"),
                    "ownerReferences": [{
                        "apiVersion": "loom.beyond10x.dev/v1alpha1", "kind": "LoomAgent",
                        "name": NAME, "uid": "uid-of-the-predecessor", "controller": true,
                    }],
                }}),
            );
        }
        fake.race_delete(CHILDREN[2], code);
        let action = reconcile(
            Arc::new(agent(UID_A, 1, &example(), DEFAULT_ROUTES)),
            Arc::clone(&ctx),
        )
        .await
        .unwrap_or_else(|error| panic!("{code}: the reconcile failed: {error}"));
        assert_eq!(action, Action::requeue(Duration::from_secs(30)), "{code}");
        let seen = fake.take();
        assert_eq!(
            seen.iter()
                .filter(|seen| seen.method == Method::DELETE)
                .count(),
            CHILDREN.len(),
            "{code}: every predecessor child is deleted, the raced one included"
        );
    }
}

/// `value` against `schema`, a structural OpenAPI v3 schema as the CRD carries it: `type`,
/// `enum`, `required`, `properties` (a member it does not declare would be pruned), `items`.
fn violations(schema: &Value, value: &Value, at: &str, out: &mut Vec<String>) {
    let kind = schema["type"].as_str().unwrap_or_default();
    let fits = match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        _ => true,
    };
    if !fits {
        out.push(format!("{at}: not {kind}: {value}"));
        return;
    }
    if let Some(allowed) = schema["enum"].as_array()
        && !allowed.contains(value)
    {
        out.push(format!("{at}: {value} outside {allowed:?}"));
    }
    if let Some(object) = value.as_object() {
        for required in schema["required"].as_array().into_iter().flatten() {
            let required = required.as_str().unwrap_or_default();
            if !object.contains_key(required) {
                out.push(format!("{at}: lacks {required}"));
            }
        }
        for (member, inner) in object {
            match schema["properties"].get(member) {
                Some(declared) => violations(declared, inner, &format!("{at}.{member}"), out),
                None => out.push(format!("{at}.{member}: not declared, would be pruned")),
            }
        }
    }
    if let Some(items) = value.as_array() {
        for (index, item) in items.iter().enumerate() {
            violations(&schema["items"], item, &format!("{at}[{index}]"), out);
        }
    }
}

/// The status of every state, Pending and Degraded included, with and without a check and a
/// Deployment read, against the CRD's `status` schema.
#[test]
fn adv2_every_states_status_is_admitted_by_the_crds_status_schema() {
    let crd: Value = serde_yaml_ng::from_str(
        &fs::read_to_string(root().join("deploy/crd/loomagents.loom.beyond10x.dev.yaml"))
            .expect("the CRD"),
    )
    .expect("CRD YAML");
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["status"];
    assert_eq!(schema["type"], "object");
    let checks = [
        None,
        Some(CheckResult {
            exit_code: 0,
            message: "checked".to_owned(),
        }),
        Some(CheckResult {
            exit_code: 3,
            message: "refused: SecretLiteral at spec.agent: a URL".to_owned(),
        }),
    ];
    let reads = [
        None,
        Some(DeploymentAvailability::Available),
        Some(DeploymentAvailability::Unavailable),
    ];
    let mut out = Vec::new();
    for state in [
        LoomAgentState::Pending,
        LoomAgentState::Running,
        LoomAgentState::Degraded,
        LoomAgentState::Refused,
    ] {
        for check in &checks {
            for read in reads {
                for variant in [
                    None,
                    Some(AgentVariant::AgentServe),
                    Some(AgentVariant::Command),
                ] {
                    let row = LoomAgents {
                        name: ResourceName(NAME.to_owned()),
                        observed_generation: 7,
                        check: check.clone(),
                        deployment: read.map(|availability| DeploymentStatus { availability }),
                        variant,
                        conditions: None,
                        state,
                    };
                    let value = serde_json::to_value(status::status(&row)).expect("JSON");
                    violations(schema, &value, &format!("{state:?}"), &mut out);
                }
            }
        }
    }
    assert_eq!(out, Vec::<String>::new());
}
