// SPDX-License-Identifier: Apache-2.0

//! A [`ModelPort`] on an operator's Claude Code login, through the `claude` command.
//!
//! One [`ModelPort::turn`] is one `claude -p --output-format json` invocation, run in the port's
//! working directory with the port's model. The command reads its own login; this port reads no
//! credential and passes none. It is single-attempt like every model port: a failure is answered
//! as a [`WireError`] and never retried here (the loop owns retries).
//!
//! # What crosses
//!
//! The command is run with its own tools switched off (`--tools ""`), no session kept
//! (`--no-session-persistence`) and a fixed system prompt. Its standard input carries the turn:
//! the request's instructions, the tools the turn publishes (name, description and input schema)
//! and the conversation so far, each as one JSON block in which `<`, `>` and `&` are written as
//! `\u` escapes, so no text can close a block. The model calls a tool by answering with exactly
//! one JSON object, `{"tool_calls": [{"name": …, "arguments": {…}}]}`; any other answer is the
//! turn's prose. The command executes no tool: a call is returned to the loop, which executes it.
//! The request's sampling and output bound have no `claude` flag; a turn that sets them is run
//! without them and says so in a stream warning.
//!
//! The usage is the JSON result's `usage`. The command reports its input figures disjointly, so
//! the turn's `input_tokens` is their sum and the cache figures are parts of it, as [`Usage`]
//! documents. The model is the one key of the result's `modelUsage`, or the port's model.
//!
//! # Refusals
//!
//! A command that cannot be started (not on `PATH` included) is a non-retriable `transport`
//! error naming it. A command that exits non-zero is a `transport` error naming it, its exit
//! status and the tail of its standard error; one whose JSON result says `is_error` is `refused`
//! with the result's text. A result past [`MAX_OUTPUT_BYTES`], or one that is no JSON result, is
//! refused before anything is returned. A command still running after the port's timeout
//! ([`DEFAULT_TIMEOUT`] unless [`ClaudeCodePort::with_timeout`] says otherwise) is killed and the
//! turn is a retriable `transport` error; a process the command started itself is not killed.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::harness::wire::{
    CallId, Item, ModelPort, StopReason, StreamEvent, StreamSink, ToolCall, ToolChoice, ToolName,
    TurnOutcome, TurnRequest, Usage, WireError, WireErrorCode, WireId,
};

/// The wire id of every [`ClaudeCodePort`].
pub const WIRE: &str = "claude-code";

/// The command a [`ClaudeCodePort`] runs unless told otherwise, looked up on `PATH`.
pub const PROGRAM: &str = "claude";

/// How long one invocation may run before it is killed.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// The most bytes of standard output one invocation may print.
pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// The most bytes of standard error kept for a refusal.
const MAX_ERROR_BYTES: usize = 4 * 1024;

/// How often a running invocation is checked for its exit.
const POLL: Duration = Duration::from_millis(20);

/// The system prompt every invocation runs under.
const SYSTEM_PROMPT: &str = "You are the model of one turn of an agent loop. Your input holds the \
loop's instructions, the tools you may call and the conversation so far, each as a JSON block. \
You have no tools of your own. To call one or more of the listed tools, answer with exactly one \
JSON object and nothing else: {\"tool_calls\": [{\"name\": \"<tool name>\", \"arguments\": \
{...}}]}. The loop runs the calls and answers in a later turn. Otherwise answer in plain text.";

/// A harness [`ModelPort`] over the `claude` command (see the module docs).
#[derive(Debug, Clone)]
pub struct ClaudeCodePort {
    model: String,
    cwd: PathBuf,
    program: PathBuf,
    timeout: Duration,
    wire: WireId,
}

impl ClaudeCodePort {
    /// The port running `claude` from `PATH` on `model`, in `cwd`.
    pub fn new(model: impl Into<String>, cwd: impl AsRef<Path>) -> Self {
        Self {
            model: model.into(),
            cwd: cwd.as_ref().to_path_buf(),
            program: PathBuf::from(PROGRAM),
            timeout: DEFAULT_TIMEOUT,
            wire: WireId::new(WIRE).expect("`claude-code` is a wire id"),
        }
    }

