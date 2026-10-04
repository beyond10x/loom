//! Adversary pass 2 on `story:run-outcomes`: the conformance suite `ess verify conform synthesize`
//! derives from `ess/` for the Run commands, run by hand against the generated behaviours over
//! [`RunStore`].
//!
//! Nothing else executes those scenarios against this crate: the gate only synthesizes them. This
//! runner interprets every step kind the suite holds and fails on one it does not know, so a new
//! step kind cannot pass silently. A wrong-state refusal must also carry the state the run was in
//! when the command arrived.
//!
//! Source paths are read when the test runs (`CARGO_MANIFEST_DIR`), never baked in at build time.

use b10x_commission::model::behaviour::{Context, Generated, RunStorage};
use b10x_commission::model::json::{self, Value};
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::obligations::{
    ResumeRunBehavior, RunStatesQuery, StartRunBehavior, SuspendRunBehavior,
};
use b10x_commission::model::responsibility::{
    CommissionId, ResumeRun, ResumeRunOutcome, RunId, RunSnapshot, RunState, StartRun,
    StartRunOutcome, SuspendRun, SuspendRunOutcome, SuspensionReason,
};
use b10x_commission::outcome::RunStore;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

const NS: &str = "commission.responsibility.";

fn root() -> PathBuf {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is unset: run this test through cargo"));
    let root = PathBuf::from(manifest).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// The suite, synthesized from this tree's `ess/` now.
fn suite() -> Value {
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("adversary2_run_conformance");
    std::fs::create_dir_all(&out_dir).expect("create suite dir");
    let out = out_dir.join("suite.json");
    let run = Command::new("ess")
        .args(["verify", "conform", "synthesize", "--path"])
        .arg(root().join("ess"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap_or_else(|error| panic!("run `ess verify conform synthesize`: {error}"));
    assert!(
        run.status.success(),
        "`ess verify conform synthesize` failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(&out).expect("read suite");
    json::parse(&text).unwrap_or_else(|error| panic!("suite is not JSON: {error:?}"))
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn state_name(state: RunState) -> &'static str {
    match state {
        RunState::Running => "Running",
        RunState::Suspended => "Suspended",
    }
}

fn reason_value(reason: &SuspensionReason) -> Value {
    match reason {
        SuspensionReason::Authority(value) => {
            object(vec![("kind", text("Authority")), ("value", value.clone())])
        }
        other => {
            panic!("this runner encodes only the Authority reason the suite uses, not {other:?}")
        }
    }
}

fn reason_from(value: &Value) -> SuspensionReason {
    match value.member("kind") {
        Some(Value::Text(kind)) if kind == "Authority" => {
            SuspensionReason::Authority(value.member("value").cloned().unwrap_or(Value::Null))
        }
        other => panic!("this runner decodes only the Authority reason, not {other:?}"),
    }
}

/// JSON equality where numbers compare by value: the suite spells the integer 1 as `1.0`.
fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            a.parse::<f64>().ok() == b.parse::<f64>().ok() && a.parse::<f64>().is_ok()
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(name, value)| {
                    b.iter()
                        .any(|(other, theirs)| other == name && same(value, theirs))
                })
        }
        _ => left == right,
    }
}

fn is_uuid(value: &Value) -> bool {
    let Value::Text(text) = value else {
        return false;
    };
    let groups: Vec<&str> = text.split('-').collect();
    groups.iter().map(|group| group.len()).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
        && groups
            .iter()
            .all(|group| group.chars().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Debug, Clone)]
struct Event {
    name: String,
    payload: Value,
}

#[derive(Debug)]
struct Executed {
    command: String,
    outcome: String,
    events: Vec<Event>,
    /// The error's type, and the state it carries where it carries one.
    error: Option<(String, Option<RunState>)>,
    /// The state the run named by the input was in before the command, if it was stored.
    before: Option<RunState>,
}

struct Runner<P> {
    runs: Generated<P>,
    instances: BTreeMap<String, Value>,
    history: Vec<Event>,
    last: Option<Executed>,
    rows: Vec<Value>,
    subject: Option<(Value, Option<Value>)>,
}

