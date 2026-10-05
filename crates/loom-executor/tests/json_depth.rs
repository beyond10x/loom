// SPDX-License-Identifier: Apache-2.0

//! `story:json-depth-bound`: every place Loom decodes JSON a model or a provider sent refuses a
//! document nested past serde_json's 128-level limit with a typed error, under a `--workspace`
//! build too.
//!
//! A `cargo test --workspace` build compiles serde_json with `unbounded_depth` (ESS's
//! `ess-conformance` asks for it, and Cargo unifies features across one invocation). That feature
//! adds `Deserializer::disable_recursion_limit`; nothing here calls it, so the limit is expected to
//! hold. These cases are the observation that it does.
//!
//! The entries, one case each at 129 levels and one far deeper:
//!
//! | entry | where the parse is |
//! |---|---|
//! | [`SseReader::next_payload`] | `harness/http/sse.rs`, every streamed event payload |
//! | `messages::decode_stream` | `harness/messages/mod.rs`, streamed tool-call arguments |
//! | `responses::decode_stream` | `harness/responses/project.rs`, function-call arguments |
//! | [`JsonExchange::post`] | `harness/http/exchange.rs`, a provider's document answer |
//! | [`SessionFile::load`] | `session.rs`, transcript replay of a filed session |
//!
//! Each case runs on a thread with a [`STACK_BYTES`] stack. A refused parse needs almost none of
//! it. A parse that has lost its limit would then succeed on the deep document, instead of
//! overflowing the test thread's stack and aborting the binary, so a regression shows up as a
//! failed assertion naming its entry. The decoded value is also dropped on that thread, because
//! dropping a deeply nested `Value` recurses too.

