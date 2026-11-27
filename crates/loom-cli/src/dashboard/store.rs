//! One atomic observation record per state owner. Terminal evidence comes from the owner's
//! RecordLine; the separate observation file adds wall-clock timestamps and model measurements.
use super::{digest, invalid, now, random, save};
use b10x_loom_plugin::{RecordLine, decode_record_line, encode_record_line, observe::TurnObserver};
use loom_sdk::{agent::AgentSpec, commission::model::responsibility::RuntimeEvent};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) const MAX_RUNS: usize = 512;
const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Run {
    pub id: String,
    pub agent_id: String,
    pub item_id: String,
    pub outcome: String,
    pub action: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub steps: Vec<Value>,
    pub steps_left: Option<i64>,
    pub model_calls_left: Option<i64>,
    pub model_calls: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_micro_usd: Option<u64>,
    pub cost: Option<f64>,
    pub events: Vec<Value>,
    pub stop_reason: Option<String>,
}

impl Run {
    fn new(agent: &str, item: &str, started_at: Option<String>) -> Self {
        let id = digest(format!("{agent}\0{item}").as_bytes());
        Self {
            id: id.clone(),
            agent_id: agent.to_owned(),
            item_id: item_label(item, &id),
            outcome: "running".into(),
            action: "classifying".into(),
            started_at,
            ended_at: None,
            steps: vec![],
            steps_left: None,
            model_calls_left: None,
            model_calls: None,
            input_tokens: None,
            output_tokens: None,
            cost_micro_usd: None,
            cost: None,
            events: vec![],
            stop_reason: None,
        }
    }
    /// Reproject older stored labels at the viewer boundary as well as at the producer.
    pub(super) fn protect_labels(&mut self) {
        self.item_id = item_label(&self.item_id, &self.id);
        fn protected(name: &str) -> String {
            match name {
                "unknown" | "classifying" | "selecting" => name.to_owned(),
                _ if name
                    .strip_prefix("action-")
                    .is_some_and(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit())) =>
                {
                    name.to_owned()
                }
                _ => action_label(name),
            }
        }
        self.action = protected(&self.action);
        for record in self.events.iter_mut().chain(&mut self.steps) {
            if let Some(action) = record["action"].as_str() {
                record["action"] = json!(protected(action));
            }
        }
    }

    fn evidence(&mut self, line: &RecordLine) {
        let wire = serde_json::from_str::<Value>(&encode_record_line(line))
            .expect("the owned codec renders JSON");
        self.outcome = wire["outcome"].as_str().unwrap_or("unknown").into();
        // Only allow named actions, discrete outcomes and budgets through. Reasons and
        // proposals in the source record can contain provider output or repository content.
        self.steps=wire["steps"].as_array().into_iter().flatten().take(128).map(|step|json!({
            "action":action_label(step["action"].as_str().unwrap_or("unknown")),
            "outcome":step["outcome"],"steps_left":step["steps_left"],"model_calls_left":step["model_calls_left"]
        })).collect();
        let last = self.steps.last();
        self.action = last
            .and_then(|s| s["action"].as_str())
            .unwrap_or("unknown")
            .to_owned();
        self.steps_left = last.and_then(|s| s["steps_left"].as_i64());
        self.model_calls_left = last.and_then(|s| s["model_calls_left"].as_i64());
        self.stop_reason = wire["detail"]
            .as_str()
            .and_then(|s| {
                if s.contains("awaits approval") {
                    Some("approval required")
                } else if s.contains("budget") {
                    Some("budget exhausted")
                } else {
                    None
                }
            })
            .map(str::to_owned);
    }
}

fn item_label(item: &str, id: &str) -> String {
    let source = if item.starts_with("a2a:") {
        "a2a"
    } else {
        "item"
    };
    format!("{source}:{}", id.chars().take(16).collect::<String>())
}

fn action_label(action: &str) -> String {
    match action {
        b10x_loom_plugin::project::READ
        | b10x_loom_plugin::project::PROPOSE
        | b10x_loom_plugin::project::DECLINE
        | b10x_loom_plugin::project::SEND => action.to_owned(),
        _ => format!("action-{}", &digest(action.as_bytes())[..16]),
    }
}

