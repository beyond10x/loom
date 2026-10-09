// SPDX-License-Identifier: Apache-2.0

//! Adversary pass 1 on `story:compaction-target-bound`.
//!
//! The story's session is what the provider counts: the conversation items **and** the
//! instruction and tool schemas it reports in its input (Outcome; `LoopStop::ContextAboveTrigger`
//! docs; CHANGELOG). These cases drive the loop with a provider that reports its input the way a
//! real one does — a fixed overhead for the instruction and tool schemas plus the conversation it
//! was handed — and with one that omits usage on a later turn, and check the two promises: the
//! session after compaction is at or below the target wherever eliding can reach it, and no
//! request goes out while the session is at or above the trigger.
//!
//! Every model here is a scripted [`ModelPort`] in this process: no socket, no network.

use std::collections::VecDeque;
use std::time::Duration;

use b10x_loom_executor::harness::responses;
use b10x_loom_executor::harness::turn_loop::{
    AgentLoop, ApproveAll, LoopConfig, LoopError, LoopOutcome, LoopStop,
    LoopStopContextAboveTrigger, VecLoopSink,
};
use b10x_loom_executor::harness::wire::{
    Approval, CallId, Envelope, Item, ModelPort, StopReason, StreamSink, ToolCall, ToolName,
    ToolOutcome, ToolPort, ToolSpec, TurnOutcome, TurnRequest, Usage, WireError, WireId,
};
use serde_json::json;

const MODEL: &str = "scripted-model";
const INSTRUCTIONS: &str = "INSTRUCTIONS-standing";
const BYTES_PER_TOKEN: u64 = 4;

// --- the provider counts the instruction and tool schemas ---------------------------------------

/// The instruction and tool schemas take 700 tokens of a 1 000-token window, which the provider
/// counts in every request. With the task, the newest turn group and the elision note on top, what
/// no compaction removes is at or above the 800-token trigger, so the run must stop by name before
/// the next request. The loop instead decides on its byte estimate of the items alone, which after
/// eliding is about 160 tokens, and sends the request: the provider counts it above the trigger.
#[test]
fn a_request_is_not_sent_when_the_counted_instructions_keep_the_session_above_its_trigger() {
    let window = 1_000;
    let (outcome, sent) = run_counted(
        window,
        700,
        vec![
            Reply::Conversation(says_and_asks(&long_text(), "call-1")),
            Reply::Summary(Err(WireError::protocol("the summary stream broke"))),
            Reply::Conversation(answer("DONE")),
        ],
    );
    let outcome = outcome.expect("a stop is an outcome, not an error");

    let trigger = window * 80 / 100;
    let after: Vec<u64> = sent
        .iter()
        .skip_while(|request| !is_summary(request))
        .filter(|request| !is_summary(request))
        .map(|request| counted(request, 700))
        .collect();
    assert!(
        after.iter().all(|count| *count < trigger),
        "after the compaction the loop sent conversation request(s) the provider counts at \
         {after:?} tokens of a {window}-token window, at or above its {trigger}-token trigger; \
         the run ended {:?}",
        outcome.stop
    );
}

/// The instruction and tool schemas take 300 tokens of a 1 000-token window. Eliding the fold
/// leaves the session, as the provider counts it, at about 450 tokens: under the 500-token target.
/// A summary of about 1 kB leaves it at about 610. The story keeps a summary only when the session
/// it leaves is at or below the target (or when it is smaller than the note), so this one must be
/// elided; the loop measures the target in item bytes only, keeps it, and the next request goes out
/// above the target that eliding would have reached.
#[test]
fn a_summary_that_leaves_the_counted_session_above_its_target_is_not_kept() {
    let window = 1_000;
    let summary = "SUMMARY-fits: the plan was read and tested. ".repeat(22);
    let (outcome, sent) = run_counted(
        window,
        300,
        vec![
            Reply::Conversation(says_and_asks(&long_text(), "call-1")),
            Reply::Summary(Ok(answer_turn(&summary))),
            Reply::Conversation(answer("DONE")),
        ],
    );
    let outcome = outcome.expect("the run answers after compacting");
    assert_eq!(outcome.stop, LoopStop::Completed, "precondition");
    assert_eq!(sent.len(), 3, "turn 1, the summary request, turn 2");
    assert!(
        is_summary(&sent[1]),
        "precondition: a summary turn was spent"
    );

    let target = window * 50 / 100;
    let next = counted(&sent[2], 300);
    assert!(
        next <= target,
        "the request after the compaction is {next} tokens as the provider counts it, above the \
         {target}-token target, though eliding the fold leaves about 450: {:?}",
        sent[2].items
    );
}