use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use b10x_loom_executor::harness::http::{
    FailureBody, Framing, JsonExchange, JsonPost, MAX_EXCHANGE_BODY_BYTES, SseReader,
};
use b10x_loom_executor::harness::messages;
use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::wire::{
    MAX_TOOL_ARGUMENT_BYTES, TurnOutcome, VecSink, WireError, WireErrorCode,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::SessionId;
use b10x_loom_executor::session::{SessionError, SessionFile};
use serde_json::{Value, json};

/// One level past serde_json's default limit of 128.
const JUST_OVER: usize = 129;

/// Far past it: deep enough to exhaust a test thread's stack if the limit were gone.
const DEEP: usize = 100_000;

/// The stack each case runs on. Large enough that an unlimited parse of [`DEEP`] levels, and the
/// drop of what it built, finish rather than overflow.
const STACK_BYTES: usize = 1 << 30;

/// The text serde_json gives its depth refusal.
const DEPTH_REFUSAL: &str = "recursion limit exceeded";

/// `depth` nested arrays, closed, so the document is valid JSON and only its depth is wrong.
fn nested(depth: usize) -> String {
    let mut text = String::with_capacity(depth * 2);
    text.extend(std::iter::repeat_n('[', depth));
    text.extend(std::iter::repeat_n(']', depth));
    text
}

/// `{"deep": <depth nested arrays>}`: a JSON object, which is what tool arguments must be.
fn nested_object(depth: usize) -> String {
    format!("{{\"deep\":{}}}", nested(depth))
}

/// The deepest closed nesting whose object form fits in `bytes`.
fn deepest_within(bytes: usize) -> usize {
    (bytes - "{\"deep\":}".len()) / 2
}

/// Runs `case` on a thread with [`STACK_BYTES`] of stack, re-raising its panic here.
fn on_large_stack(case: impl FnOnce() + Send + 'static) {
    let outcome = std::thread::Builder::new()
        .name("json-depth-case".to_owned())
        .stack_size(STACK_BYTES)
        .spawn(case)
        .expect("a case thread starts")
        .join();
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

fn assert_depth_refusal<T: std::fmt::Debug>(
    entry: &str,
    depth: usize,
    outcome: Result<T, WireError>,
    code: WireErrorCode,
) {
    match outcome {
        Ok(decoded) => panic!(
            "{entry}: JSON nested {depth} levels deep was accepted, not refused: {:.200}",
            format!("{decoded:?}")
        ),
        Err(error) => {
            assert_eq!(
                error.code, code,
                "{entry}: {depth} levels refused with the wrong code: {error:?}"
            );
            assert!(
                error.message.contains(DEPTH_REFUSAL),
                "{entry}: {depth} levels refused, but not for depth: {error:?}"
            );
        }
    }
}

/// One streamed `data:` frame per payload.
fn sse(payloads: &[Value]) -> Vec<u8> {
    let mut body = String::new();
    for payload in payloads {
        body.push_str("data: ");
        body.push_str(&payload.to_string());
        body.push_str("\n\n");
    }
    body.into_bytes()
}

// --- harness/http/sse.rs: every streamed payload ------------------------------------------------

fn sse_payload_case(depth: usize, framing: Framing) {
    on_large_stack(move || {
        let body = format!("data: {}\n\n", nested(depth));
        let mut reader = SseReader::new(body.as_bytes(), framing);
        assert_depth_refusal(
            &format!("SseReader::next_payload ({framing:?})"),
            depth,
            reader.next_payload(),
            WireErrorCode::Protocol,
        );
    });
}

#[test]
fn a_streamed_payload_past_the_limit_is_refused() {
    sse_payload_case(JUST_OVER, Framing::PayloadsOnly);
    sse_payload_case(JUST_OVER, Framing::DoneSentinel);
}

#[test]
fn a_streamed_payload_far_past_the_limit_is_refused() {
    sse_payload_case(DEEP, Framing::PayloadsOnly);
    sse_payload_case(DEEP, Framing::DoneSentinel);
}

// --- the two wires' decoders, whole streams --------------------------------------------------------

fn messages_stream(payloads: &[Value]) -> Result<TurnOutcome, WireError> {
    let body = sse(payloads);
    let mut sink = VecSink::new();
    messages::decode_stream("depth-model", body.as_slice(), &mut sink)
}

fn responses_stream(payloads: &[Value]) -> Result<TurnOutcome, WireError> {
    let body = sse(payloads);
    let mut sink = VecSink::new();
    responses::decode_stream("depth-model", body.as_slice(), &mut sink)
}

/// A whole stream whose one event payload is nested too deep, on each wire's live decoder.
fn stream_payload_case(depth: usize) {
    on_large_stack(move || {
        let mut frame = sse(&[json!({"type": "ping"})]);
        frame.extend_from_slice(format!("data: {}\n\n", nested(depth)).as_bytes());
        let mut sink = VecSink::new();
        assert_depth_refusal(
            "messages::decode_stream (event payload)",
            depth,
            messages::decode_stream("depth-model", frame.as_slice(), &mut sink),
            WireErrorCode::Protocol,
        );
        let mut frame = sse(&[json!({"type": "response.created"})]);
        frame.extend_from_slice(format!("data: {}\n\n", nested(depth)).as_bytes());
        let mut sink = VecSink::new();
        assert_depth_refusal(
            "responses::decode_stream (event payload)",
            depth,
            responses::decode_stream("depth-model", frame.as_slice(), &mut sink),
            WireErrorCode::Protocol,
        );
    });
}

#[test]
fn a_decoded_stream_event_past_the_limit_is_refused() {
    stream_payload_case(JUST_OVER);
}

#[test]
fn a_decoded_stream_event_far_past_the_limit_is_refused() {
    stream_payload_case(DEEP);
}

// --- harness/messages/mod.rs: streamed tool-call arguments -----------------------------------------

fn messages_arguments_case(depth: usize) {
    on_large_stack(move || {
        let arguments = nested_object(depth);
        assert!(
            arguments.len() <= MAX_TOOL_ARGUMENT_BYTES,
            "inside the byte bound"
        );
        let outcome = messages_stream(&[
            json!({"type": "message_start", "message": {"model": "depth-model",
                "usage": {"input_tokens": 1, "output_tokens": 0}}}),
            json!({"type": "content_block_start", "index": 0, "content_block": {
                "type": "tool_use", "id": "toolu_1", "name": "workspace_read", "input": {},
            }}),
            json!({"type": "content_block_delta", "index": 0, "delta": {
                "type": "input_json_delta", "partial_json": arguments,
            }}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"},
                "usage": {"output_tokens": 1}}),
            json!({"type": "message_stop"}),
        ]);
        assert_depth_refusal(
            "messages::decode_stream (tool-call arguments)",
            depth,
            outcome,
            WireErrorCode::Protocol,
        );
    });
}

#[test]
fn streamed_tool_arguments_past_the_limit_are_refused() {
    messages_arguments_case(JUST_OVER);
}

#[test]
fn streamed_tool_arguments_far_past_the_limit_are_refused() {
    messages_arguments_case(deepest_within(MAX_TOOL_ARGUMENT_BYTES));
}

// --- harness/responses/project.rs: function-call arguments -----------------------------------------

fn responses_arguments_case(depth: usize) {
    on_large_stack(move || {
        let arguments = nested_object(depth);
        assert!(
            arguments.len() <= MAX_TOOL_ARGUMENT_BYTES,
            "inside the byte bound"
        );
        let call = json!({"type": "function_call", "id": "fc_1", "call_id": "call_1",
            "name": "workspace_read", "arguments": arguments});
        let outcome = responses_stream(&[
            json!({"type": "response.created"}),
            json!({"type": "response.output_item.added", "item":
                {"type": "function_call", "id": "fc_1", "call_id": "call_1"}}),
            json!({"type": "response.output_item.done", "item": call}),
            json!({"type": "response.completed", "response": {"status": "completed",
                "model": "depth-model", "output": [call],
                "usage": {"input_tokens": 1, "output_tokens": 1}}}),
        ]);
        assert_depth_refusal(
            "responses::decode_stream (function-call arguments)",
            depth,
            outcome,
            WireErrorCode::Protocol,
        );
    });
}

#[test]
fn function_call_arguments_past_the_limit_are_refused() {
    responses_arguments_case(JUST_OVER);
}

#[test]
fn function_call_arguments_far_past_the_limit_are_refused() {
    responses_arguments_case(deepest_within(MAX_TOOL_ARGUMENT_BYTES));
}

// --- harness/http/exchange.rs: a provider's document answer ----------------------------------------

/// Serves `body` once as a 200 answer and posts to it through [`JsonExchange`].
fn exchange_answer(body: Vec<u8>) -> Result<Value, WireError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let url = format!("http://{}/document", listener.local_addr().expect("addr"));
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("a request");
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request).expect("request bytes");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n",
            body.len()
        )
        .expect("headers");
        stream.write_all(&body).expect("body");
    });
    let outcome = JsonExchange::new().expect("a client").post(&JsonPost {
        who: "the depth peer",
        url: &url,
        body: &json!({}),
        failure_body: FailureBody::Omit,
    });
    server.join().expect("the server answered");
    outcome
}