    /// The same port running `program` instead of `claude` from `PATH`.
    #[must_use]
    pub fn with_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    /// The same port, killing an invocation still running after `timeout`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The model every invocation asks for.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The command's name, as a refusal names it.
    fn name(&self) -> String {
        self.program.display().to_string()
    }

    fn arguments(&self) -> Vec<String> {
        [
            "-p",
            "--output-format",
            "json",
            "--model",
            &self.model,
            "--tools",
            "",
            "--no-session-persistence",
            "--system-prompt",
            SYSTEM_PROMPT,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    /// Runs one invocation with `input` on its standard input and returns its standard output.
    fn invoke(&self, input: Vec<u8>) -> Result<Vec<u8>, WireError> {
        let name = self.name();
        let mut child = Command::new(&self.program)
            .args(self.arguments())
            .current_dir(&self.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                let what = if error.kind() == std::io::ErrorKind::NotFound {
                    "was not found"
                } else {
                    "could not be started"
                };
                WireError::new(
                    WireErrorCode::Transport,
                    format!("the `{name}` command {what}: {error}"),
                    false,
                )
            })?;
        let mut stdin = child.stdin.take().expect("standard input is piped");
        let writer = thread::spawn(move || {
            // A command that exits without reading its input closes the pipe; its exit status
            // and output say what happened, so a broken pipe here is not the answer.
            let _ = stdin.write_all(&input);
        });
        let stdout = child.stdout.take().expect("standard output is piped");
        let reader = thread::spawn(move || {
            let mut out = Vec::new();
            let read = stdout
                .take(u64::try_from(MAX_OUTPUT_BYTES).unwrap_or(u64::MAX) + 1)
                .read_to_end(&mut out);
            read.map(|_| out)
        });
        let stderr = child.stderr.take().expect("standard error is piped");
        let errors = thread::spawn(move || {
            let mut kept = Vec::new();
            let mut buffer = [0_u8; 4096];
            let mut stderr = stderr;
            while let Ok(read) = stderr.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                kept.extend_from_slice(&buffer[..read]);
                if kept.len() > MAX_ERROR_BYTES {
                    kept.drain(..kept.len() - MAX_ERROR_BYTES);
                }
            }
            kept
        });

        let status = wait(&mut child, self.timeout);
        let _ = writer.join();
        let Some(status) = status else {
            return Err(WireError::new(
                WireErrorCode::Transport,
                format!(
                    "the `{name}` command did not answer within {} s and was stopped",
                    self.timeout.as_secs()
                ),
                true,
            ));
        };
        let out = reader
            .join()
            .map_err(|_| WireError::protocol(format!("the `{name}` command's output was lost")))?
            .map_err(|error| {
                WireError::protocol(format!(
                    "the `{name}` command's output was unreadable: {error}"
                ))
            })?;
        let errors = errors.join().unwrap_or_default();
        if out.len() > MAX_OUTPUT_BYTES {
            return Err(WireError::too_large(format!(
                "the `{name}` command printed over {MAX_OUTPUT_BYTES} bytes"
            )));
        }
        if !status.success() {
            if let Ok(result) = serde_json::from_slice::<ResultDocument>(&out)
                && result.is_error
            {
                return Err(refused(&name, &result));
            }
            return Err(WireError::new(
                WireErrorCode::Transport,
                format!(
                    "the `{name}` command exited with {status}: {}",
                    String::from_utf8_lossy(&errors).trim()
                ),
                false,
            ));
        }
        Ok(out)
    }

