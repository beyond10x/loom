//! The model a plugin turn runs on: an llm [`Model`] behind Loom's harness [`ModelPort`].
//!
//! The command line builds `llm_core` models ([`crate::model_catalog`], or a Codex model), while
//! a plugin turn runs Loom's governed model loop, which talks to a harness [`ModelPort`].
//! [`LlmModelPort`] is the bridge: each [`ModelPort::turn`] is exactly one [`Model::turn`] on a
//! runtime the port owns, never retried here (the loop owns retries) and never moved to another
//! target.
//!
//! A request is carried over field by field: instructions, items, tools (name, description and
//! input schema; the harness's approval and envelope are the loop's business and stay behind),
//! the output bound, sampling and tool choice. The request's model is the bound model's own name,
//! as llm requires. An opaque item the model returned crosses back whole, as the llm item it is,
//! under the wire id [`WIRE`]; an opaque item of any other wire is refused before any call.
//! Stream events are collected during the call and handed to the loop's sink when it returns.
//! An llm error becomes the [`WireError`] of the same class, keeping whether it may be retried.

use std::sync::Arc;

use llm_core::{Cancel, Model, VecSink};
use loom_sdk::loom::harness::wire::{
    self, ModelPort, StreamSink, TurnOutcome, TurnRequest, WireError, WireErrorCode, WireId,
};

/// The wire id of every [`LlmModelPort`].
pub const WIRE: &str = "llm-model";

/// The most stream events one turn collects before the call fails.
const MAX_EVENTS: usize = 65_536;
/// The most bytes of stream events one turn collects before the call fails.
const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

/// A harness [`ModelPort`] over an llm [`Model`].
pub struct LlmModelPort {
    model: Arc<dyn Model>,
    wire: WireId,
    runtime: tokio::runtime::Runtime,
}

