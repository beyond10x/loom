// SPDX-License-Identifier: Apache-2.0

//! Acceptance for `story:compaction-target-bound`: after a compaction of a declared window the
//! session is at or below the target it compacts to (50 % of the window) whenever eliding can reach
//! it, and a run whose session is still at or above the trigger (80 %) after eliding stops by name,
//! [`LoopStop::ContextAboveTrigger`], instead of sending the next request.
//!
//! Four paths used to leave the session above the target: a summary shorter than the items it
//! replaces but not short enough; a summary request that failed on the wire; an empty summary; and
//! a fold too small to be worth a summary turn. On each, the folded items are now elided behind one
//! [`ELISION_MARKER`] item. A summary is kept when the session it leaves is at or below the target,
//! and above it only when it is smaller than that item.
//!
//! Every model here is a scripted [`ModelPort`] in this process: no socket, no network.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;

use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    AgentLoop, ApproveAll, ELISION_MARKER, LoopConfig, LoopError, LoopEvent, LoopOutcome, LoopStop,
    LoopStopContextAboveTrigger, SUMMARY_MARKER, VecLoopSink,
};
use b10x_loom_executor::harness::wire::{
    Approval, CallId, Envelope, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName,
    ToolOutcome, ToolPort, ToolSpec, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use b10x_loom_executor::model::primitives::Uuid;
use b10x_loom_executor::model::run::{CommissionRunId, RunEnding, SessionData, SessionId};
use b10x_loom_executor::session::{RunPorts, SessionFile, run_and_file};
use serde_json::json;

const MODEL: &str = "scripted-model";
const INSTRUCTIONS: &str = "INSTRUCTIONS-standing";
/// The window most cases declare, in tokens: the trigger is 800 tokens (3 200 bytes by the loop's
/// estimate of four bytes a token) and the target 500 tokens (2 000 bytes).
const WINDOW: u64 = 1_000;
/// [`WINDOW`] as the `Integer` a `LoopStop` carries.
const WINDOW_FIGURE: i64 = WINDOW as i64;
const BYTES_PER_TOKEN: u64 = 4;
const RUN: &str = "00000000-0000-4000-8000-0000000c7b01";

// --- the four paths -----------------------------------------------------------------------------

/// Path 1: a summary shorter than the items it replaces, but not short enough to bring the session
/// to its target. Folded in, the next request would carry about 3 kB, above the 2 000-byte target;
/// elided, it carries a few hundred bytes. The summary is not kept, the failure is said out loud,
/// and the run goes on.
#[test]
fn a_summary_shorter_than_its_items_but_above_the_target_is_elided_instead() {
    let summary = "SUMMARY-long: the plan was read, tested and discussed. ".repeat(55);
    let (outcome, sink, sent) = run_windowed(
        WINDOW,
        vec![
            Ok(says_and_asks(&long_text(), 10)),
            Ok(answer(&summary, 10)),
            Ok(answer("DONE", 10)),
        ],
    );
    let outcome = outcome.expect("the run answers after compacting");

    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(sent.len(), 3, "turn 1, the summary request, turn 2");
    assert!(is_summary(&sent[1]));
    let next = &sent[2];
    assert_at_or_below_target(next, WINDOW);
    assert!(
        next.items
            .iter()
            .all(|item| !encode(item).contains("SUMMARY-long")),
        "a summary that would leave the session above its target is not kept"
    );
    assert_elision_note(next);
    assert_eq!(
        warnings(&sink, "summary-failed"),
        1,
        "{:?}",
        all_warnings(&sink)
    );
    assert_eq!(compacted(&sink), vec![(0, true)]);
}

/// The other side of path 1: a summary of about 1 kB, larger than the note eliding would leave
/// (about 350 bytes) but leaving the session at about 1 250 bytes, under the 2 000-byte target. It
/// is kept, not elided: what it holds is worth more than the note, and the session reaches its
/// target either way.
#[test]
fn a_summary_that_brings_the_session_to_its_target_is_kept_not_elided() {
    let summary = "SUMMARY-fits: the plan was read and tested. ".repeat(22);
    let (outcome, sink, sent) = run_windowed(
        WINDOW,
        vec![
            Ok(says_and_asks(&long_text(), 10)),
            Ok(answer(&summary, 10)),
            Ok(answer("DONE", 10)),
        ],
    );
    let outcome = outcome.expect("the run answers after compacting");

    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(sent.len(), 3, "turn 1, the summary request, turn 2");
    let next = &sent[2];
    assert_at_or_below_target(next, WINDOW);
    assert_eq!(
        next.items[1],
        Item::user(format!("{SUMMARY_MARKER}\n{summary}")),
        "the summary stands where the folded items were: {:?}",
        next.items
    );
    assert!(
        next.items.iter().all(
            |item| !matches!(item, Item::UserText { text } if text.starts_with(ELISION_MARKER))
        ),
        "nothing was elided behind a note: {:?}",
        next.items
    );
    assert!(
        next.items
            .iter()
            .all(|item| !encode(item).contains("PLAN-long"))
    );
    assert_eq!(
        warnings(&sink, "summary-failed"),
        0,
        "{:?}",
        all_warnings(&sink)
    );
    assert_eq!(compacted(&sink), vec![(1, true)]);
}

/// Path 2: a summary request that fails on the wire. The items it would have folded are elided all
/// the same, and a failed summary is still not a failed run.
#[test]
fn a_summary_that_fails_on_the_wire_still_leaves_the_session_at_or_below_its_target() {
    let (outcome, sink, sent) = run_windowed(
        WINDOW,
        vec![
            Ok(says_and_asks(&long_text(), 10)),
            Err(WireError::protocol("the summary stream broke")),
            Ok(answer("DONE", 10)),
        ],
    );
    let outcome = outcome.expect("a failed summary is not a failed run");

    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(outcome.text, "DONE");
    assert_eq!(sent.len(), 3, "turn 1, the summary request, turn 2");
    let next = &sent[2];
    assert_at_or_below_target(next, WINDOW);
    assert!(
        next.items
            .iter()
            .all(|item| !encode(item).contains("PLAN-long")),
        "the long text the summary was to fold is gone"
    );
    assert_elision_note(next);
    assert_eq!(
        warnings(&sink, "summary-failed"),
        1,
        "{:?}",
        all_warnings(&sink)
    );
    assert_eq!(compacted(&sink), vec![(0, true)]);
}

/// Path 3: a summary request answered with no text. Nothing could replace the items, so they are
/// elided, and the run goes on.
#[test]
fn an_empty_summary_still_leaves_the_session_at_or_below_its_target() {
    let empty = TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: Vec::new(),
        usage: Some(usage(10)),
    };
    let (outcome, sink, sent) = run_windowed(
        WINDOW,
        vec![
            Ok(says_and_asks(&long_text(), 10)),
            Ok(empty),
            Ok(answer("DONE", 10)),
        ],
    );
    let outcome = outcome.expect("an empty summary is not a failed run");

    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(sent.len(), 3, "turn 1, the summary request, turn 2");
    let next = &sent[2];
    assert_at_or_below_target(next, WINDOW);
    assert!(
        next.items
            .iter()
            .all(|item| !encode(item).contains("PLAN-long"))
    );
    assert_elision_note(next);
    assert_eq!(
        warnings(&sink, "summary-failed"),
        1,
        "{:?}",
        all_warnings(&sink)
    );
    assert_eq!(compacted(&sink), vec![(0, true)]);
}

