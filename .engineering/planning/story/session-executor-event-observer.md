---
format: aep.planning-md/3
id: story:session-executor-event-observer
kind: story
status: draft
title: A caller of the session executor sees each metaharness event while the session runs
summary: SessionExecutor::with_observer hands each event and the session's copy to the caller, in transcript order; takt prints a harness step live
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:O3
revision: 2
---
## Outcome

A caller of the session executor (`b10x-loom-session-executor`, reached as `loom_sdk::session`)
sees every `metaharness.event/1` line of a session while the session runs, not only after the
call returns: `SessionExecutor::with_observer` takes a function that is handed each event the
executor receives, with the session's scratch copy, right after the executor appends it to the
report's transcript, in transcript order. A host such as takt can then show a harness step's tool
calls, files, tokens and thinking live.

The observer reads; it cannot change the transcript, the report, the proposal or the session's
decisions. With no observer, every call behaves as before, byte for byte. A panic in the observer
is not swallowed: the executor halts the session (metaharness kills the harness child) and the
panic continues out of the call with its own payload.

## Why

`SessionExecutor::session` (`crates/loom-session-executor/src/lib.rs`) polls `run.poll_event()`
and only appends each event to `SessionReport::transcript`; after the session it replaces that
transcript with the vendor record metaharness kept. A caller of `run_with_usage` gets nothing
until the step ends, so a long harness step is silent to the person watching it.

Consumer: takt, which prints a harness step's progress while it runs.

## ESS first

No `ess/` change. The observed noun is metaharness's `EventLine` (`metaharness.event/1`), not a
Loom noun, and the observer is a builder of the executor, not a command or a record of the
`loom.session` domain. The named test that fails before the change is the story's own
`crates/loom-session-executor/tests/event_observer.rs`.

## Acceptance

`crates/loom-session-executor/tests/event_observer.rs`, over a scripted Codex session
(metaharness's `ScriptedRunner`: no harness program, no model, no network):

1. `the_observer_sees_tool_calls_and_usage_while_the_session_runs`: the observer is handed the
   session's `ToolRequested` and `Usage` events before `run_with_usage` returns, while the
   session's copy still exists.
2. `the_observed_events_are_the_transcript_in_order`: with no vendor record kept, each observed
   event, serialized as the executor serializes it, is the report's transcript line at the same
   place, and there are as many of each; with a vendor record kept, the report's transcript is
   that record, as without an observer, and the observer saw the same events in the same order.
3. `the_observer_is_handed_the_sessions_copy`: every path handed to the observer is the report's
   `cwd`, the session's copy.
4. `a_panicking_observer_halts_the_session_and_propagates`: a panic in the observer leaves the
   call with the observer's payload, and the harness child is killed.
5. Every existing test of the crate passes unchanged (no observer: behaviour as before).

## Scope

- `crates/loom-session-executor/src/lib.rs` (`SessionExecutor`, `with_observer`, the poll loop of
  `session`) — cited
- `crates/loom-session-executor/tests/event_observer.rs` — new
- `AGENTS.md` § Session executor and § Checks that hold the rules, `CHANGELOG.md` (`### Added`),
  `website/docs/reference/sdk-stability.md` (the `loom_sdk::session::SessionExecutor` row) — cited
- Not changed: `ess/`, `crates/loom-sdk` (it re-exports the crate as `loom_sdk::session`, so the
  method is there), `crates/loom-sdk/tests/host_ports.rs`, `consumers/pinned/`, `docs/contracts/`

## Out of scope

A Loom-side rendering of the events; a `Send` observer (the executor is not `Send`); takt's own
change.
