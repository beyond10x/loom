use super::{
    Scope, digest, now,
    store::{AgentState, Run, snapshot_lock},
};
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read},
};

pub(super) type Time = DateTime<FixedOffset>;

#[derive(Default)]
pub(super) struct Filter {
    agent: Option<String>,
    from: Option<Time>,
    to: Option<Time>,
}

impl Filter {
    pub fn parse(query: &str) -> Option<Self> {
        let mut f = Self::default();
        let mut keys = BTreeSet::new();
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            if !keys.insert(k.clone()) || v.is_empty() {
                return None;
            }
            match k.as_ref() {
                "agent" => f.agent = Some(v.into_owned()),
                "from" => f.from = Some(v.parse().ok()?),
                "to" => f.to = Some(v.parse().ok()?),
                _ => return None,
            }
        }
        if f.from.zip(f.to).is_some_and(|(a, b)| a >= b) {
            None
        } else {
            Some(f)
        }
    }
    fn includes(&self, r: &Run) -> bool {
        if self.from.is_none() && self.to.is_none() {
            return true;
        }
        let t = r.ended_at.as_deref().and_then(|t| t.parse::<Time>().ok());
        t.is_some_and(|t| self.from.is_none_or(|a| a <= t) && self.to.is_none_or(|b| t < b))
    }
}

fn observed_sum(values: impl Iterator<Item = Option<u64>>) -> (Option<u64>, usize) {
    let mut n = 0;
    let sum = values.flatten().fold(0u64, |a, b| {
        n += 1;
        a.saturating_add(b)
    });
    ((n > 0).then_some(sum), n)
}