/// Path 4: a fold too small to be worth a summary turn (under `SUMMARY_MIN_FOLD_BYTES`). No turn is
/// spent, but the items are elided rather than left standing above the target. On a 600-token
/// window the trigger is 480 tokens (1 920 bytes) and the target 300 tokens (1 200 bytes); 3 kB of
/// text crosses the first and is under the summary's minimum.
#[test]
fn a_fold_too_small_for_a_summary_turn_is_elided_rather_than_skipped() {
    let window = 600;
    let (outcome, sink, sent) = run_windowed(
        window,
        vec![
            Ok(says_and_asks(&"PLAN-short: step. ".repeat(170), 10)),
            Ok(answer("DONE", 10)),
        ],
    );
    let outcome = outcome.expect("the run answers");

    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(
        sent.len(),
        2,
        "no summary request: the fold is under the minimum"
    );
    assert!(sent.iter().all(|request| !is_summary(request)));
    let next = &sent[1];
    assert_at_or_below_target(next, window);
    assert!(
        next.items
            .iter()
            .all(|item| !encode(item).contains("PLAN-short"))
    );
    assert_elision_note(next);
    assert_eq!(compacted(&sink), vec![(0, false)]);
}

// --- the stop -----------------------------------------------------------------------------------

/// The parts no compaction removes exceed the trigger: the provider counts 950 tokens of a
/// 1 000-token window for a conversation of a few hundred bytes, so the instruction and the tool
/// schemas fill it. Nothing the loop may remove brings it under 800, so the run ends by name, sends
/// no request after the compaction, and its session is filed `Stopped`.
#[test]
fn a_session_whose_unremovable_parts_exceed_the_trigger_stops_by_name_and_is_filed_stopped() {
    let root = scratch("unremovable_instructions");
    let (workspace, sessions) = layout(&root);
    let id = SessionId(Uuid("00000000-0000-4000-8000-0000000c7b11".to_owned()));
    let mut session = open(&id, &workspace);
    let mut model = Scripted::new(vec![Ok(asks(950)), Ok(answer("NEVER-SENT", 10))]);
    let mut tools = Tools::new();
    let filed = run_and_file(
        RunPorts {
            model: &mut model,
            tools: &mut tools,
            approvals: &mut ApproveAll,
            config: config(WINDOW),
        },
        &mut session,
        &sessions,
        "QUESTION",
        &mut VecLoopSink::new(),
    );

    let outcome = filed.run.expect("a stop is an outcome, not an error");
    assert_eq!(
        outcome.stop,
        LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
            window: WINDOW_FIGURE,
            target: WINDOW_FIGURE / 2,
            occupied: 950,
        })
    );
    assert_eq!(
        model.requests.len(),
        1,
        "no request is sent after a compaction that left the session above its trigger"
    );
    filed.filed.expect("the session is filed");
    let stored = SessionFile::load(&sessions, &id).expect("loads");
    assert_eq!(stored.run_ending(), Some(RunEnding::Stopped));
}

