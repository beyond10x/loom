//! The run event stream `b10x-loom run --output jsonl` writes: one JSON object per line
//! (`intake.events.RunEventLine` in `ess/intake/domains/events.yaml`), built from the generated
//! `intake.events` types.
//!
//! A line is the record's fields beside `schema_version` and `kind`, the union's tag. A counter or
//! field the run does not have is absent, never `null` or zero. The stream is a projection of what
//! the run does; it adds no authority and is not evidence. Every write is flushed, so a supervisor
//! reads each record as it happens, and [`EventStream::finish`] writes the one terminal record,
//! last.

use std::io::{self, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_intake_slice::run::{SliceRun, StopReason};
use intake_model::context::RequestPhase;
use intake_model::events::{
    ApprovalEvent, RouteEvent, RunEvent, RunEventLine, SchemaVersion, TerminalEvent, ToolCallEvent,
    TurnEvent, UsageEvent,
};
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StopReason as TurnStop, StreamSink,
    TurnOutcome, TurnRequest,
};
use serde_json::{Map, Value, json};

use crate::exit_status;

/// The schema version every line carries, as written: `intake.events.SchemaVersion::V1`.
pub const SCHEMA_VERSION: u64 = version(SchemaVersion::V1);

/// The number a schema version is written as.
pub const fn version(schema: SchemaVersion) -> u64 {
    match schema {
        SchemaVersion::V1 => 1,
    }
}

/// The exit status of a run that failed rather than stopped for a reason.
pub const FAILED: u8 = 1;

/// Where the records of one run go. Clones share the writer and the turn count.
#[derive(Clone)]
pub struct EventStream {
    shared: Arc<Mutex<Shared>>,
}

struct Shared {
    out: Box<dyn Write + Send>,
    turns: i64,
    failure: Option<io::Error>,
}

impl EventStream {
    /// A stream writing to `out`, usually standard output.
    pub fn new(out: impl Write + Send + 'static) -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                out: Box::new(out),
                turns: 0,
                failure: None,
            })),
        }
    }

    /// `model`, recording each turn it answers, the tool calls in it and the usage reported for it.
    pub fn observe<'m>(&self, model: &'m dyn Model) -> Observed<'m> {
        Observed {
            inner: model,
            stream: self.clone(),
        }
    }

    /// The route record: the protocol the router picked, and whether the run executes it.
    ///
    /// # Errors
    /// The stream cannot be written.
    pub fn route(&self, protocol: &str, accepted: bool) -> io::Result<()> {
        self.emit(&RunEvent::Route(RouteEvent {
            protocol: protocol.to_owned(),
            accepted,
        }))
    }

    /// Ends the stream: an approval record when the run stopped for approval, then the one
    /// terminal record. Answers the exit status the process returns: [`exit_status`] of the stop
    /// reason, or [`FAILED`] for a run that failed with the given message.
    ///
    /// # Errors
    /// The stream cannot be written, now or at an earlier record.
    pub fn finish(&self, ending: Result<&SliceRun, &str>) -> io::Result<u8> {
        let terminal = match ending {
            Ok(run) => {
                if run.stop_reason == StopReason::ApprovalRequired {
                    self.emit(&RunEvent::Approval(ApprovalEvent {
                        actions: run.approvals.clone(),
                    }))?;
                }
                TerminalEvent {
                    stop_reason: Some(run.stop_reason),
                    exit_status: i64::from(exit_status(run.stop_reason)),
                    protocol: Some(run.protocol.clone()),
                    steps: Some(i64::try_from(run.steps).unwrap_or(i64::MAX)),
                    error: None,
                }
            }
            Err(error) => TerminalEvent {
                stop_reason: None,
                exit_status: i64::from(FAILED),
                protocol: None,
                steps: None,
                error: Some(error.to_owned()),
            },
        };
        let status = u8::try_from(terminal.exit_status).unwrap_or(FAILED);
        self.emit(&RunEvent::Terminal(terminal))?;
        match lock(&self.shared).failure.take() {
            Some(error) => Err(error),
            None => Ok(status),
        }
    }

    fn emit(&self, event: &RunEvent) -> io::Result<()> {
        let mut shared = lock(&self.shared);
        let line = line(&RunEventLine {
            schema_version: SchemaVersion::V1,
            event: event.clone(),
        });
        let written = writeln!(shared.out, "{line}").and_then(|()| shared.out.flush());
        if let Err(error) = &written
            && shared.failure.is_none()
        {
            shared.failure = Some(io::Error::new(error.kind(), error.to_string()));
        }
        written
    }

    fn next_turn(&self) -> i64 {
        let mut shared = lock(&self.shared);
        shared.turns += 1;
        shared.turns
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One line of the stream, as JSON.
pub fn line(line: &RunEventLine) -> Value {
    let (kind, fields) = match &line.event {
        RunEvent::Route(event) => (
            "Route",
            json!({"protocol": event.protocol, "accepted": event.accepted}),
        ),
        RunEvent::Turn(event) => (
            "Turn",
            json!({
                "turn": event.turn,
                "phase": format!("{:?}", event.phase),
                "model": event.model,
                "turn_stop": event.turn_stop,
            }),
        ),
        RunEvent::ToolCall(event) => (
            "ToolCall",
            json!({
                "turn": event.turn,
                "call_id": event.call_id,
                "name": event.name,
                "arguments": event.arguments,
            }),
        ),
        RunEvent::Usage(event) => (
            "Usage",
            json!({
                "turn": event.turn,
                "model": event.model,
                "input_tokens": event.input_tokens,
                "output_tokens": event.output_tokens,
                "cached_input_tokens": event.cached_input_tokens,
                "cache_creation_input_tokens": event.cache_creation_input_tokens,
                "reasoning_output_tokens": event.reasoning_output_tokens,
                "final_usage": event.final_usage,
            }),
        ),
        RunEvent::Approval(event) => ("Approval", json!({"actions": event.actions})),
        RunEvent::Terminal(event) => (
            "Terminal",
            json!({
                "stop_reason": event.stop_reason.map(|reason| format!("{reason:?}")),
                "exit_status": event.exit_status,
                "protocol": event.protocol,
                "steps": event.steps,
                "error": event.error,
            }),
        ),
    };
    let mut object = Map::new();
    object.insert("schema_version".into(), json!(version(line.schema_version)));
    object.insert("kind".into(), json!(kind));
    if let Value::Object(fields) = fields {
        object.extend(fields.into_iter().filter(|(_, value)| !value.is_null()));
    }
    Value::Object(object)
}

/// A model whose turns are recorded on an [`EventStream`]; it answers exactly as the model it wraps.
pub struct Observed<'m> {
    inner: &'m dyn Model,
    stream: EventStream,
}

