//! Launch only named, operator-configured agent specs. The state lease belongs to agent serve.
//! The request journal makes repeat submissions safe even if the browser lost the first reply.
use super::{Harness, Scope, digest, http, invalid, private_file, save};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs, io,
    net::TcpStream,
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    templates: Vec<Template>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Template {
    id: String,
    spec: PathBuf,
    state: PathBuf,
    catalog: Option<PathBuf>,
    model: Option<String>,
    classifier_model: Option<String>,
    a2a_bearer_file: Option<PathBuf>,
    workspace: Option<PathBuf>,
    #[serde(default)]
    files: Vec<String>,
}

struct Prepared {
    template: Template,
    name: String,
    spec_id: String,
    revision: String,
    protocol: Option<String>,
    harness: &'static str,
    spec: loom_sdk::agent::AgentSpec,
}

impl Prepared {
    fn kind(&self) -> &'static str {
        if self.harness == "native" {
            "agent_spec"
        } else {
            "session"
        }
    }
    fn description(&self) -> Value {
        json!({"id":self.template.id,"name":self.name,"spec_id":self.spec_id,
            "protocol":self.protocol,"kind":self.kind(),"default_harness":self.harness,
            "harnesses":[self.harness]})
    }
}

fn identifier(s: &str) -> bool {
    (1..=96).contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn templates(scope: &Scope) -> io::Result<BTreeMap<String, Prepared>> {
    let text = match http::private_text(&scope.root.join("launch-templates.json")) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        value => value?,
    };
    let catalog: Catalog = serde_json::from_str(&text)?;
    let mut registered = BTreeMap::new();
    for t in catalog.templates {
        let paths = [&t.spec, &t.state]
            .into_iter()
            .chain(t.catalog.iter())
            .chain(t.a2a_bearer_file.iter())
            .chain(t.workspace.iter());
        if !identifier(&t.id)
            || paths.into_iter().any(|p| !p.is_absolute())
            || registered.contains_key(&t.id)
        {
            return Err(invalid("invalid launch template identity or path"));
        }
        if t.catalog.is_some() && (t.model.is_none() || t.classifier_model.is_none()) {
            return Err(invalid("a launch catalog requires both model roles"));
        }
        let (spec, source) =
            crate::agent::read_spec_source(&t.spec).map_err(|_| invalid("invalid launch spec"))?;
        let harness = match (&t.workspace, &spec.session) {
            (None, _) if t.files.is_empty() => "native",
            (Some(_), Some(s)) => match s.kind {
                loom_sdk::session::model::session::SessionKind::ClaudeCode => "claude",
                loom_sdk::session::model::session::SessionKind::Codex => "codex",
                _ => return Err(invalid("unsupported launch session kind")),
            },
            _ => {
                return Err(invalid(
                    "session launch requires a workspace and session spec",
                ));
            }
        };
        let protocol = if harness != "native" {
            Some(loom_sdk::protocols::SESSION_PROPOSAL.to_owned())
        } else if crate::a2a::answers_only(&spec) {
            Some("inbound-answer@1".to_owned())
        } else {
            spec.loops.as_ref().map(|l| l.inner.protocol.clone())
        };
        registered.insert(
            t.id.clone(),
            Prepared {
                template: t,
                name: spec.agent.name.clone(),
                spec_id: spec.agent.id.0.clone(),
                revision: digest(source.as_bytes()),
                protocol,
                harness,
                spec,
            },
        );
    }
    Ok(registered)
}

