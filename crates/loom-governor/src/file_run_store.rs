//! [`FileWorkflowRunStore`], `b10x-loom-executor`'s [`WorkflowRunStore`] port in a directory, so a
//! workflow run continues after a restart (`architecture-decision-record:workflow-run-store`).
//!
//! A record is the generated `loom.workflow.WorkflowRunRecord` (`ess/domains/workflow.yaml`): the
//! Run it is keyed by, the definition it runs, the steps performed and the one proposed, the
//! outputs taken, and the usage carried (`loom.metering.CarriedUsage`). `b10x-loom-executor`'s
//! `WorkflowExecutor` writes it after every performed step and every executor call, and reads it
//! back to resume the run.
//!
//! The file store keeps one file per Run, `workflow-run-<hex of the run id>.json`, through the
//! directory discipline [`FileCaseStore`](crate::FileCaseStore) and
//! [`FileSuspensionStore`](crate::FileSuspensionStore) share (the private `locked_dir` module):
//! every operation holds the directory's lock, every write goes to a fresh temporary file that is
//! synced and renamed over its target, so a reader sees the previous record or the next one and
//! never part of one, and the three stores may share one directory. Before anything is written it
//! is read back and compared with what was meant, so a record that would not come back exactly is
//! refused rather than written. A document naming a member its declaration does not, or one member
//! twice, is refused on read, as is one holding the record of another Run: a record is read only
//! under the Run it names.
//!
//! A run id over [`LONGEST_RUN_ID`] bytes is refused: its file name would pass the 255 bytes a file
//! name may have.

use std::fmt;
use std::path::{Path, PathBuf};

use b10x_loom_executor::workflow::{RunStoreError, WorkflowRunStore};
use loom::json::{
    self, DecodeError, Value, integer_at, items_at, member, member_at, members_at, nested,
    push_integer, push_text, push_value, text_at, uuid_at,
};
use loom::metering::{ActionUsage, CarriedUsage, EffectsUsage, StepUsage};
use loom::primitives::Uuid;
use loom::run::CommissionRunId;
use loom::workflow::{RecordedOutput, StepRequest, UnproducedOutput, WorkflowRunRecord};

use crate::FileStoreError;
use crate::locked_dir::{LockedDir, read_if_present};

/// The longest run id, in bytes, the file store keeps: `workflow-run-`, 200 hex digits and `.json`
/// is 218 bytes.
pub const LONGEST_RUN_ID: usize = 100;

impl From<FileStoreError> for RunStoreError {
    fn from(error: FileStoreError) -> Self {
        Self(error.to_string())
    }
}

/// A [`WorkflowRunStore`] keeping one file per Run in a directory (see the module docs).
#[derive(Debug)]
pub struct FileWorkflowRunStore {
    dir: LockedDir,
}

fn failed(path: &Path, problem: impl fmt::Display) -> RunStoreError {
    RunStoreError(format!("{}: {problem}", path.display()))
}

impl FileWorkflowRunStore {
    /// A store over `dir`, created when it does not exist yet. `dir` may be the directory a
    /// [`FileCaseStore`](crate::FileCaseStore) or a [`FileSuspensionStore`](crate::FileSuspensionStore)
    /// keeps its files in.
    ///
    /// # Errors
    ///
    /// [`RunStoreError`] when the directory or its lock file cannot be created, a temporary file an
    /// earlier life left behind cannot be removed, or the directory does not take a write.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, RunStoreError> {
        Ok(Self {
            dir: LockedDir::open(dir.into())?,
        })
    }

    /// The directory the store keeps its files in.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    /// The file that holds `run`'s record, named so no id names a file outside the store.
    pub fn path(&self, run: &CommissionRunId) -> Result<PathBuf, RunStoreError> {
        let id = run.0.0.as_bytes();
        if id.len() > LONGEST_RUN_ID {
            return Err(RunStoreError(format!(
                "a run id of {} bytes is longer than the {LONGEST_RUN_ID} this store keeps",
                id.len()
            )));
        }
        let mut name = String::from("workflow-run-");
        for byte in id {
            name.push_str(&format!("{byte:02x}"));
        }
        name.push_str(".json");
        Ok(self.dir.path().join(name))
    }
}