/// IDs and verbs originate outside the viewer boundary. Unstructured values are shown by digest.
fn label(value: &str) -> String {
    if value.len() <= 160
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.@:".contains(&b))
    {
        value.into()
    } else {
        format!("item-{}", &digest(value.as_bytes())[..16])
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct AgentState {
    pub id: String,
    pub spec_id: String,
    pub spec_revision: Option<String>,
    pub protocol: Option<String>,
    pub a2a_protocol: Option<String>,
    pub poll_interval_seconds: Option<i64>,
    pub name: String,
    pub instance: String,
    pub last_seen: String,
    pub online: bool,
    pub model: Option<String>,
    pub current: BTreeMap<String, Run>,
    #[serde(default, skip_serializing)]
    pub history: VecDeque<Run>,
    #[serde(skip)]
    history_dirty: bool,
    pub partial: bool,
}

impl AgentState {
    fn upsert(&mut self, run: Run) {
        self.history_dirty = true;
        self.history.retain(|r| r.id != run.id);
        if self.history.len() >= MAX_RUNS {
            self.history.pop_front();
            self.partial = true;
        }
        self.history.push_back(run);
    }
}

// A writer holds the shared lock only while publishing one agent. Snapshot readers hold
// the exclusive lock while opening file handles; decoding and rendering happen after release.
pub(super) fn snapshot_lock(agents: &Path) -> io::Result<File> {
    super::private_file(&agents.join("snapshot.lock"))
}

fn publish(path: &Path, data: &mut AgentState) -> io::Result<()> {
    let guard = snapshot_lock(
        path.parent()
            .ok_or_else(|| invalid("missing agents directory"))?,
    )?;
    guard.lock_shared()?;
    if data.history_dirty {
        save(&path.with_extension("history"), &data.history)?;
    }
    save(path, data)?;
    data.history_dirty = false;
    Ok(())
}

pub(super) struct Agent {
    path: PathBuf,
    data: Mutex<AgentState>,
    step_budget: Option<i64>,
    model_budget: Option<i64>,
}

/// Backfill only the last bounded segment of legacy records. Missing timestamps stay missing.
fn backfill(state: &Path, data: &mut AgentState) -> io::Result<()> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(state.join("record.jsonl"))
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(invalid("agent record is not a regular file"));
    }
    let start = meta.len().saturating_sub(MAX_RECORD_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.take(MAX_RECORD_BYTES).read_to_end(&mut buf)?;
    let mut lines = buf.split(|b| *b == b'\n');
    if start > 0 {
        lines.next();
        data.partial = true;
    }
    let observed = std::mem::take(&mut data.history);
    for raw in lines.filter(|raw| !raw.is_empty()) {
        let line = match std::str::from_utf8(raw)
            .ok()
            .and_then(|s| decode_record_line(s).ok())
        {
            Some(v) => v,
            None => {
                data.partial = true;
                continue;
            }
        };
        let mut r = Run::new(&data.id, &line.item.0, None);
        if observed.iter().any(|existing| existing.id == r.id) {
            continue;
        }
        r.evidence(&line);
        data.upsert(r);
    }
    for r in observed {
        data.upsert(r);
    }
    Ok(())
}

impl Agent {
    pub fn register(
        root: &Path,
        state: &Path,
        spec: &AgentSpec,
        revision: &str,
        source: Option<&str>,
        model: Option<&str>,
    ) -> io::Result<Self> {
        let id = digest(state.canonicalize()?.as_os_str().as_encoded_bytes());
        let path = root.join("agents").join(format!("{id}.json"));
        let mut data = match File::open(&path) {
            Ok(f) => serde_json::from_reader(f)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => AgentState {
                id: id.clone(),
                spec_id: String::new(),
                spec_revision: None,
                protocol: None,
                a2a_protocol: None,
                poll_interval_seconds: None,
                name: String::new(),
                instance: String::new(),
                last_seen: now(),
                online: false,
                model: None,
                current: BTreeMap::new(),
                history: VecDeque::new(),
                history_dirty: true,
                partial: false,
            },
            Err(e) => return Err(e),
        };
        match File::open(path.with_extension("history")) {
            Ok(history) => data.history = serde_json::from_reader(history)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        data.history_dirty = true;
        data.partial |= !data.current.is_empty();
        // On a restart discard abandoned in-flight attempts. Terminal history is retained.
        data.current.clear();
        data.spec_id = label(&spec.agent.id.0);
        let prior_revision = data.spec_revision.replace(revision.to_owned());
        data.a2a_protocol = spec
            .expose
            .as_ref()
            .and_then(|e| e.a2a.as_ref())
            .map(|_| b10x_loom_plugin::PROTOCOL.to_owned());
        let polling = spec.channel.as_ref().and(spec.loops.as_ref());
        data.protocol = polling
            .map(|l| l.inner.protocol.clone())
            .or_else(|| data.a2a_protocol.clone());
        data.poll_interval_seconds = polling.map(|l| l.outer.poll_interval_seconds);
        data.name = spec
            .agent
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(120)
            .collect();
        data.instance = random()?;
        data.last_seen = now();
        data.online = true;
        data.model = model.map(str::to_owned).or_else(|| {
            spec.model
                .roles
                .iter()
                .find(|r| r.role == loom_sdk::models::ModelRole::Agent)
                .map(|r| r.model.clone())
        });
        backfill(state, &mut data)?;
        if let Some(text) = source {
            if digest(text.as_bytes()) != revision {
                return Err(invalid("dashboard spec digest mismatch"));
            }
            // Publish immutable bytes before publishing the pointer. A crash cannot expose
            // new contents under the old revision.
            super::atomic_write(
                &path.with_extension(format!("{revision}.yaml")),
                text.as_bytes(),
            )?;
        }
        publish(&path, &mut data)?;
        if let Some(prior) = prior_revision
            && prior != revision
            && is_digest(&prior)
        {
            let retired = path.with_extension(format!("{prior}.yaml"));
            match std::fs::remove_file(retired) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("b10x-loom: obsolete dashboard spec could not be retired: {e}"),
            }
        }
        let inner = spec.loops.as_ref().map(|l| &l.inner);
        Ok(Self {
            path,
            data: Mutex::new(data),
            step_budget: inner.map(|l| l.step_budget),
            model_budget: inner.map(|l| l.model_budget),
        })
    }
    fn lock(&self) -> MutexGuard<'_, AgentState> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn update(&self, update: impl FnOnce(&mut AgentState)) {
        let mut data = self.lock();
        update(&mut data);
        data.last_seen = now();
        if let Err(e) = publish(&self.path, &mut data) {
            data.partial = true;
            eprintln!("b10x-loom: dashboard observation write failed: {e}");
        }
    }
    pub fn heartbeat(&self) {
        self.update(|_| {});
    }
    pub fn offline(&self) {
        self.update(|d| {
            d.online = false;
            d.current.clear();
        });
    }
}