impl LlmModelPort {
    /// The port over `model`, with a runtime of its own.
    ///
    /// # Errors
    /// No runtime could be built.
    pub fn new(model: Arc<dyn Model>) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("no runtime for the model: {error}"))?;
        Ok(Self {
            model,
            wire: WireId::new(WIRE).map_err(|error| error.to_string())?,
            runtime,
        })
    }

    /// The name of the bound model, which every request carries.
    pub fn model_name(&self) -> String {
        self.model.provenance().model.as_str().to_owned()
    }

    fn request(&self, request: &TurnRequest) -> Result<llm_core::TurnRequest, WireError> {
        let items = request
            .items
            .iter()
            .map(|item| self.item(item))
            .collect::<Result<Vec<_>, _>>()?;
        let mut sent = llm_core::TurnRequest::new(self.model_name(), items);
        sent.instructions.clone_from(&request.instructions);
        sent.tools = request
            .tools
            .iter()
            .map(|tool| {
                Ok(llm_core::ToolSpec {
                    name: llm_name(tool.name.as_str())?,
                    description: tool.description.clone(),
                    input_schema: tool.input_schema.clone(),
                })
            })
            .collect::<Result<Vec<_>, WireError>>()?;
        sent.max_output_tokens = request.max_output_tokens;
        sent.sampling = llm_core::Sampling {
            temperature: request.sampling.temperature,
            top_p: request.sampling.top_p,
            reasoning_effort: request.sampling.reasoning_effort.clone(),
        };
        sent.tool_choice = match &request.tool_choice {
            wire::ToolChoice::Auto => llm_core::ToolChoice::Auto,
            wire::ToolChoice::Required => llm_core::ToolChoice::Required,
            wire::ToolChoice::Named(name) => llm_core::ToolChoice::Named(llm_name(name.as_str())?),
        };
        Ok(sent)
    }

    fn item(&self, item: &wire::Item) -> Result<llm_core::Item, WireError> {
        Ok(match item {
            wire::Item::UserText { text } => llm_core::Item::user(text.clone()),
            wire::Item::AssistantText { text } => llm_core::Item::assistant(text.clone()),
            wire::Item::ToolCall(call) => llm_core::Item::ToolCall(llm_core::ToolCall {
                call_id: llm_call(call.call_id.as_str())?,
                name: llm_name(call.name.as_str())?,
                arguments: call.arguments.clone(),
            }),
            wire::Item::ToolResult {
                call_id,
                output,
                failed,
            } => llm_core::Item::ToolResult {
                call_id: llm_call(call_id.as_str())?,
                output: output.clone(),
                failed: *failed,
            },
            wire::Item::Opaque { wire, payload } if *wire == self.wire => {
                serde_json::from_value(payload.clone()).map_err(|error| {
                    WireError::protocol(format!("an opaque `{WIRE}` item is no llm item: {error}"))
                })?
            }
            wire::Item::Opaque { wire, .. } => {
                return Err(WireError::unsupported(format!(
                    "an opaque `{wire}` item cannot be replayed into `{WIRE}`"
                )));
            }
        })
    }

    fn outcome(&self, outcome: llm_core::TurnOutcome) -> Result<TurnOutcome, WireError> {
        let stop_reason = match outcome.stop_reason {
            llm_core::StopReason::EndTurn => wire::StopReason::EndTurn,
            llm_core::StopReason::ToolCalls => wire::StopReason::ToolCalls,
            llm_core::StopReason::MaxOutputTokens => wire::StopReason::MaxOutputTokens,
            llm_core::StopReason::Incomplete { reason } => wire::StopReason::Incomplete { reason },
        };
        let items = outcome
            .items
            .into_iter()
            .map(|item| {
                Ok(match item {
                    llm_core::Item::UserText { text } => wire::Item::user(text),
                    llm_core::Item::AssistantText { text } => wire::Item::assistant(text),
                    llm_core::Item::ToolCall(call) => wire::Item::ToolCall(wire::ToolCall {
                        call_id: wire_call(call.call_id.as_str())?,
                        name: wire_name(call.name.as_str())?,
                        arguments: call.arguments,
                    }),
                    llm_core::Item::ToolResult {
                        call_id,
                        output,
                        failed,
                    } => wire::Item::ToolResult {
                        call_id: wire_call(call_id.as_str())?,
                        output,
                        failed,
                    },
                    opaque @ (llm_core::Item::Opaque { .. }
                    | llm_core::Item::UnattributedOpaque { .. }) => wire::Item::Opaque {
                        wire: self.wire.clone(),
                        payload: serde_json::to_value(&opaque).map_err(|error| {
                            WireError::protocol(format!("an opaque item cannot be kept: {error}"))
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, WireError>>()?;
        let observation = &outcome.observation;
        let usage = observation.usage.as_ref().and_then(|usage| {
            Some(wire::Usage {
                model: observation
                    .upstream_model
                    .as_ref()
                    .unwrap_or(&observation.binding.model)
                    .as_str()
                    .to_owned(),
                input_tokens: usage.input_tokens?,
                output_tokens: usage.output_tokens?,
                cached_input_tokens: usage.cached_input_tokens.unwrap_or(0),
                cache_creation_input_tokens: usage.cache_creation_input_tokens,
            })
        });
        Ok(TurnOutcome {
            stop_reason,
            items,
            usage,
        })
    }
}

impl ModelPort for LlmModelPort {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        let sent = self.request(request)?;
        let mut events = VecSink::new(MAX_EVENTS, MAX_EVENT_BYTES);
        let cancel = Cancel::new();
        let answered = self
            .runtime
            .block_on(self.model.turn(&sent, &mut events, &cancel));
        for event in events.into_events() {
            let forwarded = match event {
                llm_core::StreamEvent::TextDelta { text } => wire::StreamEvent::TextDelta { text },
                llm_core::StreamEvent::ToolArgumentsDelta { call_id, delta } => {
                    match wire_call(call_id.as_str()) {
                        Ok(call_id) => wire::StreamEvent::ToolArgumentsDelta { call_id, delta },
                        Err(_) => continue,
                    }
                }
                llm_core::StreamEvent::ReasoningDelta { text } => {
                    wire::StreamEvent::ReasoningDelta { text }
                }
                llm_core::StreamEvent::Warning { code, message } => {
                    wire::StreamEvent::Warning { code, message }
                }
                llm_core::StreamEvent::ToolCallStarted { .. } => continue,
            };
            sink.emit(forwarded);
        }
        self.outcome(answered.map_err(wire_error)?)
    }
}

/// The [`WireError`] of an llm error's class, keeping whether it may be retried.
fn wire_error(error: llm_core::Error) -> WireError {
    let code = match error.code {
        llm_core::ErrorCode::InvalidRequest | llm_core::ErrorCode::Protocol => {
            WireErrorCode::Protocol
        }
        llm_core::ErrorCode::Transport
        | llm_core::ErrorCode::Deadline
        | llm_core::ErrorCode::Unavailable => WireErrorCode::Transport,
        llm_core::ErrorCode::Unauthorized => WireErrorCode::Unauthorized,
        llm_core::ErrorCode::RateLimited => WireErrorCode::RateLimited,
        llm_core::ErrorCode::Refused => WireErrorCode::Refused,
        llm_core::ErrorCode::TooLarge => WireErrorCode::TooLarge,
        llm_core::ErrorCode::Unsupported => WireErrorCode::Unsupported,
        llm_core::ErrorCode::Cancelled => WireErrorCode::Cancelled,
    };
    WireError::new(code, error.message, error.retriable)
}

fn llm_name(name: &str) -> Result<llm_core::ToolName, WireError> {
    llm_core::ToolName::new(name)
        .map_err(|_| WireError::protocol(format!("`{name}` is no llm tool name")))
}

fn llm_call(id: &str) -> Result<llm_core::CallId, WireError> {
    llm_core::CallId::new(id).map_err(|_| WireError::protocol(format!("`{id}` is no llm call id")))
}

fn wire_name(name: &str) -> Result<wire::ToolName, WireError> {
    wire::ToolName::new(name)
        .map_err(|_| WireError::protocol(format!("the model called `{name}`, no tool name")))
}

fn wire_call(id: &str) -> Result<wire::CallId, WireError> {
    wire::CallId::new(id)
        .map_err(|_| WireError::protocol(format!("the model named the call `{id}`, no call id")))
}