impl WorkflowRunStore for FileWorkflowRunStore {
    fn load(&self, run: &CommissionRunId) -> Result<Option<WorkflowRunRecord>, RunStoreError> {
        let path = self.path(run)?;
        self.dir.locked(|| {
            let Some(text) = read_if_present(&path)? else {
                return Ok(None);
            };
            let record = json::parse(&text)
                .map_err(|error| failed(&path, error))
                .and_then(|value| decode(&value).map_err(|error| failed(&path, error)))?;
            if record.run != *run {
                return Err(failed(
                    &path,
                    format!(
                        "holds the record of run `{}`, not `{}`; it is not read",
                        record.run.0.0, run.0.0
                    ),
                ));
            }
            Ok(Some(record))
        })?
    }

    fn save(&self, record: &WorkflowRunRecord) -> Result<(), RunStoreError> {
        let path = self.path(&record.run)?;
        let mut text = String::new();
        encode(&mut text, record);
        let reread = json::parse(&text)
            .ok()
            .and_then(|value| decode(&value).ok());
        if reread.as_ref() != Some(record) {
            return Err(failed(
                &path,
                "the record would not read back as it is held; nothing was written",
            ));
        }
        self.dir
            .locked(|| self.dir.replace(&path, &text))?
            .map_err(RunStoreError::from)
    }
}

/// The members a record has, in the order written.
const MEMBERS: [&str; 9] = [
    "run",
    "workflow",
    "definition_digest",
    "performed",
    "proposed",
    "outputs",
    "unproduced",
    "attempted",
    "usage",
];

fn encode(out: &mut String, record: &WorkflowRunRecord) {
    out.push('{');
    member(out, "run");
    push_text(out, &record.run.0.0);
    member(out, "workflow");
    push_text(out, &record.workflow);
    member(out, "definition_digest");
    push_text(out, &record.definition_digest);
    member(out, "performed");
    list(out, &record.performed, encode_request);
    if let Some(proposed) = &record.proposed {
        member(out, "proposed");
        encode_request(out, proposed);
    }
    member(out, "outputs");
    list(out, &record.outputs, |out, output: &RecordedOutput| {
        out.push('{');
        member(out, "step");
        push_text(out, &output.step);
        member(out, "output");
        push_text(out, &output.output);
        member(out, "value");
        push_value(out, &output.value);
        out.push('}');
    });
    if let Some(unproduced) = &record.unproduced {
        member(out, "unproduced");
        out.push('{');
        member(out, "step");
        push_text(out, &unproduced.step);
        member(out, "output");
        push_text(out, &unproduced.output);
        member(out, "why");
        push_text(out, &unproduced.why);
        out.push('}');
    }
    member(out, "attempted");
    list(out, &record.attempted, |out, step: &String| {
        push_text(out, step)
    });
    member(out, "usage");
    out.push('{');
    member(out, "steps");
    list(out, &record.usage.steps, encode_step_usage);
    member(out, "effects");
    encode_effects(out, &record.usage.effects);
    out.push('}');
    out.push('}');
}

fn list<T>(out: &mut String, items: &[T], each: impl Fn(&mut String, &T)) {
    out.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        each(out, item);
    }
    out.push(']');
}

fn encode_request(out: &mut String, request: &StepRequest) {
    out.push('{');
    member(out, "step");
    push_text(out, &request.step);
    member(out, "action");
    push_text(out, &request.action);
    member(out, "arguments");
    push_value(out, &request.arguments);
    out.push('}');
}

fn optional_integer(out: &mut String, name: &str, value: Option<i64>) {
    if let Some(value) = value {
        member(out, name);
        push_integer(out, value);
    }
}