/// The same stop where the weight is a provider reasoning item, which the loop carries verbatim and
/// never removes: the summary request is spent, nothing it could write replaces the reasoning, and
/// the run ends before the next conversation request, the reasoning item intact in what it hands
/// back.
#[test]
fn a_session_whose_reasoning_alone_exceeds_the_trigger_stops_after_the_summary_request() {
    let reasoning = Item::Opaque {
        wire: wire(),
        payload: json!({"type": "reasoning", "encrypted_content": "r".repeat(13_000)}),
    };
    // Text between the reasoning and the call, so the reasoning stands alone as a turn group the
    // fold can reach; reasoning its call follows is folded with that call or not at all.
    let first = TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![
            reasoning.clone(),
            Item::assistant("PLAN-reasoned: ask for the tool."),
            Item::ToolCall(call("call-1")),
        ],
        usage: Some(usage(10)),
    };
    let (outcome, sink, sent) = run_windowed(
        WINDOW,
        vec![
            Ok(first),
            Ok(answer("SUMMARY-short", 10)),
            Ok(answer("NEVER-SENT", 10)),
        ],
    );
    let outcome = outcome.expect("a stop is an outcome, not an error");

    let LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
        window,
        target,
        occupied,
    }) = outcome.stop
    else {
        panic!("the run did not stop by name: {:?}", outcome.stop);
    };
    assert_eq!((window, target), (WINDOW_FIGURE, WINDOW_FIGURE / 2));
    assert!(
        occupied * 100 >= WINDOW_FIGURE * 80,
        "the stop names what was left, at or above the trigger: {occupied}"
    );
    assert_eq!(
        sent.len(),
        2,
        "turn 1 and the summary request, and nothing after"
    );
    assert!(is_summary(&sent[1]));
    assert_eq!(
        outcome
            .items
            .iter()
            .filter(|item| **item == reasoning)
            .count(),
        1,
        "the reasoning item is carried verbatim"
    );
    assert_eq!(
        warnings(&sink, "context-above-trigger"),
        1,
        "{:?}",
        all_warnings(&sink)
    );
}

// --- the run ------------------------------------------------------------------------------------

/// Runs one loop on `window` tokens over the scripted `replies`. Returns the outcome, the events
/// and every request sent.
fn run_windowed(
    window: u64,
    replies: Vec<Result<TurnOutcome, WireError>>,
) -> (
    Result<LoopOutcome, LoopError>,
    VecLoopSink,
    Vec<TurnRequest>,
) {
    let mut model = Scripted::new(replies);
    let mut tools = Tools::new();
    let mut approvals = ApproveAll;
    let mut sink = VecLoopSink::new();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config(window))
        .run("TASK-do-the-thing", &mut sink);
    (outcome, sink, model.requests)
}

fn config(window: u64) -> LoopConfig {
    LoopConfig::new(MODEL, INSTRUCTIONS)
        .with_retry_backoff(Duration::from_millis(1))
        .with_context_window(Some(window))
}

/// 12 kB of assistant text, weight that only a fold can reach.
fn long_text() -> String {
    format!(
        "PLAN-long: {}",
        "read the change, run the tests. ".repeat(380)
    )
}

fn says_and_asks(text: &str, input: u64) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::assistant(text), Item::ToolCall(call("call-1"))],
        usage: Some(usage(input)),
    }
}

fn asks(input: u64) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(call("call-1"))],
        usage: Some(usage(input)),
    }
}

fn answer(text: &str, input: u64) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant(text)],
        usage: Some(usage(input)),
    }
}

fn call(id: &str) -> ToolCall {
    ToolCall {
        call_id: CallId::new(id).expect("a call id"),
        name: ToolName::new("a").expect("a tool name"),
        arguments: json!({}),
    }
}

