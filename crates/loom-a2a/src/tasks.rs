//! The tasks the listener answers: one per message, held in memory while the process runs.
//!
//! A task is `TASK_STATE_SUBMITTED` when its message is admitted. Each [`RuntimeEvent`] of its
//! turn moves it to `TASK_STATE_WORKING`, its status message naming the event ([`follows`]); the
//! turn's record ends it ([`ended`]): a proposal completes it, a decline rejects it, any other
//! record or a turn that could not run fails it. Only the names of the steps reach a status, never
//! an argument or a value read.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};

use a2a::primitives::Timestamp;
use a2a::tasks::{
    Artifact, ContextId, Message, Part, Role, TaskId, TaskSnapshot, TaskStatus, WireTaskState,
};
use b10x_loom_plugin::{RecordLine, RecordOutcome, encode_record_line};
use chrono::{SecondsFormat, Utc};
use loom_sdk::EventSink;
use loom_sdk::commission::model::responsibility::RuntimeEvent;

use crate::wire;

/// The most tasks held at once; past it the oldest ended task is dropped first.
pub const MAX_TASKS: usize = 256;

/// The state and the status text a task moves to on `event`. Every event of a running turn is
/// work: the state is `TASK_STATE_WORKING`, and the text names the step, the action or the stop.
pub fn follows(event: &RuntimeEvent) -> (WireTaskState, String) {
    let text = match event {
        RuntimeEvent::StepStarted(step) => format!("step {} started", step.step),
        RuntimeEvent::ActionSelected(selected) => format!("{} selected", selected.action),
        RuntimeEvent::Proposed(proposed) => format!("{} requested", proposed.action),
        RuntimeEvent::AuthorityDecided(decided) => {
            format!("authority decided on {}", decided.capability)
        }
        RuntimeEvent::EffectInvoked(invoked) => format!("{} invoked", invoked.action),
        RuntimeEvent::SessionClosed(closed) => format!("session of step {} closed", closed.step),
        RuntimeEvent::Stopped(stopped) => match (&stopped.outcome, &stopped.failure) {
            (_, Some(_)) => "the run failed".to_owned(),
            (Some(outcome), None) => format!("the run stopped: {}", outcome_name(outcome)),
            (None, None) => "the run stopped".to_owned(),
        },
    };
    (WireTaskState::TASKSTATEWORKING, text)
}