fn encode_step_usage(out: &mut String, step: &StepUsage) {
    out.push('{');
    member(out, "step");
    push_integer(out, step.step);
    if let Some(label) = &step.label {
        member(out, "label");
        push_text(out, label);
    }
    member(out, "model_calls");
    push_integer(out, step.model_calls);
    member(out, "input_tokens");
    push_integer(out, step.input_tokens);
    member(out, "output_tokens");
    push_integer(out, step.output_tokens);
    member(out, "cached_input_tokens");
    push_integer(out, step.cached_input_tokens);
    optional_integer(out, "cost_micro_usd", step.cost_micro_usd);
    out.push('}');
}

fn encode_effects(out: &mut String, effects: &EffectsUsage) {
    out.push('{');
    member(out, "trials");
    push_integer(out, effects.trials);
    member(out, "input_tokens");
    push_integer(out, effects.input_tokens);
    member(out, "output_tokens");
    push_integer(out, effects.output_tokens);
    member(out, "duration_ms");
    push_integer(out, effects.duration_ms);
    optional_integer(out, "cost_micro_usd", effects.cost_micro_usd);
    member(out, "actions");
    list(out, &effects.actions, |out, action: &ActionUsage| {
        out.push('{');
        member(out, "action");
        push_text(out, &action.action);
        member(out, "trials");
        push_integer(out, action.trials);
        member(out, "input_tokens");
        push_integer(out, action.input_tokens);
        member(out, "output_tokens");
        push_integer(out, action.output_tokens);
        member(out, "duration_ms");
        push_integer(out, action.duration_ms);
        optional_integer(out, "cost_micro_usd", action.cost_micro_usd);
        out.push('}');
    });
    out.push('}');
}

/// Refuses an object with a member `declared` does not name, or one member twice.
fn exactly(value: &Value, at: &str, declared: &[&str]) -> Result<(), DecodeError> {
    let members = members_at(value, at, "an object")?;
    for (index, (name, _)) in members.iter().enumerate() {
        if !declared.contains(&name.as_str()) {
            return Err(DecodeError {
                at: nested(at, name),
                expected: "no such member".to_owned(),
                found: "a member the declaration does not name".to_owned(),
            });
        }
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(DecodeError {
                at: nested(at, name),
                expected: "one member of this name".to_owned(),
                found: "a second one".to_owned(),
            });
        }
    }
    Ok(())
}

/// The member `name` of the object at `at`, or `None` when it is absent.
fn optional<'v>(value: &'v Value, at: &str, name: &str) -> Result<Option<&'v Value>, DecodeError> {
    Ok(members_at(value, at, "an object")?
        .iter()
        .find(|(member, _)| member == name)
        .map(|(_, value)| value))
}

fn text(value: &Value, at: &str, name: &str) -> Result<String, DecodeError> {
    text_at(member_at(value, at, name)?, &nested(at, name), "a string").map(str::to_owned)
}

fn integer(value: &Value, at: &str, name: &str) -> Result<i64, DecodeError> {
    integer_at(member_at(value, at, name)?, &nested(at, name), "an integer")
}

fn optional_integer_at(value: &Value, at: &str, name: &str) -> Result<Option<i64>, DecodeError> {
    optional(value, at, name)?
        .map(|found| integer_at(found, &nested(at, name), "an integer"))
        .transpose()
}

fn items<T>(
    value: &Value,
    at: &str,
    name: &str,
    each: impl Fn(&Value, &str) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    let path = nested(at, name);
    items_at(member_at(value, at, name)?, &path, "an array")?
        .iter()
        .enumerate()
        .map(|(index, item)| each(item, &format!("{path}[{index}]")))
        .collect()
}

