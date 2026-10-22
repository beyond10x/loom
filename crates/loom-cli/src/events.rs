//! The run event stream `b10x-loom run --output jsonl` writes: one JSON object per line
//! (`intake.events.RunEventLine` in `ess/intake/domains/events.yaml`), built from the generated
//! `intake.events` types.
//!
//! A line is the record's fields beside `schema_version` and `kind`, the union's tag. A counter or
//! field the run does not have is absent, never `null` or zero. The stream is a projection of what
//! the run does; it adds no authority and is not evidence. Every write is flushed, so a supervisor
//! reads each record as it happens, and [`EventStream::finish`] writes the one terminal record,
//! last.
//!
//! An agent session a host runs through the session executor ([`EventStream::sessions`]) is one
//! `Session` record: the session's record named by its `trace-ir/1` digest and source, with
//! metaharness's `trace-ir-summary/1` of it, or why it does not project. The session's own turns,
//! tool calls and usage get no `Turn`, `ToolCall` or `Usage` record; those are Loom's own model
//! calls ([`EventStream::observe`]).

use std::io::{self, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use b10x_loom_commission::model::responsibility::{
    Commission, ExecutorOutcome, ExecutorStep, Frontier, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_intake_slice::run::{SliceRun, StopReason};
use intake_model::context::RequestPhase;
use intake_model::events::{
    ApprovalEvent, RouteEvent, RunEvent, RunEventLine, SchemaVersion, SessionEvent, SessionModel,
    SessionToolCall, TerminalEvent, ToolCallEvent, TurnEvent, UsageEvent,
};
use intake_model::primitives::Decimal;
use llm_core::{
    BoxFuture, Cancel, Capabilities, Error, Model, Provenance, StopReason as TurnStop, StreamSink,
    TurnOutcome, TurnRequest,
};
use loom_sdk::session::metaharness::protocol::TraceIrSummary;
use loom_sdk::session::{SessionExecutor, SessionTrace};
use serde_json::{Map, Number, Value, json};

use crate::run_exit_status;

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

    /// `executor`, writing one `Session` record for each agent session it runs ([`Self::session`])
    /// once the session ended; it answers exactly as the executor it wraps. A call that started no
    /// session writes none.
    pub fn sessions<'e>(&self, executor: &'e SessionExecutor) -> Sessions<'e> {
        Sessions {
            inner: executor,
            stream: self.clone(),
        }
    }

    /// The `Session` record of one agent session: `trace`'s digest and source, with every field
    /// of its summary the record states, or, for a record metaharness did not project, why.
    ///
    /// # Errors
    /// The stream cannot be written.
    pub fn session(&self, trace: &SessionTrace) -> io::Result<()> {
        self.emit(&RunEvent::Session(session_event(trace)))
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
    /// terminal record. Answers the exit status the process returns: [`run_exit_status`] of the
    /// run, or [`FAILED`] for a run that failed with the given message. The terminal record of a
    /// `NoLocalExecutor` stop carries the actions no local executor performs
    /// ([`SliceRun::unperformed`]) as `unperformed_actions`; every other record leaves it absent.
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
                    exit_status: i64::from(run_exit_status(run)),
                    protocol: Some(run.protocol.clone()),
                    steps: Some(i64::try_from(run.steps).unwrap_or(i64::MAX)),
                    error: None,
                    unperformed_actions: (run.stop_reason == StopReason::NoLocalExecutor
                        && !run.unperformed.is_empty())
                    .then(|| run.unperformed.clone()),
                }
            }
            Err(error) => TerminalEvent {
                stop_reason: None,
                exit_status: i64::from(FAILED),
                protocol: None,
                steps: None,
                error: Some(error.to_owned()),
                unperformed_actions: None,
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
        RunEvent::Session(event) => ("Session", session_fields(event)),
        RunEvent::Terminal(event) => (
            "Terminal",
            json!({
                "stop_reason": event.stop_reason.map(|reason| format!("{reason:?}")),
                "exit_status": event.exit_status,
                "protocol": event.protocol,
                "steps": event.steps,
                "error": event.error,
                "unperformed_actions": event.unperformed_actions,
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

/// A `Session` record's fields as JSON: a cost as the number it is, and an entry of `models` or
/// `tool_calls` without the members it does not state. The line drops the record's own absent
/// fields.
fn session_fields(event: &SessionEvent) -> Value {
    let models = event.models.as_ref().map(|models| {
        models
            .iter()
            .map(|model| {
                stated([
                    ("name", json!(model.name)),
                    ("input_tokens", json!(model.input_tokens)),
                    ("output_tokens", json!(model.output_tokens)),
                    (
                        "cache_read_input_tokens",
                        json!(model.cache_read_input_tokens),
                    ),
                    (
                        "cache_creation_input_tokens",
                        json!(model.cache_creation_input_tokens),
                    ),
                    ("cost_usd", cost(model.cost_usd.as_ref())),
                ])
            })
            .collect::<Vec<_>>()
    });
    let tool_calls = event.tool_calls.as_ref().map(|calls| {
        calls
            .iter()
            .map(|call| {
                stated([
                    ("call_id", json!(call.call_id)),
                    ("name", json!(call.name)),
                    ("answered", json!(call.answered)),
                    ("error", json!(call.error)),
                ])
            })
            .collect::<Vec<_>>()
    });
    stated([
        ("transcript_digest", json!(event.transcript_digest)),
        ("source", json!(event.source)),
        ("adapter", json!(event.adapter)),
        ("model", json!(event.model)),
        ("models", json!(models)),
        ("turns", json!(event.turns)),
        ("tokens_basis", json!(event.tokens_basis)),
        ("input_tokens", json!(event.input_tokens)),
        ("output_tokens", json!(event.output_tokens)),
        (
            "cache_read_input_tokens",
            json!(event.cache_read_input_tokens),
        ),
        (
            "cache_creation_input_tokens",
            json!(event.cache_creation_input_tokens),
        ),
        ("cost_usd", cost(event.cost_usd.as_ref())),
        ("duration_ms", json!(event.duration_ms)),
        ("duration_api_ms", json!(event.duration_api_ms)),
        ("first_timestamp_ms", json!(event.first_timestamp_ms)),
        ("last_timestamp_ms", json!(event.last_timestamp_ms)),
        ("is_error", json!(event.is_error)),
        ("stop_reason", json!(event.stop_reason)),
        ("tool_calls", json!(tool_calls)),
        ("unk", json!(event.unk)),
        ("projection_error", json!(event.projection_error)),
    ])
}

/// An object of the `members` that are not `null`.
fn stated<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

/// A cost as the JSON number its decimal spells, `null` when there is none.
fn cost(cost: Option<&Decimal>) -> Value {
    cost.and_then(|Decimal(spelled)| spelled.parse::<Number>().ok())
        .map_or(Value::Null, Value::Number)
}

/// The `Session` record of `trace`: its digest and source, and either every field its summary
/// states or why the record did not project. A summary cost that has no JSON spelling (not a
/// finite number) is not written as absent, which would read as a record that stated none: the
/// summary is refused as a whole, naming the cost.
#[must_use]
pub fn session_event(trace: &SessionTrace) -> SessionEvent {
    let named = |projection_error: String| SessionEvent {
        transcript_digest: trace.transcript_digest.clone(),
        source: trace.source.clone(),
        adapter: None,
        model: None,
        models: None,
        turns: None,
        tokens_basis: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        cost_usd: None,
        duration_ms: None,
        duration_api_ms: None,
        first_timestamp_ms: None,
        last_timestamp_ms: None,
        is_error: None,
        stop_reason: None,
        tool_calls: None,
        unk: None,
        projection_error: Some(projection_error),
    };
    match &trace.summary {
        Ok(summary) => summarized(summary).unwrap_or_else(named),
        Err(why) => named(why.clone()),
    }
}

/// The `Session` record holding `summary`, or why it cannot hold it.
fn summarized(summary: &TraceIrSummary) -> Result<SessionEvent, String> {
    let models = summary
        .models
        .iter()
        .map(|model| {
            Ok(SessionModel {
                name: model.name.clone(),
                input_tokens: model.input_tokens.map(count),
                output_tokens: model.output_tokens.map(count),
                cache_read_input_tokens: model.cache_read_input_tokens.map(count),
                cache_creation_input_tokens: model.cache_creation_input_tokens.map(count),
                cost_usd: model.cost_usd.map(decimal).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let tool_calls = summary
        .tool_calls
        .iter()
        .map(|call| SessionToolCall {
            call_id: call.call_id.clone(),
            name: call.name.clone(),
            answered: call.answered,
            error: call.error,
        })
        .collect();
    let tokens_basis = summary.tokens_basis.map(|basis| {
        serde_json::to_value(basis)
            .ok()
            .and_then(|spelled| spelled.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| format!("{basis:?}"))
    });
    Ok(SessionEvent {
        transcript_digest: Some(summary.transcript_digest.clone()),
        source: Some(summary.source.to_owned()),
        adapter: summary.adapter.clone(),
        model: summary.model.clone(),
        models: Some(models),
        turns: summary.turns.map(count),
        tokens_basis,
        input_tokens: summary.input_tokens.map(count),
        output_tokens: summary.output_tokens.map(count),
        cache_read_input_tokens: summary.cache_read_input_tokens.map(count),
        cache_creation_input_tokens: summary.cache_creation_input_tokens.map(count),
        cost_usd: summary.cost_usd.map(decimal).transpose()?,
        duration_ms: summary.duration_ms.map(count),
        duration_api_ms: summary.duration_api_ms.map(count),
        first_timestamp_ms: summary.first_timestamp_ms,
        last_timestamp_ms: summary.last_timestamp_ms,
        is_error: summary.is_error,
        stop_reason: summary.stop_reason.clone(),
        tool_calls: Some(tool_calls),
        unk: Some(count(summary.unk)),
        projection_error: None,
    })
}

/// A count as the integer it is written as; one past the largest is the largest.
fn count(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A cost as the decimal JSON spells it, or why it has no JSON spelling.
fn decimal(cost: f64) -> Result<Decimal, String> {
    Number::from_f64(cost)
        .map(|number| Decimal(number.to_string()))
        .ok_or_else(|| {
            format!(
                "a summary cost of {cost} has no JSON spelling, and writing it as absent would \
                 read as a record that stated no cost"
            )
        })
}

/// A session executor whose sessions are recorded on an [`EventStream`], one `Session` record
/// each, after the session ended and before its step is answered; it answers exactly as the
/// executor it wraps. A write failure is kept by the stream and answered by
/// [`EventStream::finish`]; the run goes on.
pub struct Sessions<'e> {
    inner: &'e SessionExecutor,
    stream: EventStream,
}

impl AgentExecutor for Sessions<'_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &Frontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let step = self.inner.run_with_usage(commission, frontier);
        if let Some(trace) = self.inner.last_report().and_then(|report| report.trace) {
            let _ = self.stream.session(&trace);
        }
        step
    }
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
                unperformed_actions: None,
            }),
        });
        assert_eq!(
            value,
            json!({"schema_version": 1, "kind": "Terminal", "exit_status": 1, "error": "no login"})
        );
    }

    fn unprojected(why: &str) -> SessionTrace {
        SessionTrace {
            transcript_digest: Some("ab".repeat(32)),
            source: Some("codex-rollout".to_owned()),
            summary: Err(why.to_owned()),
        }
    }

    #[test]
    fn a_session_line_writes_a_cost_as_a_number_and_omits_what_an_entry_does_not_state() {
        let mut event = session_event(&unprojected("unused"));
        event.projection_error = None;
        event.cost_usd = Some(Decimal("0.0123".to_owned()));
        event.models = Some(vec![SessionModel {
            name: "a-model".to_owned(),
            input_tokens: Some(3),
            output_tokens: None,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            cost_usd: Some(Decimal("1e-7".to_owned())),
        }]);
        event.tool_calls = Some(vec![SessionToolCall {
            call_id: None,
            name: Some("Read".to_owned()),
            answered: false,
            error: None,
        }]);
        event.unk = Some(0);
        let value = line(&RunEventLine {
            schema_version: SchemaVersion::V1,
            event: RunEvent::Session(event),
        });
        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "kind": "Session",
                "transcript_digest": "ab".repeat(32),
                "source": "codex-rollout",
                "models": [{"name": "a-model", "input_tokens": 3, "cost_usd": 1e-7}],
                "cost_usd": 0.0123,
                "tool_calls": [{"name": "Read", "answered": false}],
                "unk": 0,
            })
        );
    }

    #[test]
    fn an_unprojected_record_is_named_with_why_and_states_nothing_else() {
        let value = line(&RunEventLine {
            schema_version: SchemaVersion::V1,
            event: RunEvent::Session(session_event(&unprojected("no record"))),
        });
        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "kind": "Session",
                "transcript_digest": "ab".repeat(32),
                "source": "codex-rollout",
                "projection_error": "no record",
            })
        );
    }

    #[test]
    fn a_cost_with_no_json_spelling_is_named_never_written_absent() {
        for cost in [f64::NAN, f64::INFINITY] {
            let refusal = decimal(cost).expect_err("no spelling");
            assert!(refusal.contains("no JSON spelling"), "{refusal}");
        }
        assert_eq!(decimal(0.0123), Ok(Decimal("0.0123".to_owned())));
    }
}