impl<P: RunStorage + Context> Runner<P> {
    fn new(ports: P) -> Self {
        Self {
            runs: Generated::new(ports),
            instances: BTreeMap::new(),
            history: Vec::new(),
            last: None,
            rows: Vec::new(),
            subject: None,
        }
    }

    fn resolve(&self, source: &Value) -> Result<Value, String> {
        match source.member("kind") {
            Some(Value::Text(kind)) if kind == "literal" => source
                .member("value")
                .cloned()
                .ok_or_else(|| "literal without value".to_owned()),
            Some(Value::Text(kind)) if kind == "instance" => {
                let Some(Value::Text(name)) = source.member("instance") else {
                    return Err("instance without a name".to_owned());
                };
                self.instances
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("instance `{name}` was never captured"))
            }
            Some(Value::Text(kind)) if kind == "observed" => {
                let (Some(Value::Text(event)), Some(Value::Text(field))) =
                    (source.member("event"), source.member("field"))
                else {
                    return Err("observed without event and field".to_owned());
                };
                self.history
                    .iter()
                    .rev()
                    .find(|seen| &seen.name == event)
                    .and_then(|seen| seen.payload.member(field).cloned())
                    .ok_or_else(|| format!("no `{event}` with `{field}` was observed"))
            }
            other => Err(format!("unknown value source {other:?}")),
        }
    }

    fn input(&self, input: &Value, field: &str) -> Result<Value, String> {
        self.resolve(
            input
                .member(field)
                .ok_or_else(|| format!("input has no `{field}`"))?,
        )
    }

    fn run_id(&self, input: &Value) -> Result<RunId, String> {
        match self.input(input, "run_id")? {
            Value::Text(id) => Ok(RunId(Uuid(id))),
            other => Err(format!("run_id is {other:?}")),
        }
    }

    fn execute(&mut self, command: &str, input: &Value) -> Result<(), String> {
        let short = command.strip_prefix(NS).unwrap_or(command);
        let executed = match short {
            "StartRun" => {
                let Value::Text(commission) = self.input(input, "commission_id")? else {
                    return Err("commission_id is not a string".to_owned());
                };
                let revision = match self.input(input, "case_revision")? {
                    Value::Number(number) => number
                        .parse::<f64>()
                        .map_err(|error| format!("case_revision {number}: {error}"))?
                        as i64,
                    other => return Err(format!("case_revision is {other:?}")),
                };
                let StartRunOutcome::Started { run_started } = self
                    .runs
                    .start_run(StartRun {
                        commission_id: CommissionId(Uuid(commission)),
                        case_revision: revision,
                    })
                    .map_err(|unmet| format!("StartRun unmet: {unmet}"))?;
                Executed {
                    command: command.to_owned(),
                    outcome: "started".to_owned(),
                    events: vec![Event {
                        name: format!("{NS}RunStarted"),
                        payload: object(vec![
                            ("run_id", text(&run_started.run_id.0.0)),
                            ("commission_id", text(&run_started.commission_id.0.0)),
                            (
                                "case_revision",
                                Value::Number(run_started.case_revision.to_string()),
                            ),
                        ]),
                    }],
                    error: None,
                    before: None,
                }
            }
            "SuspendRun" => {
                let run_id = self.run_id(input)?;
                let reason = reason_from(&self.input(input, "reason")?);
                let before = RunStorage::get(&self.runs.ports, &run_id).map(|held| held.state);
                let outcome = self
                    .runs
                    .suspend_run(SuspendRun { run_id, reason })
                    .map_err(|unmet| format!("SuspendRun unmet: {unmet}"))?;
                let (outcome, events, error) = match outcome {
                    SuspendRunOutcome::Suspended { run_suspended } => (
                        "suspended",
                        vec![Event {
                            name: format!("{NS}RunSuspended"),
                            payload: object(vec![
                                ("run_id", text(&run_suspended.run_id.0.0)),
                                ("reason", reason_value(&run_suspended.reason)),
                            ]),
                        }],
                        None,
                    ),
                    SuspendRunOutcome::WrongState { error } => (
                        "wrong-state",
                        Vec::new(),
                        Some((format!("{NS}RunStateConflict"), Some(error.state))),
                    ),
                    SuspendRunOutcome::WrongStateUnknownInstance => (
                        "wrong-state",
                        Vec::new(),
                        Some((format!("{NS}RunStateConflict"), None)),
                    ),
                };
                Executed {
                    command: command.to_owned(),
                    outcome: outcome.to_owned(),
                    events,
                    error,
                    before,
                }
            }
            "ResumeRun" => {
                let run_id = self.run_id(input)?;
                let before = RunStorage::get(&self.runs.ports, &run_id).map(|held| held.state);
                let outcome = self
                    .runs
                    .resume_run(ResumeRun { run_id })
                    .map_err(|unmet| format!("ResumeRun unmet: {unmet}"))?;
                let (outcome, events, error) = match outcome {
                    ResumeRunOutcome::Resumed { run_resumed } => (
                        "resumed",
                        vec![Event {
                            name: format!("{NS}RunResumed"),
                            payload: object(vec![("run_id", text(&run_resumed.run_id.0.0))]),
                        }],
                        None,
                    ),
                    ResumeRunOutcome::WrongState { error } => (
                        "wrong-state",
                        Vec::new(),
                        Some((format!("{NS}RunStateConflict"), Some(error.state))),
                    ),
                    ResumeRunOutcome::WrongStateUnknownInstance => (
                        "wrong-state",
                        Vec::new(),
                        Some((format!("{NS}RunStateConflict"), None)),
                    ),
                };
                Executed {
                    command: command.to_owned(),
                    outcome: outcome.to_owned(),
                    events,
                    error,
                    before,
                }
            }
            other => return Err(format!("unknown command `{other}`")),
        };
        self.history.extend(executed.events.iter().cloned());
        self.last = Some(executed);
        Ok(())
    }

    fn last(&self) -> Result<&Executed, String> {
        self.last
            .as_ref()
            .ok_or_else(|| "no command was executed".to_owned())
    }

    fn row_matches(&self, row: &Value, fields: &Value) -> Result<bool, String> {
        let Value::Object(fields) = fields else {
            return Err("fields is not an object".to_owned());
        };
        for (name, source) in fields {
            let expected = self.resolve(source)?;
            match row.member(name) {
                Some(actual) if same(actual, &expected) => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn query(&mut self) -> Result<(), String> {
        let rows = self
            .runs
            .run_states()
            .map_err(|unmet| format!("RunStates unmet: {unmet}"))?;
        self.rows = rows
            .into_iter()
            .map(|row| {
                object(vec![
                    ("run_id", text(&row.run_id.0.0)),
                    ("commission_id", text(&row.commission_id.0.0)),
                    (
                        "case_revision",
                        Value::Number(row.case_revision.to_string()),
                    ),
                    ("state", text(state_name(row.state))),
                ])
            })
            .collect();
        Ok(())
    }

    fn subject_row(&self, subject: &Value) -> Result<Option<Value>, String> {
        for row in &self.rows {
            if self.row_matches(row, subject)? {
                return Ok(Some(row.clone()));
            }
        }
        Ok(None)
    }

    fn step(&mut self, step: &Value) -> Result<(), String> {
        let Some(Value::Text(kind)) = step.member("step") else {
            return Err("step without a kind".to_owned());
        };
        let name = |field: &str| match step.member(field) {
            Some(Value::Text(value)) => Ok(value.clone()),
            other => Err(format!("`{field}` is {other:?}")),
        };
        match kind.as_str() {
            "execute_command" => {
                let command = name("command")?;
                let input = step.member("input").cloned().unwrap_or(Value::Null);
                self.execute(&command, &input)
            }
            "expect_outcome" => {
                let outcome = step.member("outcome").ok_or("no outcome")?;
                let (Some(Value::Text(command)), Some(Value::Text(expected))) =
                    (outcome.member("command"), outcome.member("outcome"))
                else {
                    return Err("outcome without command and name".to_owned());
                };
                let last = self.last()?;
                if &last.command == command && &last.outcome == expected {
                    Ok(())
                } else {
                    Err(format!(
                        "expected {command}/{expected}, got {}/{}",
                        last.command, last.outcome
                    ))
                }
            }
            "capture_instance" => {
                let instance = name("instance")?;
                let event = name("event")?;
                let field = name("field")?;
                let value = self
                    .last()?
                    .events
                    .iter()
                    .find(|seen| seen.name == event)
                    .and_then(|seen| seen.payload.member(&field).cloned())
                    .ok_or_else(|| format!("no `{event}`.`{field}` to capture"))?;
                self.instances.insert(instance, value);
                Ok(())
            }
            "expect_event" => {
                let event = name("event")?;
                let last = self.last()?;
                let seen = last
                    .events
                    .iter()
                    .find(|seen| seen.name == event)
                    .ok_or_else(|| format!("`{event}` was not emitted: {:?}", last.events))?;
                if let Some(Value::Object(payload)) = step.member("payload") {
                    for (field, expected) in payload {
                        match seen.payload.member(field) {
                            Some(actual) if same(actual, expected) => {}
                            actual => {
                                return Err(format!(
                                    "`{event}`.`{field}` is {actual:?}, expected {expected:?}"
                                ));
                            }
                        }
                    }
                }
                if let Some(Value::Object(shape)) = step.member("shape") {
                    for (field, expected) in shape {
                        let actual = seen
                            .payload
                            .member(field)
                            .ok_or_else(|| format!("`{event}` has no `{field}`"))?;
                        if matches!(expected.member("kind"), Some(Value::Text(k)) if k == "uuid")
                            && !is_uuid(actual)
                        {
                            return Err(format!("`{event}`.`{field}` is not a uuid: {actual:?}"));
                        }
                    }
                }
                Ok(())
            }
            "expect_no_event" => {
                let event = name("event")?;
                if self.last()?.events.iter().any(|seen| seen.name == event) {
                    Err(format!("`{event}` was emitted"))
                } else {
                    Ok(())
                }
            }
            "expect_no_events" => {
                let last = self.last()?;
                if last.events.is_empty() {
                    Ok(())
                } else {
                    Err(format!("events were emitted: {:?}", last.events))
                }
            }
            "expect_error" => {
                let error = name("error")?;
                let last = self.last()?;
                match &last.error {
                    Some((name, state)) if *name == error => {
                        if *state != last.before {
                            Err(format!(
                                "`{error}` carries {state:?}, but the run was in {:?}",
                                last.before
                            ))
                        } else {
                            Ok(())
                        }
                    }
                    other => Err(format!("expected error `{error}`, got {other:?}")),
                }
            }
            "query_view" => self.query(),
            "expect_view" => {
                let expectation = step.member("expectation").ok_or("no expectation")?;
                match expectation.member("expect") {
                    Some(Value::Text(mode)) if mode == "contains" => {}
                    other => return Err(format!("unknown view expectation {other:?}")),
                }
                let fields = expectation.member("fields").ok_or("no fields")?;
                for row in &self.rows {
                    if self.row_matches(row, fields)? {
                        return Ok(());
                    }
                }
                Err(format!("no row matches {fields:?} in {:?}", self.rows))
            }
            "snapshot_complete_subject" => {
                let subject = step.member("subject").ok_or("no subject")?.clone();
                let row = self.subject_row(&subject)?;
                if row.is_none() {
                    return Err(format!("no row for subject {subject:?}"));
                }
                self.subject = Some((subject, row));
                Ok(())
            }
            "expect_complete_subject_unchanged" => {
                let (subject, before) = self.subject.clone().ok_or("no snapshot was taken")?;
                let now = self.subject_row(&subject)?;
                if now == before {
                    Ok(())
                } else {
                    Err(format!("subject changed from {before:?} to {now:?}"))
                }
            }
            other => Err(format!("unknown step kind `{other}`")),
        }
    }
}

fn store() -> RunStore {
    let mut issued = 0u64;
    RunStore::new(move || {
        issued += 1;
        RunId(Uuid(format!(
            "00000000-0000-4000-8000-{:012x}",
            0xa000 + issued
        )))
    })
}

/// The scenarios this runner answers: those of the Run commands and the Run lifecycle. The
/// action-request revalidation scenarios are answered by `story:commission-ess-conformance`.
const RUN_SCENARIO_PREFIXES: [&str; 4] = [
    "commission.responsibility.StartRun/",
    "commission.responsibility.SuspendRun/",
    "commission.responsibility.ResumeRun/",
    "commission.responsibility.Run/",
];

/// Runs every Run scenario of `suite` against fresh ports from `ports`: `(scenarios, failures)`.
fn run_suite<P: RunStorage + Context>(
    suite: &Value,
    ports: impl Fn() -> P,
) -> (usize, Vec<String>) {
    let Some(Value::Object(scenarios)) = suite.member("scenarios") else {
        panic!("the suite holds no scenarios");
    };
    let scenarios: Vec<&(String, Value)> = scenarios
        .iter()
        .filter(|(id, _)| {
            RUN_SCENARIO_PREFIXES
                .iter()
                .any(|prefix| id.starts_with(prefix))
        })
        .collect();
    let mut failures = Vec::new();
    for (id, scenario) in &scenarios {
        let mut runner = Runner::new(ports());
        let Some(Value::Array(steps)) = scenario.member("steps") else {
            failures.push(format!("{id}: no steps"));
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            if let Err(problem) = runner.step(step) {
                failures.push(format!("{id} step {index}: {problem}"));
                break;
            }
        }
    }
    (scenarios.len(), failures)
}

#[test]
fn adversary2_run_the_synthesized_run_scenarios_pass_against_run_store() {
    let (count, failures) = run_suite(&suite(), store);
    assert_eq!(count, 9, "the brief names 9 synthesized scenarios");
    assert!(
        failures.is_empty(),
        "{} of {count} scenarios failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A store that drops every write of a suspended run: the mutant the runner above must catch, so
/// its green says something about `RunStore`.
struct ForgetsSuspension(RunStore);

impl RunStorage for ForgetsSuspension {
    fn get(&self, identity: &RunId) -> Option<RunSnapshot> {
        self.0.get(identity)
    }
    fn put(&mut self, snapshot: RunSnapshot) {
        if snapshot.state != RunState::Suspended {
            self.0.put(snapshot);
        }
    }
    fn delete(&mut self, identity: &RunId) {
        self.0.delete(identity);
    }
    fn list(&self) -> Vec<RunSnapshot> {
        self.0.list()
    }
}

impl Context for ForgetsSuspension {
    fn generate_commission_responsibility_run_id(&mut self) -> RunId {
        self.0.generate_commission_responsibility_run_id()
    }
}

/// A store that lists nothing: the RunStates view goes empty.
struct ListsNothing(RunStore);

impl RunStorage for ListsNothing {
    fn get(&self, identity: &RunId) -> Option<RunSnapshot> {
        self.0.get(identity)
    }
    fn put(&mut self, snapshot: RunSnapshot) {
        self.0.put(snapshot);
    }
    fn delete(&mut self, identity: &RunId) {
        self.0.delete(identity);
    }
    fn list(&self) -> Vec<RunSnapshot> {
        Vec::new()
    }
}

impl Context for ListsNothing {
    fn generate_commission_responsibility_run_id(&mut self) -> RunId {
        self.0.generate_commission_responsibility_run_id()
    }
}

#[test]
fn adversary2_run_the_runner_fails_a_store_that_breaks_the_scenarios() {
    let suite = suite();
    let (_, forgets) = run_suite(&suite, || ForgetsSuspension(store()));
    assert!(
        forgets.len() >= 5,
        "a store that forgets suspensions failed only {} scenarios: {forgets:?}",
        forgets.len()
    );
    // The two unknown-instance scenarios query the view but expect nothing of it; the other seven
    // each expect a row.
    let (_, lists) = run_suite(&suite, || ListsNothing(store()));
    assert_eq!(
        lists.len(),
        7,
        "a store that lists nothing passed a scenario that expects a row: {lists:?}"
    );
}