fn stats(runs: &[Run], active: usize, f: &Filter) -> Value {
    let mut ms = Vec::new();
    let (mut completed, mut failed, mut rejected) = (0, 0, 0);
    let mut refused = 0;
    let mut budget_stops = 0;
    for r in runs {
        match r.outcome.as_str() {
            "proposed" | "proposed_case" => completed += 1,
            "failed" | "stopped" => failed += 1,
            "declined" | "unclassified" => rejected += 1,
            _ => {}
        }
        refused += r.steps.iter().filter(|s| s["outcome"] == "refused").count();
        if r.stop_reason.as_deref() == Some("budget exhausted") {
            budget_stops += 1;
        }
        if let (Some(a), Some(b)) = (&r.started_at, &r.ended_at)
            && let (Ok(a), Ok(b)) = (a.parse::<Time>(), b.parse::<Time>())
        {
            let duration = (b - a).num_milliseconds();
            if duration >= 0 {
                ms.push(duration);
            }
        }
    }
    ms.sort_unstable();
    let percentile = |p: usize| {
        ms.get((ms.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let (calls, mc) = observed_sum(runs.iter().map(|r| r.model_calls));
    let (input, ic) = observed_sum(runs.iter().map(|r| r.input_tokens));
    let (output, oc) = observed_sum(runs.iter().map(|r| r.output_tokens));
    let (cost, cc) = observed_sum(runs.iter().map(|r| r.cost_micro_usd));
    let throughput = f.from.zip(f.to).and_then(|(a, b)| {
        let interval = (b - a).num_milliseconds();
        (interval > 0).then(|| completed as f64 * 3_600_000.0 / interval as f64)
    });
    json!({"active":active,"total":runs.len(),"completed":completed,"failed":failed,"rejected":rejected,
        "latency_samples":ms.len(),"p50_ms":percentile(50),"p95_ms":percentile(95),"model_calls":calls,
        "input_tokens":input,"output_tokens":output,"cost_micro_usd":cost,"cost":cost.map(|v|v as f64/1_000_000.0),
        "currency":cost.map(|_|"USD"),"throughput_per_hour":throughput,"refused_steps":refused,
        "budget_stops":budget_stops,"coverage":{"model_calls":mc,"input_tokens":ic,"output_tokens":oc,"cost":cc,"total":runs.len()}})
}

/// Read only explicitly registered shards. Scope is applied before file reads, counts, history,
/// fingerprints and filters. An inaccessible requested agent is indistinguishable from a missing one.
pub(super) fn read(
    scope: &Scope,
    viewer: Option<&[String]>,
    f: &Filter,
) -> io::Result<Option<(String, Value)>> {
    let mut agents = vec![];
    let mut runs = vec![];
    let mut active = 0;
    let mut partial = false;
    let timestamp = Utc::now();
    let directory = scope.root.join("agents");
    let gate = snapshot_lock(&directory)?;
    gate.lock()?;
    let mut handles = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if viewer.is_some_and(|v| !v.iter().any(|s| s == id))
            || f.agent.as_deref().is_some_and(|s| s != id)
        {
            continue;
        }
        if !entry.file_type()?.is_file() {
            partial = true;
            continue;
        }
        let history = match File::open(path.with_extension("history")) {
            Ok(f) => Some(f),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        handles.push((id.to_owned(), File::open(&path)?, history));
    }
    drop(gate);
    for (id, file, history) in handles {
        let mut state: AgentState = match serde_json::from_reader(file.take(64 * 1024 * 1024)) {
            Ok(s) => s,
            Err(_) => {
                partial = true;
                continue;
            }
        };
        if let Some(history) = history {
            match serde_json::from_reader(history.take(64 * 1024 * 1024)) {
                Ok(records) => state.history = records,
                Err(_) => {
                    partial = true;
                }
            }
        }
        // If a process died between publishing its history and its live record, terminal
        // evidence takes precedence over the previous in-flight observation of that item.
        for terminal in &state.history {
            state.current.remove(&terminal.id);
        }
        if state.id != id {
            partial = true;
            continue;
        }
        for run in state.current.values_mut().chain(&mut state.history) {
            run.protect_labels();
        }
        let live = state.online
            && state
                .last_seen
                .parse::<Time>()
                .is_ok_and(|seen| (timestamp - seen.to_utc()).num_seconds() < 16);
        let current: Vec<_> = state.current.into_values().filter(|_| live).collect();
        active += current.len();
        let status = if !live {
            "offline"
        } else if current.is_empty() {
            "idle"
        } else {
            "working"
        };
        partial |= state.partial;
        agents.push(json!({"id":state.id,"spec_id":state.spec_id,"name":state.name,"instance":state.instance,
            "kind":state.harness.map(|h| match h {super::Harness::Native=>"agent_spec", _=>"session"}), "harness":state.harness,
            "status":status,"last_seen":state.last_seen,"current":current,"model":state.model.as_deref().and_then(super::routing::identifier),
            "protocol":state.protocol,"a2a_protocol":state.a2a_protocol,"spec_revision":state.spec_revision,"poll_interval_seconds":state.poll_interval_seconds}));
        for run in state.history {
            if (f.from.is_some() || f.to.is_some()) && run.ended_at.is_none() {
                partial = true;
            }
            if f.includes(&run) {
                runs.push(run);
            }
        }
    }
    if f.agent.is_some() && agents.is_empty() {
        return Ok(None);
    }
    agents.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    runs.sort_by(|a, b| b.ended_at.cmp(&a.ended_at).then_with(|| a.id.cmp(&b.id)));
    partial |= runs.iter().any(|run| {
        run.started_at.is_none()
            || run.ended_at.is_none()
            || run.model_calls.is_none()
            || run.input_tokens.is_none()
            || run.output_tokens.is_none()
            || run.cost_micro_usd.is_none()
    });
    let stats = stats(&runs, active, f);
    let mut snapshot =
        json!({"server":scope.id,"agents":agents,"runs":runs,"stats":stats,"partial":partial});
    let cursor = digest(&serde_json::to_vec(&snapshot)?);
    snapshot["time"] = json!(now());
    snapshot["cursor"] = json!(&cursor);
    Ok(Some((cursor, snapshot)))
}