fn exchange_case(depth: usize) {
    on_large_stack(move || {
        let body = nested_object(depth);
        assert!(
            body.len() <= MAX_EXCHANGE_BODY_BYTES,
            "inside the byte bound"
        );
        assert_depth_refusal(
            "JsonExchange::post",
            depth,
            exchange_answer(body.into_bytes()),
            WireErrorCode::Refused,
        );
    });
}

#[test]
fn a_provider_document_past_the_limit_is_refused() {
    exchange_case(JUST_OVER);
}

#[test]
fn a_provider_document_far_past_the_limit_is_refused() {
    exchange_case(deepest_within(MAX_EXCHANGE_BODY_BYTES));
}

// --- session.rs: transcript replay -------------------------------------------------------------

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("json_depth")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch");
    }
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn session_case(depth: usize, name: &'static str) {
    on_large_stack(move || {
        let sessions = scratch(name);
        let id_text = "00000000-0000-4000-8000-00000000d0e9";
        let text = format!(
            "{{\"version\":2,\"id\":\"{id_text}\",\"items\":[{}]}}",
            nested(depth)
        );
        std::fs::write(sessions.join(format!("{id_text}.json")), text).expect("a session file");
        let outcome = SessionFile::load(&sessions, &SessionId(Uuid(id_text.to_owned())));
        match outcome {
            Ok(session) => panic!(
                "SessionFile::load: a session nested {depth} levels deep was replayed, not \
                 refused: {:.200}",
                format!("{session:?}")
            ),
            Err(SessionError::Refused(message)) => assert!(
                message.contains(DEPTH_REFUSAL),
                "SessionFile::load: {depth} levels refused, but not for depth: {message}"
            ),
            Err(other) => panic!(
                "SessionFile::load: {depth} levels refused with the wrong variant: {other:?}"
            ),
        }
    });
}

#[test]
fn a_filed_session_past_the_limit_is_refused() {
    session_case(JUST_OVER, "just_over");
}

#[test]
fn a_filed_session_far_past_the_limit_is_refused() {
    session_case(DEEP, "deep");
}