fn usage(input: u64) -> Usage {
    Usage {
        model: MODEL.to_owned(),
        input_tokens: input,
        output_tokens: 5,
        cached_input_tokens: 0,
        cache_creation_input_tokens: None,
    }
}

fn wire() -> WireId {
    WireId::new(responses::WIRE).expect("a wire")
}

// --- what the next request carries -------------------------------------------------------------

fn encode(item: &Item) -> String {
    serde_json::to_string(item).expect("encodes")
}

/// The request's conversation in the loop's own measure, the bytes of each item as JSON.
fn bytes(request: &TurnRequest) -> u64 {
    request
        .items
        .iter()
        .map(|item| u64::try_from(encode(item).len()).expect("small"))
        .sum()
}

fn assert_at_or_below_target(request: &TurnRequest, window: u64) {
    let tokens = bytes(request) / BYTES_PER_TOKEN;
    assert!(
        tokens * 100 <= window * 50,
        "the request after the compaction carries {tokens} tokens of a {window}-token window, \
         above its target: {:?}",
        request.items
    );
}

/// One item begins with the elision marker, and none with the summary marker.
fn assert_elision_note(request: &TurnRequest) {
    let notes = request
        .items
        .iter()
        .filter(|item| matches!(item, Item::UserText { text } if text.starts_with(ELISION_MARKER)))
        .count();
    assert_eq!(notes, 1, "{:?}", request.items);
    assert!(
        request.items.iter().all(
            |item| !matches!(item, Item::UserText { text } if text.starts_with(SUMMARY_MARKER))
        ),
        "{:?}",
        request.items
    );
    assert_eq!(
        request.items[0],
        Item::user("TASK-do-the-thing"),
        "the task is never folded"
    );
}

/// A summary request is the one request not carrying the run's standing instruction.
fn is_summary(request: &TurnRequest) -> bool {
    request.instructions != INSTRUCTIONS
}

fn warnings(sink: &VecLoopSink, code: &str) -> usize {
    sink.warnings().filter(|(seen, _)| *seen == code).count()
}

fn all_warnings(sink: &VecLoopSink) -> Vec<(String, String)> {
    sink.warnings()
        .map(|(code, message)| (code.to_owned(), message.to_owned()))
        .collect()
}

/// Each compaction event: items folded into a summary, and whether a summary turn was spent.
fn compacted(sink: &VecLoopSink) -> Vec<(usize, bool)> {
    sink.events()
        .iter()
        .filter_map(|event| match event {
            LoopEvent::Compacted {
                summarised_items,
                summary_turn,
                ..
            } => Some((*summarised_items, *summary_turn)),
            _ => None,
        })
        .collect()
}

// --- ports --------------------------------------------------------------------------------------

/// Answers its scripted replies in order and keeps every request; past the script it fails.
struct Scripted {
    wire: WireId,
    replies: VecDeque<Result<TurnOutcome, WireError>>,
    requests: Vec<TurnRequest>,
}

impl Scripted {
    fn new(replies: Vec<Result<TurnOutcome, WireError>>) -> Self {
        Self {
            wire: wire(),
            replies: replies.into(),
            requests: Vec::new(),
        }
    }
}

impl ModelPort for Scripted {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests.push(request.clone());
        self.replies
            .pop_front()
            .unwrap_or_else(|| Err(WireError::protocol("the script has no further turn")))
    }
}

/// One tool, `a`, answering a short result.
struct Tools {
    specs: Vec<ToolSpec>,
}

impl Tools {
    fn new() -> Self {
        Self {
            specs: vec![ToolSpec {
                name: ToolName::new("a").expect("a tool name"),
                description: "the a tool".to_owned(),
                envelope: Envelope::default(),
                input_schema: json!({"type": "object"}),
                approval: Approval::NotRequired,
            }],
        }
    }
}

impl ToolPort for Tools {
    fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    fn call(&mut self, _call: &ToolCall) -> ToolOutcome {
        ToolOutcome::ok(json!({"ok": true}))
    }
}

// --- the session --------------------------------------------------------------------------------

fn open(id: &SessionId, workspace: &Path) -> SessionFile {
    SessionFile::open(
        &SessionData {
            session_id: id.clone(),
            commission_run: CommissionRunId(Uuid(RUN.to_owned())),
            wire: responses::WIRE.to_owned(),
            boundary_refusals: 0,
        },
        MODEL,
        "http://127.0.0.1:9/v1",
        workspace,
    )
    .expect("a session opens")
}

fn layout(root: &Path) -> (PathBuf, PathBuf) {
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create");
    (workspace, root.join("state").join("sessions"))
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("compaction-target-{name}-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear scratch");
    }
    root
}