impl Observed<'_> {
    fn record(&self, request: &TurnRequest, outcome: &TurnOutcome) {
        let turn = self.stream.next_turn();
        let model = outcome.observation.binding.model.as_str().to_owned();
        let phase = request
            .tools
            .iter()
            .find_map(|tool| match tool.name.as_str() {
                "pick_protocol" => Some(RequestPhase::Classification),
                "select_action" => Some(RequestPhase::Selection),
                "action_arguments" => Some(RequestPhase::Arguments),
                _ => None,
            });
        // A write failure is kept by the stream and answered by `finish`; the run goes on.
        if let Some(phase) = phase {
            let _ = self.stream.emit(&RunEvent::Turn(TurnEvent {
                turn,
                phase,
                model: model.clone(),
                turn_stop: turn_stop(&outcome.stop_reason).to_owned(),
            }));
        }
        for call in outcome.tool_calls() {
            let _ = self.stream.emit(&RunEvent::ToolCall(ToolCallEvent {
                turn,
                call_id: call.call_id.as_str().to_owned(),
                name: call.name.as_str().to_owned(),
                arguments: call.arguments.to_string(),
            }));
        }
        if let Some(usage) = &outcome.observation.usage {
            let count = |value: Option<u64>| value.map(|n| i64::try_from(n).unwrap_or(i64::MAX));
            let _ = self.stream.emit(&RunEvent::Usage(UsageEvent {
                turn,
                model,
                input_tokens: count(usage.input_tokens),
                output_tokens: count(usage.output_tokens),
                cached_input_tokens: count(usage.cached_input_tokens),
                cache_creation_input_tokens: count(usage.cache_creation_input_tokens),
                reasoning_output_tokens: count(usage.reasoning_output_tokens),
                final_usage: outcome.observation.final_usage,
            }));
        }
    }
}

fn turn_stop(stop: &TurnStop) -> &'static str {
    match stop {
        TurnStop::EndTurn => "EndTurn",
        TurnStop::ToolCalls => "ToolCalls",
        TurnStop::MaxOutputTokens => "MaxOutputTokens",
        TurnStop::Incomplete { .. } => "Incomplete",
    }
}

impl Model for Observed<'_> {
    fn provenance(&self) -> &Provenance {
        self.inner.provenance()
    }

    fn capabilities(&self) -> &Capabilities {
        self.inner.capabilities()
    }

    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        sink: &'a mut dyn StreamSink,
        cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        Box::pin(async move {
            let answer = self.inner.turn(request, sink, cancel).await;
            if let Ok(outcome) = &answer {
                self.record(request, outcome);
            }
            answer
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_carries_the_version_and_tag_and_omits_what_is_absent() {
        let value = line(&RunEventLine {
            schema_version: SchemaVersion::V1,
            event: RunEvent::Terminal(TerminalEvent {
                stop_reason: None,
                exit_status: 1,
                protocol: None,
                steps: None,
                error: Some("no login".into()),
            }),
        });
        assert_eq!(
            value,
            json!({"schema_version": 1, "kind": "Terminal", "exit_status": 1, "error": "no login"})
        );
    }
}