// --- a count that is no longer true -------------------------------------------------------------

/// A 4 000-token window: trigger 3 200, target 2 000. Turn 2 is sent at about 3 100 tokens of
/// conversation and the provider counts 3 500 (about 400 tokens of instruction and tool schemas).
/// The compaction before turn 3 elides the fold and brings the session to a few hundred tokens.
/// Turn 3's reply omits usage, which the loop already tolerates (`usage_unobservable`, no ceiling
/// declared). The compaction check before turn 4 then reads turn 2's 3 500 as the session's size —
/// a count of a
/// conversation that no longer exists — finds nothing left to remove and ends the run
/// `ContextAboveTrigger` with a session of a few hundred tokens. Before this unit that stale count
/// cost a no-op compaction; now it ends the run.
#[test]
fn a_count_from_before_the_compaction_does_not_stop_a_run_whose_session_is_small() {
    let window = 4_000;
    let mut model = Scripted::new(vec![
        Ok(with_usage(says_and_asks(&long_text(), "call-1"), Some(10))),
        Ok(with_usage(asks("call-2"), Some(3_500))),
        Err(WireError::protocol("the summary stream broke")),
        Ok(with_usage(asks("call-3"), None)),
        Ok(with_usage(answer("DONE"), Some(200))),
    ]);
    let mut tools = Tools::new();
    let mut approvals = ApproveAll;
    let mut sink = VecLoopSink::new();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config(window))
        .run("TASK-do-the-thing", &mut sink)
        .expect("a stop is an outcome, not an error");

    let sizes: Vec<u64> = model
        .requests
        .iter()
        .map(|request| bytes(request) / BYTES_PER_TOKEN)
        .collect();
    assert_eq!(
        outcome.stop,
        LoopStop::Completed,
        "the run was stopped with the conversation at {sizes:?} estimated tokens per request \
         (the last a few hundred of a {window}-token window); warnings: {:?}",
        sink.warnings().collect::<Vec<_>>()
    );
    assert_eq!(outcome.text, "DONE");
}

// --- boundaries of the trigger ------------------------------------------------------------------

/// Exactly at the trigger with nothing removable: the run stops by name, naming 800.
#[test]
fn a_session_exactly_at_its_trigger_with_nothing_to_remove_stops() {
    let mut model = Scripted::new(vec![
        Ok(with_usage(asks("call-1"), Some(800))),
        Ok(with_usage(answer("NEVER-SENT"), Some(10))),
    ]);
    let mut tools = Tools::new();
    let mut approvals = ApproveAll;
    let mut sink = VecLoopSink::new();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config(1_000))
        .run("TASK-do-the-thing", &mut sink)
        .expect("a stop is an outcome");
    assert_eq!(
        outcome.stop,
        LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
            window: 1_000,
            target: 500,
            occupied: 800
        })
    );
    assert_eq!(model.requests.len(), 1);
}

/// One token under the trigger: no compaction and no stop.
#[test]
fn a_session_one_token_under_its_trigger_is_not_stopped() {
    let mut model = Scripted::new(vec![
        Ok(with_usage(asks("call-1"), Some(799))),
        Ok(with_usage(answer("DONE"), Some(10))),
    ]);
    let mut tools = Tools::new();
    let mut approvals = ApproveAll;
    let mut sink = VecLoopSink::new();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config(1_000))
        .run("TASK-do-the-thing", &mut sink)
        .expect("answers");
    assert_eq!(outcome.stop, LoopStop::Completed);
    assert_eq!(model.requests.len(), 2);
}