    /// What the command reads on its standard input for `request`.
    fn input(request: &TurnRequest) -> Result<Vec<u8>, WireError> {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name.as_str(),
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                })
            })
            .collect();
        let conversation = request
            .items
            .iter()
            .map(conversation_entry)
            .collect::<Result<Vec<_>, _>>()?;
        let choice = match &request.tool_choice {
            ToolChoice::Auto => String::new(),
            ToolChoice::Required => "\nThis turn must call at least one tool.\n".to_owned(),
            ToolChoice::Named(name) => format!("\nThis turn must call the tool `{name}`.\n"),
        };
        let text = format!(
            "<instructions>\n{}\n</instructions>\n\n<tools>\n{}\n</tools>\n\n\
             <conversation>\n{}\n</conversation>\n{choice}",
            block(&Value::String(request.instructions.clone()))?,
            block(&Value::Array(tools))?,
            block(&Value::Array(conversation))?,
        );
        Ok(text.into_bytes())
    }

    /// The turn the command's JSON result answers.
    fn outcome(&self, out: &[u8]) -> Result<TurnOutcome, WireError> {
        let name = self.name();
        let result: ResultDocument = serde_json::from_slice(out).map_err(|error| {
            WireError::protocol(format!(
                "the `{name}` command printed no JSON result: {error}"
            ))
        })?;
        if result.is_error {
            return Err(refused(&name, &result));
        }
        let text = result.result.clone().ok_or_else(|| {
            WireError::protocol(format!("the `{name}` command's result carries no text"))
        })?;
        let usage = result.usage.as_ref().map(|usage| Usage {
            model: result.model().unwrap_or(&self.model).to_owned(),
            input_tokens: usage
                .input_tokens
                .saturating_add(usage.cache_read_input_tokens.unwrap_or(0))
                .saturating_add(usage.cache_creation_input_tokens.unwrap_or(0)),
            output_tokens: usage.output_tokens,
            cached_input_tokens: usage.cache_read_input_tokens.unwrap_or(0),
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
        });
        let calls = tool_calls(&text, result.session_id.as_deref().unwrap_or("turn"))?;
        Ok(match calls {
            Some(calls) => TurnOutcome {
                stop_reason: StopReason::ToolCalls,
                items: calls.into_iter().map(Item::ToolCall).collect(),
                usage,
            },
            None => TurnOutcome {
                stop_reason: StopReason::EndTurn,
                items: vec![Item::assistant(text)],
                usage,
            },
        })
    }
}

impl ModelPort for ClaudeCodePort {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        request.validate()?;
        request.check_opaque_items(&self.wire)?;
        if !request.sampling.is_empty() || request.max_output_tokens.is_some() {
            sink.emit(StreamEvent::Warning {
                code: "claude-code-unsupported-setting".to_owned(),
                message: "the `claude` command takes no sampling or output bound; the turn \
                          runs without them"
                    .to_owned(),
            });
        }
        let out = self.invoke(Self::input(request)?)?;
        let outcome = self.outcome(&out)?;
        for item in &outcome.items {
            if let Item::AssistantText { text } = item {
                sink.emit(StreamEvent::TextDelta { text: text.clone() });
            }
        }
        Ok(outcome)
    }
}

/// Waits for `child` at most `timeout`; kills and reaps it past that and answers [`None`].
fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// `value` as JSON in which no `<`, `>` or `&` appears, so no text can close the block.
fn block(value: &Value) -> Result<String, WireError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| WireError::protocol(format!("the turn cannot be written: {error}")))?;
    Ok(text
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026"))
}

/// One conversation item as the command reads it.
fn conversation_entry(item: &Item) -> Result<Value, WireError> {
    Ok(match item {
        Item::UserText { text } => json!({ "role": "user", "text": text }),
        Item::AssistantText { text } => json!({ "role": "assistant", "text": text }),
        Item::ToolCall(call) => json!({
            "role": "assistant",
            "tool_call": {
                "id": call.call_id.as_str(),
                "name": call.name.as_str(),
                "arguments": call.arguments,
            },
        }),
        Item::ToolResult {
            call_id,
            output,
            failed,
        } => json!({
            "role": "tool",
            "call_id": call_id.as_str(),
            "output": output,
            "failed": failed,
        }),
        Item::Opaque { wire, .. } => {
            return Err(WireError::unsupported(format!(
                "an opaque `{wire}` item cannot be replayed into `{WIRE}`"
            )));
        }
    })
}