pub(super) fn list(scope: &Scope, stream: &mut TcpStream) -> io::Result<()> {
    match templates(scope) {
        Ok(all) => http::json_reply(
            stream,
            "200 OK",
            &json!({"templates":all.values().map(Prepared::description).collect::<Vec<_>>()}),
        ),
        Err(_) => http::error(
            stream,
            "503 Service Unavailable",
            "template_configuration_unavailable",
        ),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    template: String,
    request_id: String,
    harness: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    request_id: String,
    template: String,
    harness: String,
    agent_id: String,
    prior_instance: Option<String>,
    #[serde(default)]
    claim: Option<String>,
    result: Option<Value>,
}

#[derive(Deserialize)]
struct Instance {
    instance: String,
    online: bool,
    spec_revision: Option<String>,
    last_seen: chrono::DateTime<chrono::Utc>,
    harness: Option<String>,
    routing: Option<super::routing::Routing>,
    launch_configuration: Option<String>,
    workspace_key: Option<String>,
}

fn instance(scope: &Scope, agent: &str) -> io::Result<Option<Instance>> {
    match http::private_text_bounded(
        &scope.root.join("agents").join(format!("{agent}.json")),
        64 * 1024 * 1024,
    ) {
        Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn active(i: &Instance) -> bool {
    i.online && (chrono::Utc::now() - i.last_seen).num_seconds() < 16
}

// A fresh heartbeat can briefly outlive a crashed process. The serving host holds
// the record file for its whole lifetime, even when it registered with another scope.
fn state_held(state: &Path) -> io::Result<bool> {
    use std::os::unix::fs::OpenOptionsExt;
    let record = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(state.join(b10x_loom_plugin::state::RECORD_FILE))
    {
        Ok(record) => record,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if !record.metadata()?.is_file() {
        return Err(invalid("invalid agent state record"));
    }
    match record.try_lock() {
        Ok(()) => Ok(false),
        Err(fs::TryLockError::WouldBlock) => Ok(true),
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
}

fn generation_held(scope: &Scope, id: &str, expected: &str) -> io::Result<bool> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let path = scope.root.join("agents").join(format!("{id}.lease"));
    let lease = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if !lease.metadata()?.is_file() {
        return Err(invalid("invalid generation lease"));
    }
    match lease.try_lock() {
        Ok(()) => Ok(false),
        Err(fs::TryLockError::WouldBlock) => {
            let mut actual = String::new();
            lease.take(65).read_to_string(&mut actual)?;
            Ok(actual == expected)
        }
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
}

fn durable(path: &Path, data: &impl Serialize) -> io::Result<()> {
    save(path, data)?;
    fs::File::open(path)?.sync_all()?;
    fs::File::open(
        path.parent()
            .ok_or_else(|| invalid("missing journal directory"))?,
    )?
    .sync_all()
}

fn result(p: &Prepared, id: &str, instance: &str, status: &str) -> Value {
    json!({"agent_id":id,"instance":instance,"kind":p.kind(),"harness":p.harness,"status":status})
}

fn systemd_user() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .is_some_and(|d| d.join("systemd/private").exists())
}

// The manager receives only a path and an opaque request nonce; the launch credential
// is read in the intended agent process. Interactive CLI usage retains its existing env var.
const BEARER_FILE: &str = "LOOM_DASHBOARD_AGENT_BEARER_FILE";
const CLAIM_ENV: &str = "LOOM_DASHBOARD_LAUNCH_CLAIM";

pub fn agent_bearer() -> Result<Option<String>, String> {
    if let Some(path) = std::env::var_os(BEARER_FILE) {
        if path.is_empty() {
            return Ok(None);
        }
        read_bearer(Path::new(&path))
            .map(Some)
            .map_err(|_| "configured agent bearer file is unavailable or invalid".to_owned())
    } else {
        Ok(std::env::var(crate::a2a::BEARER_ENV).ok())
    }
}

fn read_bearer(path: &Path) -> io::Result<String> {
    let value = http::private_text_bounded(path, 4098)?;
    let credential = value.trim();
    if credential.is_empty() || credential.len() > 4096 || credential.chars().any(char::is_control)
    {
        return Err(invalid("invalid configured agent bearer"));
    }
    Ok(credential.to_owned())
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Claim {
    agent_id: String,
    revision: String,
    harness: String,
    pub(super) configuration: String,
    instance: Option<String>,
}

fn claim_path(scope: &Scope, nonce: &str) -> io::Result<PathBuf> {
    if !super::store::is_digest(nonce) {
        return Err(invalid("invalid launch claim"));
    }
    Ok(scope.root.join(format!("launch-{nonce}.json")))
}

impl Claim {
    fn read(path: &Path) -> io::Result<Self> {
        Ok(serde_json::from_str(&http::private_text(path)?)?)
    }
}

/// Check a spawned child's original intent under the startup and state leases, before it
/// can register. Even a delayed duplicate cannot start after the first instance has ended.
pub(super) fn registration(
    scope: &Scope,
    state: &Path,
    revision: &str,
    harness: Harness,
) -> io::Result<Option<(PathBuf, Claim)>> {
    let Some(nonce) = std::env::var_os(CLAIM_ENV) else {
        return Ok(None);
    };
    let path = claim_path(scope, nonce.to_str().unwrap_or(""))?;
    let claim = Claim::read(&path)?;
    let agent_id = digest(state.canonicalize()?.as_os_str().as_encoded_bytes());
    let kind = match harness {
        Harness::Native => "native",
        Harness::Claude => "claude",
        Harness::Codex => "codex",
    };
    if claim.instance.is_some()
        || claim.agent_id != agent_id
        || claim.revision != revision
        || claim.harness != kind
    {
        return Err(invalid("launch claim does not match this agent"));
    }
    Ok(Some((path, claim)))
}

pub(super) fn registered(pending: Option<(PathBuf, Claim)>, instance: String) -> io::Result<()> {
    if let Some((path, mut claim)) = pending {
        claim.instance = Some(instance);
        durable(&path, &claim)?;
    }
    Ok(())
}

fn spawn(scope: &Scope, p: &Prepared, id: &str, nonce: &str) -> io::Result<Child> {
    let t = &p.template;
    let exe = std::env::current_exe()?;
    let managed = systemd_user();
    // --wait keeps a local child handle for early startup failure. The transient service
    // belongs to the user manager and survives its dashboard listener's exit.
    let mut cmd = if managed {
        let mut c = Command::new("systemd-run");
        c.args([
            "--user",
            "--collect",
            "--quiet",
            "--wait",
            "--service-type=exec",
        ])
        .arg(format!(
            "--unit=loom-dispatch-{}-{}",
            &scope.id[..12],
            &id[..16]
        ))
        .arg("--setenv=LOOM_DASHBOARD_DIR")
        .arg(format!("--setenv={CLAIM_ENV}"))
        .arg(format!("--setenv={BEARER_FILE}"))
        .arg("--")
        .arg(exe);
        c
    } else {
        let mut c = Command::new(exe);
        // A new session releases the controlling terminal in addition to the process group.
        // SAFETY: setsid(2) is async-signal-safe and the child is not a process-group leader.
        unsafe {
            c.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        c
    };
    cmd.env("LOOM_DASHBOARD_DIR", &scope.root)
        .env(CLAIM_ENV, nonce)
        .env(
            BEARER_FILE,
            t.a2a_bearer_file
                .as_deref()
                .unwrap_or_else(|| Path::new("")),
        )
        .env_remove("LOOM_A2A_BEARER")
        .env_remove("LOOM_DASHBOARD_ADDR");
    cmd.args(["agent", "serve", "--spec"])
        .arg(&t.spec)
        .arg("--state")
        .arg(&t.state)
        .args(["--a2a-address", "127.0.0.1"]);
    if let Some(value) = &t.catalog {
        cmd.arg("--catalog").arg(value);
    }
    if let Some(value) = &t.model {
        cmd.arg("--model").arg(value);
    }
    if let Some(value) = &t.classifier_model {
        cmd.arg("--classifier-model").arg(value);
    }
    if let Some(value) = &t.workspace {
        cmd.arg("--session-workspace").arg(value);
        for file in &t.files {
            cmd.arg("--session-file").arg(file);
        }
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
}

struct Reap(Option<Child>);
impl Drop for Reap {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take()
            && !matches!(child.try_wait(), Ok(Some(_)))
        {
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

type Reply = (&'static str, Value);
fn fault(status: &'static str, code: &str) -> Reply {
    (status, json!({"error":code}))
}

fn configuration(p: &Prepared) -> io::Result<String> {
    let t = &p.template;
    let credential = t.a2a_bearer_file.as_deref().map(read_bearer).transpose()?;
    // Operator-only identity of execution settings. No path, input or bearer is serialized to a receipt.
    Ok(digest(&serde_json::to_vec(&(
        &p.revision,
        p.harness,
        &t.catalog,
        &t.model,
        &t.classifier_model,
        &t.workspace,
        &t.files,
        credential.map(|s| digest(s.as_bytes())),
    ))?))
}

pub(super) fn workspace_key(workspace: &Path, files: &[String]) -> io::Result<String> {
    let path = workspace.canonicalize()?;
    let mut permitted: Vec<_> = files.iter().collect();
    permitted.sort_unstable();
    permitted.dedup();
    Ok(digest(&serde_json::to_vec(&(path, permitted))?))
}

fn compatible(
    scope: &Scope,
    id: &str,
    p: &Prepared,
    i: &Instance,
    configuration: &str,
) -> io::Result<bool> {
    if i.spec_revision.as_deref() != Some(&p.revision) || i.harness.as_deref() != Some(p.harness) {
        return Ok(false);
    }
    if let Some(actual) = &i.launch_configuration {
        if actual != configuration {
            return Ok(false);
        }
    } else if let Some(file) = p.template.a2a_bearer_file.as_deref()
        && !super::dispatch::authenticates(scope, id, &i.instance, &read_bearer(file)?)?
    {
        return Ok(false);
    }
    if let Some(workspace) = p.template.workspace.as_deref() {
        return Ok(i.workspace_key.as_deref()
            == Some(workspace_key(workspace, &p.template.files)?.as_str()));
    }
    let t = &p.template;
    let (_, _, expected) = crate::agent::served_models(
        &p.spec,
        &t.spec,
        t.catalog.as_deref(),
        t.model.as_deref(),
        t.classifier_model.as_deref(),
    )
    .map_err(|_| invalid("launch model configuration unavailable"))?;
    Ok(i.routing.as_ref() == Some(&expected))
}

fn remember(scope: &Scope, receipts: &mut VecDeque<Receipt>, receipt: Receipt) -> io::Result<()> {
    receipts.retain(|r| r.request_id != receipt.request_id);
    receipts.push_back(receipt);
    let retired = if receipts.len() > 512 {
        receipts.pop_front()
    } else {
        None
    };
    durable(&scope.root.join("launch-requests.json"), receipts)?;
    if let Some(nonce) = retired.and_then(|entry| entry.claim) {
        let _ = fs::remove_file(claim_path(scope, &nonce)?);
    }
    Ok(())
}

fn launch(scope: &Scope, request: Request) -> io::Result<Reply> {
    let transaction = private_file(&scope.root.join("launch.lock"))?;
    let lock_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match transaction.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) if Instant::now() < lock_deadline => {
                thread::sleep(Duration::from_millis(25))
            }
            Err(_) => return Err(invalid("launch transaction unavailable")),
        }
    }
    let journal = scope.root.join("launch-requests.json");
    let mut receipts: VecDeque<Receipt> =
        match http::private_text_bounded(&journal, 4 * 1024 * 1024) {
            Ok(text) => serde_json::from_str(&text)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => VecDeque::new(),
            Err(e) => return Err(e),
        };
    let old = receipts
        .iter()
        .find(|r| r.request_id == request.request_id)
        .cloned();
    if let Some(r) = &old {
        if r.template != request.template
            || request.harness.as_ref().is_some_and(|h| h != &r.harness)
        {
            return Ok(fault("409 Conflict", "request_conflict"));
        }
        if let Some(reply) = &r.result {
            return Ok(("200 OK", reply.clone()));
        }
    }
    let old_claim = old
        .as_ref()
        .and_then(|r| r.claim.as_deref())
        .map(|nonce| Claim::read(&claim_path(scope, nonce)?))
        .transpose()?;
    // Recover a successful child even if it stopped before its initiating host could reply or
    // persist a final receipt. The operator might even have withdrawn its template since then.
    if let Some(c) = &old_claim
        && let Some(owner) = c.instance.as_ref()
    {
        let reply = json!({"agent_id":c.agent_id,"instance":owner,"kind":if c.harness=="native" {"agent_spec"} else {"session"},"harness":c.harness,"status":"started"});
        let mut receipt = old.expect("a claim requires its receipt");
        receipt.result = Some(reply.clone());
        remember(scope, &mut receipts, receipt)?;
        return Ok(("200 OK", reply));
    }
    let Ok(mut all) = templates(scope) else {
        return Ok(fault(
            "503 Service Unavailable",
            "template_configuration_unavailable",
        ));
    };
    let Some(p) = all.remove(&request.template) else {
        return Ok(fault("404 Not Found", "template_not_found"));
    };
    if request.harness.as_deref().is_some_and(|h| h != p.harness) {
        return Ok(fault("400 Bad Request", "invalid_request"));
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&p.template.state)?;
    let id = digest(
        p.template
            .state
            .canonicalize()?
            .as_os_str()
            .as_encoded_bytes(),
    );
    let key = configuration(&p)?;
    if old
        .as_ref()
        .is_some_and(|r| r.agent_id != id || r.harness != p.harness)
        || old_claim
            .as_ref()
            .is_some_and(|c| c.configuration != key || c.revision != p.revision)
    {
        return Ok(fault("409 Conflict", "request_conflict"));
    }
    let prior = instance(scope, &id)?;
    let held = state_held(&p.template.state)?;
    if held {
        let Some(active) = prior.as_ref().filter(|i| active(i)) else {
            return Ok(fault("409 Conflict", "state_conflict"));
        };
        if !generation_held(scope, &id, &active.instance)?
            || !compatible(scope, &id, &p, active, &key)?
        {
            return Ok(fault("409 Conflict", "state_conflict"));
        }
        // A new request may reuse a compatible existing host. A pending request is attributed
        // only to a specific attested child, never to an unrelated successor of that state.
        if old.is_some() {
            return Ok(fault("409 Conflict", "state_conflict"));
        }
        let reply = result(&p, &id, &active.instance, "already_running");
        let receipt = Receipt {
            request_id: request.request_id,
            template: request.template,
            harness: p.harness.into(),
            agent_id: id,
            prior_instance: Some(active.instance.clone()),
            claim: None,
            result: Some(reply.clone()),
        };
        remember(scope, &mut receipts, receipt)?;
        return Ok(("200 OK", reply));
    }
    if p.spec.expose.as_ref().is_some_and(|e| e.a2a.is_some())
        && p.template.a2a_bearer_file.is_none()
    {
        return Err(invalid("launch requires a configured A2A bearer"));
    }
    let (mut receipt, nonce) = match old {
        Some(r) => match r.claim.clone() {
            Some(nonce) => (r, nonce),
            // A legacy intent lacks any child attestation. Its execution is indeterminate.
            None => return Ok(fault("409 Conflict", "request_conflict")),
        },
        None => {
            let nonce = super::random()?;
            let receipt = Receipt {
                request_id: request.request_id,
                template: request.template,
                harness: p.harness.into(),
                agent_id: id.clone(),
                prior_instance: prior.map(|i| i.instance),
                claim: Some(nonce.clone()),
                result: None,
            };
            let claim = Claim {
                agent_id: id.clone(),
                revision: p.revision.clone(),
                harness: p.harness.into(),
                configuration: key,
                instance: None,
            };
            let path = claim_path(scope, &nonce)?;
            durable(&path, &claim)?;
            remember(scope, &mut receipts, receipt.clone())?;
            (receipt, nonce)
        }
    };
    let path = claim_path(scope, &nonce)?;
    let mut child = Reap(Some(spawn(scope, &p, &id, &nonce)?));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let claim = Claim::read(&path)?;
        if let Some(owner) = claim.instance {
            let reply = result(&p, &id, &owner, "started");
            receipt.result = Some(reply.clone());
            remember(scope, &mut receipts, receipt)?;
            return Ok((
                if old_claim.is_none() {
                    "201 Created"
                } else {
                    "200 OK"
                },
                reply,
            ));
        }
        if child
            .0
            .as_mut()
            .expect("spawn returned child")
            .try_wait()?
            .is_some()
            || Instant::now() >= deadline
        {
            return Ok(fault("503 Service Unavailable", "launch_unavailable"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn handle(scope: &Scope, bytes: &[u8], stream: &mut TcpStream) -> io::Result<()> {
    let parsed = serde_json::from_slice::<Request>(bytes);
    let (status, reply) = match parsed {
        Ok(req) if identifier(&req.template) && identifier(&req.request_id) => launch(scope, req)
            .unwrap_or_else(|_| fault("503 Service Unavailable", "launch_unavailable")),
        _ => fault("400 Bad Request", "invalid_request"),
    };
    http::json_reply(stream, status, &reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scope(root: &Path) -> Scope {
        Scope {
            root: root.to_owned(),
            addr: "127.0.0.1:1".parse().unwrap(),
            token_hash: String::new(),
            id: digest(b"scope"),
        }
    }

    #[test]
    fn a_registered_pending_receipt_recovers_after_the_agent_and_template_are_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let scope = scope(tmp.path());
        let id = digest(b"state");
        let nonce = digest(b"pending launch");
        let started = digest(b"original instance");
        let ticket = Receipt {
            request_id: "retry".into(),
            template: "review".into(),
            harness: "native".into(),
            agent_id: id.clone(),
            prior_instance: None,
            claim: Some(nonce.clone()),
            result: None,
        };
        let claim = Claim {
            agent_id: id.clone(),
            harness: "native".into(),
            revision: digest(b"spec"),
            configuration: digest(b"settings"),
            instance: None,
        };
        let path = claim_path(&scope, &nonce).unwrap();
        durable(&path, &claim).unwrap();
        remember(&scope, &mut VecDeque::new(), ticket).unwrap();
        registered(Some((path, claim)), started.clone()).unwrap();
        for _ in 0..2 {
            let (status, receipt) = launch(
                &scope,
                Request {
                    template: "review".into(),
                    request_id: "retry".into(),
                    harness: None,
                },
            )
            .unwrap();
            assert_eq!(status, "200 OK");
            assert_eq!(receipt["instance"], started);
            assert_eq!(receipt["agent_id"], id);
            assert_eq!(receipt["status"], "started");
        }
        let (status, _) = launch(
            &scope,
            Request {
                template: "other".into(),
                request_id: "retry".into(),
                harness: None,
            },
        )
        .unwrap();
        assert_eq!(status, "409 Conflict");
    }

    #[test]
    fn credentials_are_bounded_and_a_different_scope_cannot_reuse_a_recent_heartbeat() {
        let tmp = tempfile::tempdir().unwrap();
        let scope = scope(tmp.path());
        fs::create_dir(tmp.path().join("agents")).unwrap();
        let id = digest(b"state");
        let generation = digest(b"actual instance");
        let lease_path = scope.root.join("agents").join(format!("{id}.lease"));
        let mut lease = private_file(&lease_path).unwrap();
        lease.lock().unwrap();
        lease.write_all(generation.as_bytes()).unwrap();
        assert!(generation_held(&scope, &id, &generation).unwrap());
        assert!(!generation_held(&scope, &id, &digest(b"previous")).unwrap());
        drop(lease);
        assert!(!generation_held(&scope, &id, &generation).unwrap());
        let bearer = tmp.path().join("agent-token");
        super::super::atomic_write(&bearer, b"fixture\n").unwrap();
        assert_eq!(read_bearer(&bearer).unwrap(), "fixture");
        for rejected in [vec![b'x'; 4097], b"one\ntwo".to_vec(), Vec::new()] {
            super::super::atomic_write(&bearer, &rejected).unwrap();
            assert!(read_bearer(&bearer).is_err());
        }
    }
}
