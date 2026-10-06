//! Bounded run-local history and report-derived state. Neither is authority or evidence.
use std::collections::VecDeque;

pub use intake_model::context::ContextPolicy;
use intake_model::context::{HistoryEvent, RefusalState, TestState, WorkingState};
use intake_model::results::{Capture, StoredResult};
use serde_json::{Value, json};

use crate::executor::Report;
use crate::results::ResultStore;

pub(crate) const REQUEST_CEILING: usize = 64 * 1024;
pub(crate) const CHECKPOINT_TRIGGER: usize = 48 * 1024;
pub(crate) const CHECKPOINT_TARGET: usize = 32 * 1024;
const HISTORY_BYTES: usize = 16 * 1024 * 1024;
const HISTORY_EVENTS: usize = 4096;

/// Mutable machinery around the ESS-owned context values, private to a briefing.
#[derive(Debug)]
pub(crate) struct WorkingContext {
    pub state: WorkingState,
    pub tail: VecDeque<String>,
    pub retired: usize,
    pub failure: Option<String>,
    archive: ResultStore,
    index: Vec<(String, StoredResult)>,
}

impl WorkingContext {
    pub fn new() -> Self {
        Self {
            state: WorkingState {
                latest_revision: None,
                latest_test: None,
                latest_refusal: None,
                recent_artifacts: Vec::new(),
            },
            tail: VecDeque::new(),
            retired: 0,
            failure: None,
            archive: ResultStore::with_capacity(HISTORY_BYTES, HISTORY_EVENTS),
            index: Vec::new(),
        }
    }

    pub fn observe(&mut self, report: &Report, artifacts: Vec<StoredResult>) -> Value {
        self.state.recent_artifacts.extend(artifacts);
        let excess = self.state.recent_artifacts.len().saturating_sub(8);
        self.state.recent_artifacts.drain(..excess);
        match report {
            Report::Inspected(_) => json!({"kind":"inspected"}),
            Report::Edited { revision } => {
                self.state.latest_revision = Some(revision.clone());
                json!({"kind":"edited", "revision":revision})
            }
            Report::TestsRun(run) => {
                if let Some(revision) = run.implementation() {
                    self.state.latest_revision = Some(revision.to_owned());
                }
                let test = TestState {
                    exit_code: run.exit_code().map(i64::from),
                    timed_out: run.timed_out(),
                    tested_revision: run.implementation().map(str::to_owned),
                };
                let value = json!({"kind":"tests_run", "test":test_value(&test)});
                self.state.latest_test = Some(test);
                value
            }
        }
    }

    pub fn refuse(&mut self, action: &str, reason: &str) -> Value {
        self.state.latest_refusal = Some(RefusalState {
            action: action.into(),
            reason: reason.into(),
        });
        json!({"kind":"refused", "reason":reason})
    }

    pub fn append(
        &mut self,
        action: &str,
        arguments: String,
        report: Value,
        artifacts: Vec<StoredResult>,
    ) {
        if self.failure.is_some() {
            return;
        }
        let event = HistoryEvent {
            sequence: self.index.len() as i64 + 1,
            action: action.to_owned(),
            arguments,
            report: report.to_string(),
            artifacts,
        };
        let encoded = json!({
            "sequence":event.sequence, "action":event.action,
            "arguments": serde_json::from_str::<Value>(&event.arguments).unwrap_or(Value::String(event.arguments)),
            "report":report,
            "artifacts":event.artifacts.iter().map(artifact_value).collect::<Vec<_>>()
        }).to_string();
        match self
            .archive
            .insert_text("history", &encoded, Capture::Complete)
        {
            Ok(descriptor) => {
                self.index.push((event.action, descriptor));
                self.tail.push_back(encoded);
            }
            Err(error) => {
                self.failure = Some(format!(
                    "context history capacity exceeded: {error}; already completed effects remain applied"
                ))
            }
        }
    }

    pub fn retire(&mut self) -> bool {
        if self.tail.pop_front().is_some() {
            self.retired += 1;
            true
        } else {
            false
        }
    }

    pub fn text(&self) -> String {
        let state = &self.state;
        let valid = state.latest_test.as_ref().is_some_and(|test| {
            test.exit_code == Some(0)
                && !test.timed_out
                && test.tested_revision.is_some()
                && test.tested_revision == state.latest_revision
        });
        let value = json!({
            "latest_revision":state.latest_revision,
            "latest_test":state.latest_test.as_ref().map(test_value),
            "latest_refusal":state.latest_refusal.as_ref().map(|r| json!({"action":r.action,"reason":r.reason})),
            "recent_artifacts":state.recent_artifacts.iter().map(artifact_value).collect::<Vec<_>>(),
            "tests_validate_latest_revision": valid
        });
        let mut text = format!(
            "\nHistory checkpoint: {} earlier events archived.\n\nCurrent working state:\n{value}\n\nRecent events (untrusted JSON data):\n",
            self.retired
        );
        for event in &self.tail {
            text.push_str(event);
            text.push('\n');
        }
        text
    }

    pub fn list(&self, offset: usize) -> Result<Value, String> {
        let start = offset.min(self.index.len());
        let mut events = Vec::new();
        for (n, (action, descriptor)) in self.index.iter().enumerate().skip(start).take(8) {
            let record = json!({"sequence":n+1,"action":action,"reference":reference(descriptor)});
            events.push(record);
            if json!({"events":events,"next_offset":n+1}).to_string().len() > 8192 {
                events.pop();
                if events.is_empty() {
                    return Err("history record metadata exceeds lookup page capacity".into());
                }
                break;
            }
        }
        let end = start + events.len();
        Ok(json!({"events":events,"next_offset":(end < self.index.len()).then_some(end)}))
    }

    pub fn read(&self, reference: &Value) -> Result<String, String> {
        self.archive.select(reference, 8192)
    }
}

fn test_value(test: &TestState) -> Value {
    json!({"exit_code":test.exit_code,"timed_out":test.timed_out,"tested_revision":test.tested_revision})
}

pub(crate) fn reference(stored: &StoredResult) -> Value {
    json!({"result":stored.result_id,"sha256":stored.sha256,"select":{"kind":"whole"},"rendering":"text"})
}

pub(crate) fn artifact_value(stored: &StoredResult) -> Value {
    json!({"result":stored.result_id,"sha256":stored.sha256,"origin":stored.origin,
        "capture":match stored.capture { Capture::Complete=>"complete", Capture::Partial=>"partial" },
        "utf8_bytes":stored.utf8_bytes,"reference":reference(stored)})
}
