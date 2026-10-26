---
format: aep.planning-md/3
id: story:a2a-record-and-streaming
kind: story
status: proposed
title: An A2A turn is recorded in record.jsonl, and SendStreamingMessage streams the task's status
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-a2a
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:20:19Z", actor: "human:timo", revision: 2}
---
## Outcome

An A2A turn of a served agent is recorded in the plugin's `record.jsonl` as a polled item's turn
is, and the listener answers `SendStreamingMessage` (alias `message/stream`) with a server-sent event
stream of the task's status updates, ending with the record, under the same bearer and connection
bounds as `SendMessage`.

## Why

AGENTS.md § Agent spec, at `b6e35835`: "Not done: A2A turns in `record.jsonl`, streaming, push and
multi-turn tasks." Filed from the agents wave by the orchestrator (dispatch-queue #13, 2026-10-26):
"A2A `record.jsonl` + streaming ... leave A2A streaming as accepted-not-started unless small". It is
not small: it changes the listener's connection model (one long response per admitted slot), the
task store and the record format's reader.

## Acceptance

- `an_a2a_turn_is_recorded_once`: a `SendMessage` turn appends one line to `record.jsonl`, keyed by
  the item revision the `messageId` gives, and a repeated `messageId` does not run a second turn.
- `a_streamed_message_sends_status_updates_then_the_record`: `SendStreamingMessage` answers
  `text/event-stream`, one event per `RuntimeEvent` status change, the last carrying the task's end.
- `a_stream_without_the_bearer_is_refused` before any event; an idle stream is cut at the answer
  bound.
- The agent card advertises `capabilities.streaming: true` only when the listener streams.
- ESS first: the decision the listener takes per request (`AdmitA2aRequest`) gains the streaming
  method, and the record's line for an A2A turn is declared before the code; scenarios red first.

## Not in scope

Push notifications; multi-turn tasks (`input-required`); resubscription to a running task.

## Scope

Derived 2026-10-26 by the coordinator. **cited** = read in the tree, **inferred** = could be wrong.

- **Primary surface:** `crates/loom-a2a` (listener, slots, task store) — cited
- **Files:** `crates/loom-agent/src/serve/a2a.rs` (`Answering`) — cited
- **Files:** `crates/loom-plugin` (the record's writer and reader, `record.jsonl`) — inferred
- **Files:** `ess/domains/agent.yaml` (`AdmitA2aRequest` :749) — cited
- **Files:** `crates/loom-cli/src/a2a.rs` — inferred
- **Documents:** AGENTS.md § Agent spec (A2A paragraph), README.md, `docs/qualification/2026-10-25-agent-a2a.md`, CHANGELOG.md — cited
- **Confidence:** medium