impl TurnObserver for Agent {
    fn started(&self, item: &str) {
        crate::metering::begin(item);
        self.update(|d| {
            let mut r = Run::new(&d.id, item, Some(now()));
            r.events
                .push(json!({"kind":"accepted","time":r.started_at}));
            r.steps_left = self.step_budget;
            r.model_calls_left = self.model_budget;
            d.current.insert(r.id.clone(), r);
        });
    }
    fn event(&self, item: &str, event: &RuntimeEvent) {
        self.update(|d| {
            let id = digest(format!("{}\0{item}", d.id).as_bytes());
            let Some(r) = d.current.get_mut(&id) else {
                return;
            };
            let kind = match event {
                RuntimeEvent::StepStarted(e) => {
                    r.action = "selecting".into();
                    r.steps_left = self.step_budget.map(|b| (b - e.step).max(0));
                    "step_started"
                }
                RuntimeEvent::ActionSelected(e) => {
                    r.action = action_label(&e.action);
                    "action_selected"
                }
                RuntimeEvent::Proposed(e) => {
                    r.action = action_label(&e.action);
                    "proposed"
                }
                RuntimeEvent::AuthorityDecided(_) => "authority_decided",
                RuntimeEvent::EffectInvoked(e) => {
                    r.action = action_label(&e.action);
                    "effect_invoked"
                }
                RuntimeEvent::SessionClosed(_) => "session_closed",
                RuntimeEvent::Stopped(_) => "stopped",
            };
            if r.events.len() < 256 {
                r.events
                    .push(json!({"kind":kind,"time":now(),"action":r.action}));
            }
            // The runtime does not report a remaining model budget on these events.
            r.model_calls_left = None;
        });
    }
    fn finished(&self, item: &str, line: Option<&RecordLine>, failed: bool) {
        let usage = crate::metering::end(item);
        self.update(|d| {
            let id = digest(format!("{}\0{item}", d.id).as_bytes());
            let Some(mut r) = d.current.remove(&id) else {
                return;
            };
            if let Some(line) = line {
                r.evidence(line);
            } else if failed {
                r.outcome = "failed".into();
            } else {
                return;
            }
            r.ended_at = Some(now());
            (
                r.model_calls,
                r.input_tokens,
                r.output_tokens,
                r.cost_micro_usd,
            ) = usage;
            r.cost = r.cost_micro_usd.map(|v| v as f64 / 1_000_000.0);
            r.events
                .push(json!({"kind":"terminal","time":r.ended_at,"outcome":r.outcome}));
            d.upsert(r);
        });
    }
}