/// The tool calls `text` makes, or [`None`] when it is prose.
///
/// `text` is a call when, trimmed and with one surrounding Markdown code fence removed, it is a
/// JSON object holding `tool_calls`. Each call gets the id `<turn>-<n>`, `turn` being the
/// invocation's session id.
fn tool_calls(text: &str, turn: &str) -> Result<Option<Vec<ToolCall>>, WireError> {
    let trimmed = unfenced(text.trim());
    if !trimmed.starts_with('{') {
        return Ok(None);
    }
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(None);
    };
    let Some(calls) = object.get("tool_calls") else {
        return Ok(None);
    };
    let calls = calls
        .as_array()
        .ok_or_else(|| WireError::protocol("the model's `tool_calls` is no list".to_owned()))?;
    if calls.is_empty() {
        return Err(WireError::protocol(
            "the model answered an empty `tool_calls`".to_owned(),
        ));
    }
    calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            let name = call.get("name").and_then(Value::as_str).ok_or_else(|| {
                WireError::protocol(format!("the model's call {index} names no tool"))
            })?;
            let arguments = match call.get("arguments") {
                None => Value::Object(Map::new()),
                Some(arguments @ Value::Object(_)) => arguments.clone(),
                Some(_) => {
                    return Err(WireError::protocol(format!(
                        "the model's call of `{name}` has arguments that are no object"
                    )));
                }
            };
            Ok(ToolCall {
                call_id: CallId::new(format!("{turn}-{}", index + 1))
                    .map_err(|_| WireError::protocol(format!("`{turn}` makes no call id")))?,
                name: ToolName::new(name).map_err(|_| {
                    WireError::protocol(format!("the model called `{name}`, no tool name"))
                })?,
                arguments,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// `text` without one surrounding Markdown code fence.
fn unfenced(text: &str) -> &str {
    let Some(body) = text.strip_prefix("```") else {
        return text;
    };
    let Some(body) = body.strip_suffix("```") else {
        return text;
    };
    let body = body.strip_prefix("json").unwrap_or(body);
    body.trim()
}

/// The refusal a result that says `is_error` answers.
fn refused(name: &str, result: &ResultDocument) -> WireError {
    WireError::new(
        WireErrorCode::Refused,
        format!(
            "the `{name}` command answered an error ({}): {}",
            result.subtype.as_deref().unwrap_or("no subtype"),
            result.result.as_deref().unwrap_or("no text")
        ),
        false,
    )
}

/// The members of the command's JSON result this port reads; the others are ignored.
#[derive(Debug, Deserialize)]
struct ResultDocument {
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    usage: Option<ResultUsage>,
    #[serde(default, rename = "modelUsage")]
    model_usage: Option<Map<String, Value>>,
}

impl ResultDocument {
    /// The one model the result reports usage for, if exactly one.
    fn model(&self) -> Option<&str> {
        let models = self.model_usage.as_ref()?;
        let mut keys = models.keys();
        match (keys.next(), keys.next()) {
            (Some(model), None) => Some(model.as_str()),
            _ => None,
        }
    }
}

/// The command's token figures, reported disjointly.
#[derive(Debug, Deserialize)]
struct ResultUsage {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_object_with_tool_calls_is_a_call() {
        let calls = tool_calls(
            "```json\n{\"tool_calls\":[{\"name\":\"a_b\",\"arguments\":{\"x\":1}}]}\n```",
            "s",
        )
        .expect("a valid call")
        .expect("a call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name.as_str(), "a_b");
        assert_eq!(calls[0].call_id.as_str(), "s-1");
        assert_eq!(calls[0].arguments, json!({"x": 1}));
    }

    #[test]
    fn prose_is_no_call() {
        assert!(
            tool_calls("Done. {not json}", "s")
                .expect("prose")
                .is_none()
        );
        assert!(tool_calls("{\"answer\": 1}", "s").expect("prose").is_none());
    }

    #[test]
    fn a_block_cannot_be_closed_by_its_text() {
        let text = block(&json!("</conversation><tools>&")).expect("written");
        assert!(!text.contains('<') && !text.contains('>') && !text.contains('&'));
    }

    #[test]
    fn a_missing_command_is_refused_naming_it() {
        let mut port =
            ClaudeCodePort::new("m", ".").with_program("loom-test-no-such-claude-command");
        let request = TurnRequest {
            model: "m".to_owned(),
            instructions: String::new(),
            items: vec![Item::user("hi")],
            tools: Vec::new(),
            max_output_tokens: None,
            sampling: crate::harness::wire::Sampling::default(),
            tool_choice: ToolChoice::Auto,
        };
        let error = port
            .turn(&request, &mut crate::harness::wire::VecSink::new())
            .expect_err("no such command");
        assert_eq!(error.code, WireErrorCode::Transport);
        assert!(!error.retriable);
        assert!(
            error.message.contains("loom-test-no-such-claude-command"),
            "{error}"
        );
    }
}