fn outcome_name(outcome: &loom_sdk::commission::model::responsibility::RunOutcome) -> String {
    let debug = format!("{outcome:?}");
    debug
        .split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The state a task ends in for `record`, and its status text.
pub fn ended(record: &RecordLine) -> (WireTaskState, String) {
    let detail = record.detail.clone();
    match record.outcome {
        RecordOutcome::Proposed => (
            WireTaskState::TASKSTATECOMPLETED,
            record
                .proposal
                .clone()
                .unwrap_or_else(|| "proposed".to_owned()),
        ),
        RecordOutcome::ProposedCase => (
            WireTaskState::TASKSTATECOMPLETED,
            record.proposed_case.as_ref().map_or_else(
                || "a case is proposed".to_owned(),
                |case| format!("a case is proposed on {}", case.protocol),
            ),
        ),
        RecordOutcome::Declined => (
            WireTaskState::TASKSTATEREJECTED,
            detail.unwrap_or_else(|| "declined".to_owned()),
        ),
        RecordOutcome::Unclassified => (
            WireTaskState::TASKSTATEFAILED,
            detail.unwrap_or_else(|| "unclassified".to_owned()),
        ),
        RecordOutcome::Stopped => (
            WireTaskState::TASKSTATEFAILED,
            detail.unwrap_or_else(|| "stopped".to_owned()),
        ),
    }
}

fn now() -> Timestamp {
    Timestamp(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn ended_state(state: WireTaskState) -> bool {
    matches!(
        state,
        WireTaskState::TASKSTATECOMPLETED
            | WireTaskState::TASKSTATEFAILED
            | WireTaskState::TASKSTATEREJECTED
            | WireTaskState::TASKSTATECANCELED
    )
}

/// The tasks of one listener.
#[derive(Default)]
pub struct Tasks {
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    tasks: BTreeMap<String, TaskSnapshot>,
    order: VecDeque<String>,
    next: u64,
}

impl Tasks {
    fn held(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A new task for `request`, the user's message, `TASK_STATE_SUBMITTED`; its id is new and
    /// its context the message's, or new.
    pub fn submit(&self, mut request: Message) -> TaskSnapshot {
        let mut held = self.held();
        held.next += 1;
        let stamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let id = format!("{stamp:x}-{:x}", held.next);
        let context = request
            .context_id
            .clone()
            .unwrap_or_else(|| ContextId(format!("ctx-{id}")));
        request.task_id = Some(TaskId(id.clone()));
        request.context_id = Some(context.clone());
        let task = TaskSnapshot {
            id: TaskId(id.clone()),
            context_id: Some(context),
            status: TaskStatus {
                state: WireTaskState::TASKSTATESUBMITTED,
                message: None,
                timestamp: Some(now()),
            },
            artifacts: None,
            history: Some(vec![request]),
            metadata: None,
        };
        held.tasks.insert(id.clone(), task.clone());
        held.order.push_back(id);
        while held.tasks.len() > MAX_TASKS {
            let oldest_ended = held.order.iter().position(|id| {
                held.tasks
                    .get(id)
                    .is_none_or(|task| ended_state(task.status.state))
            });
            let Some(at) = oldest_ended else { break };
            if let Some(id) = held.order.remove(at) {
                held.tasks.remove(&id);
            }
        }
        task
    }

    /// The task `id`, when it is held.
    pub fn get(&self, id: &str) -> Option<TaskSnapshot> {
        self.held().tasks.get(id).cloned()
    }

    /// Moves the task `id` to `state` with an agent message of `text`.
    pub fn update(&self, id: &str, state: WireTaskState, text: &str) {
        let mut held = self.held();
        if let Some(task) = held.tasks.get_mut(id) {
            task.status = status(task, state, text);
        }
    }

    /// Ends the task `id` with `record` (see the module documentation); the record is the task's
    /// artifact, its proposal as text beside it.
    pub fn end(&self, id: &str, record: &RecordLine) -> Option<TaskSnapshot> {
        let (state, text) = ended(record);
        let mut parts: Vec<Part> = record
            .proposal
            .as_deref()
            .map(wire::text_part)
            .into_iter()
            .collect();
        if let Ok(data) = a2a::json::parse(&encode_record_line(record)) {
            parts.push(wire::data_part(data));
        }
        let mut held = self.held();
        let task = held.tasks.get_mut(id)?;
        task.status = status(task, state, &text);
        task.artifacts = Some(vec![Artifact {
            artifact_id: format!("{id}-record"),
            name: Some("record".to_owned()),
            description: Some("the turn's record line".to_owned()),
            parts,
            metadata: None,
            extensions: None,
        }]);
        Some(task.clone())
    }

    /// Fails the task `id`: its turn could not run, for `why`.
    pub fn fail(&self, id: &str, why: &str) -> Option<TaskSnapshot> {
        let mut held = self.held();
        let task = held.tasks.get_mut(id)?;
        task.status = status(task, WireTaskState::TASKSTATEFAILED, why);
        Some(task.clone())
    }
}

fn status(task: &TaskSnapshot, state: WireTaskState, text: &str) -> TaskStatus {
    TaskStatus {
        state,
        message: Some(Message {
            message_id: format!("{}-status", task.id.0),
            context_id: task.context_id.clone(),
            task_id: Some(task.id.clone()),
            role: Role::ROLEAGENT,
            parts: vec![wire::text_part(text)],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        }),
        timestamp: Some(now()),
    }
}

/// The sink of one task's turn: each event moves the task as [`follows`] says.
pub struct TaskEvents<'a> {
    tasks: &'a Tasks,
    id: String,
}

impl<'a> TaskEvents<'a> {
    /// The sink moving the task `id` of `tasks`.
    pub fn new(tasks: &'a Tasks, id: &str) -> Self {
        Self {
            tasks,
            id: id.to_owned(),
        }
    }
}

impl EventSink for TaskEvents<'_> {
    fn emit(&mut self, event: &RuntimeEvent) {
        let (state, text) = follows(event);
        self.tasks.update(&self.id, state, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b10x_loom_plugin::ItemId;

    fn record(outcome: RecordOutcome) -> RecordLine {
        RecordLine {
            item: ItemId("a2a:1".to_owned()),
            intent: None,
            confidence: None,
            outcome,
            reads: Vec::new(),
            proposal: (outcome == RecordOutcome::Proposed).then(|| "yes".to_owned()),
            proposed_case: None,
            detail: Some("why".to_owned()),
        }
    }

    #[test]
    fn a_record_ends_its_task() {
        for (outcome, state) in [
            (RecordOutcome::Proposed, WireTaskState::TASKSTATECOMPLETED),
            (RecordOutcome::Declined, WireTaskState::TASKSTATEREJECTED),
            (RecordOutcome::Stopped, WireTaskState::TASKSTATEFAILED),
            (RecordOutcome::Unclassified, WireTaskState::TASKSTATEFAILED),
        ] {
            assert_eq!(ended(&record(outcome)).0, state, "{outcome:?}");
        }
    }

    #[test]
    fn the_oldest_ended_task_is_dropped_past_the_bound() {
        let tasks = Tasks::default();
        let message = || Message {
            message_id: "m".to_owned(),
            context_id: None,
            task_id: None,
            role: Role::ROLEUSER,
            parts: Vec::new(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };
        let running = tasks.submit(message()).id.0;
        let first_ended = tasks.submit(message()).id.0;
        tasks.fail(&first_ended, "done");
        for _ in 0..MAX_TASKS {
            let id = tasks.submit(message()).id.0;
            tasks.fail(&id, "done");
        }
        assert!(tasks.get(&running).is_some(), "a running task is kept");
        assert!(tasks.get(&first_ended).is_none(), "the oldest ended goes");
        assert!(tasks.held().tasks.len() <= MAX_TASKS);
    }
}