// --- the run ------------------------------------------------------------------------------------

enum Reply {
    /// A conversation turn; its usage is filled in from what the provider counts.
    Conversation(TurnOutcome),
    /// A summary turn, answered as given.
    Summary(Result<TurnOutcome, WireError>),
}

/// Runs one loop whose provider counts `overhead` tokens of instruction and tool schemas plus the
/// conversation it is handed, at the loop's own four bytes a token.
fn run_counted(
    window: u64,
    overhead: u64,
    replies: Vec<Reply>,
) -> (Result<LoopOutcome, LoopError>, Vec<TurnRequest>) {
    let mut model = Counting {
        wire: wire(),
        overhead,
        replies: replies.into(),
        requests: Vec::new(),
    };
    let mut tools = Tools::new();
    let mut approvals = ApproveAll;
    let mut sink = VecLoopSink::new();
    let outcome = AgentLoop::new(&mut model, &mut tools, &mut approvals, config(window))
        .run("TASK-do-the-thing", &mut sink);
    (outcome, model.requests)
}

fn config(window: u64) -> LoopConfig {
    LoopConfig::new(MODEL, INSTRUCTIONS)
        .with_retry_backoff(Duration::from_millis(1))
        .with_context_window(Some(window))
}

fn long_text() -> String {
    format!(
        "PLAN-long: {}",
        "read the change, run the tests. ".repeat(380)
    )
}

fn says_and_asks(text: &str, id: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::assistant(text), Item::ToolCall(call(id))],
        usage: None,
    }
}

fn asks(id: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::ToolCalls,
        items: vec![Item::ToolCall(call(id))],
        usage: None,
    }
}

fn answer(text: &str) -> TurnOutcome {
    TurnOutcome {
        stop_reason: StopReason::EndTurn,
        items: vec![Item::assistant(text)],
        usage: None,
    }
}

fn answer_turn(text: &str) -> TurnOutcome {
    with_usage(answer(text), Some(10))
}

fn with_usage(mut outcome: TurnOutcome, input: Option<u64>) -> TurnOutcome {
    outcome.usage = input.map(usage);
    outcome
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

fn bytes(request: &TurnRequest) -> u64 {
    request
        .items
        .iter()
        .map(|item| {
            u64::try_from(serde_json::to_string(item).expect("encodes").len()).expect("small")
        })
        .sum()
}

/// What a provider with `overhead` tokens of instruction and tool schemas counts for `request`.
fn counted(request: &TurnRequest, overhead: u64) -> u64 {
    overhead + bytes(request) / BYTES_PER_TOKEN
}

fn is_summary(request: &TurnRequest) -> bool {
    request.instructions != INSTRUCTIONS
}

// --- ports --------------------------------------------------------------------------------------

/// Reports, for each conversation request, the overhead plus the conversation it was handed.
struct Counting {
    wire: WireId,
    overhead: u64,
    replies: VecDeque<Reply>,
    requests: Vec<TurnRequest>,
}

impl ModelPort for Counting {
    fn wire(&self) -> &WireId {
        &self.wire
    }

    fn turn(
        &mut self,
        request: &TurnRequest,
        _sink: &mut dyn StreamSink,
    ) -> Result<TurnOutcome, WireError> {
        self.requests.push(request.clone());
        match self.replies.pop_front() {
            Some(Reply::Conversation(mut outcome)) => {
                assert!(
                    !is_summary(request),
                    "script: a conversation reply met a summary request"
                );
                outcome.usage = Some(usage(counted(request, self.overhead)));
                Ok(outcome)
            }
            Some(Reply::Summary(reply)) => {
                assert!(
                    is_summary(request),
                    "script: a summary reply met a conversation request"
                );
                reply
            }
            None => Err(WireError::protocol("the script has no further turn")),
        }
    }
}

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