fn decode(value: &Value) -> Result<WorkflowRunRecord, DecodeError> {
    exactly(value, "", &MEMBERS)?;
    let run = uuid_at(member_at(value, "", "run")?, "run", "a UUID")?;
    let usage = member_at(value, "", "usage")?;
    exactly(usage, "usage", &["steps", "effects"])?;
    Ok(WorkflowRunRecord {
        run: CommissionRunId(Uuid(run.to_owned())),
        workflow: text(value, "", "workflow")?,
        definition_digest: text(value, "", "definition_digest")?,
        performed: items(value, "", "performed", decode_request)?,
        proposed: optional(value, "", "proposed")?
            .map(|proposed| decode_request(proposed, "proposed"))
            .transpose()?,
        outputs: items(value, "", "outputs", |item, at| {
            exactly(item, at, &["step", "output", "value"])?;
            Ok(RecordedOutput {
                step: text(item, at, "step")?,
                output: text(item, at, "output")?,
                value: member_at(item, at, "value")?.clone(),
            })
        })?,
        unproduced: optional(value, "", "unproduced")?
            .map(|item| {
                let at = "unproduced";
                exactly(item, at, &["step", "output", "why"])?;
                Ok(UnproducedOutput {
                    step: text(item, at, "step")?,
                    output: text(item, at, "output")?,
                    why: text(item, at, "why")?,
                })
            })
            .transpose()?,
        attempted: items(value, "", "attempted", |item, at| {
            text_at(item, at, "a string").map(str::to_owned)
        })?,
        usage: CarriedUsage {
            steps: items(usage, "usage", "steps", decode_step_usage)?,
            effects: decode_effects(member_at(usage, "usage", "effects")?, "usage.effects")?,
        },
    })
}

fn decode_request(value: &Value, at: &str) -> Result<StepRequest, DecodeError> {
    exactly(value, at, &["step", "action", "arguments"])?;
    Ok(StepRequest {
        step: text(value, at, "step")?,
        action: text(value, at, "action")?,
        arguments: member_at(value, at, "arguments")?.clone(),
    })
}

fn decode_step_usage(value: &Value, at: &str) -> Result<StepUsage, DecodeError> {
    exactly(
        value,
        at,
        &[
            "step",
            "label",
            "model_calls",
            "input_tokens",
            "output_tokens",
            "cached_input_tokens",
            "cost_micro_usd",
        ],
    )?;
    Ok(StepUsage {
        step: integer(value, at, "step")?,
        label: optional(value, at, "label")?
            .map(|label| text_at(label, &nested(at, "label"), "a string").map(str::to_owned))
            .transpose()?,
        model_calls: integer(value, at, "model_calls")?,
        input_tokens: integer(value, at, "input_tokens")?,
        output_tokens: integer(value, at, "output_tokens")?,
        cached_input_tokens: integer(value, at, "cached_input_tokens")?,
        cost_micro_usd: optional_integer_at(value, at, "cost_micro_usd")?,
    })
}

fn decode_effects(value: &Value, at: &str) -> Result<EffectsUsage, DecodeError> {
    exactly(
        value,
        at,
        &[
            "trials",
            "input_tokens",
            "output_tokens",
            "duration_ms",
            "cost_micro_usd",
            "actions",
        ],
    )?;
    Ok(EffectsUsage {
        trials: integer(value, at, "trials")?,
        input_tokens: integer(value, at, "input_tokens")?,
        output_tokens: integer(value, at, "output_tokens")?,
        duration_ms: integer(value, at, "duration_ms")?,
        cost_micro_usd: optional_integer_at(value, at, "cost_micro_usd")?,
        actions: items(value, at, "actions", |item, at| {
            exactly(
                item,
                at,
                &[
                    "action",
                    "trials",
                    "input_tokens",
                    "output_tokens",
                    "duration_ms",
                    "cost_micro_usd",
                ],
            )?;
            Ok(ActionUsage {
                action: text(item, at, "action")?,
                trials: integer(item, at, "trials")?,
                input_tokens: integer(item, at, "input_tokens")?,
                output_tokens: integer(item, at, "output_tokens")?,
                duration_ms: integer(item, at, "duration_ms")?,
                cost_micro_usd: optional_integer_at(item, at, "cost_micro_usd")?,
            })
        })?,
    })
}
