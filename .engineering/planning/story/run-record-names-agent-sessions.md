---
format: aep.planning-md/3
id: story:run-record-names-agent-sessions
kind: story
status: implemented
title: The run record names each agent session by its trace-ir/1 digest and summary
summary: One jsonl event per metaharness session, holding its trace-ir-summary/1 (ADR 0096)
relations:
- serves: vision:governed-autonomy
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T04:27:48Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-22T04:27:48Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-22T06:44:23Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

`b10x-loom run --output jsonl` stays Loom's run record: the route, the approval, the terminal
record and the turns of Loom's own model calls. For agent work run through metaharness, the record
carries one event per agent session that names the session's `trace-ir/1` document by digest and
holds its `trace-ir-summary/1`. It does not re-encode that session's turns, tool calls or usage.

## Why

- Atlas ADR 0096, decision 7: metaharness owns the session format; other trace formats become
  projections of `trace-ir/1` or are retired. metaharness `story:retire-trace-formats-beside-trace-ir`
  keeps Loom's jsonl: agent work carries the digest and the projection.
- On `9e63c11b`, `crates/loom-cli/src/main.rs:476-484` writes the jsonl stream and
  `crates/loom-cli/src/events.rs` declares the route, turn, tool-call, usage, approval and terminal
  events (ESS `ess/intake/domains/events.yaml`). Agent work reaches metaharness through
  `crates/loom-session-executor`, and no event names the session's `trace-ir/1` document.
- metaharness 0.13.0 projects a session record: `metaharness project [--from <source>] --to
  trace-ir-summary <record>`. Its fields are those of amendment a42 of metaharness
  `docs/design/metaharness-protocol-v0.1.md`, each `null` when the session's record does not state
  it: `transcript_digest`, `source`, `tokens_basis` (the session's own token counts or a sum of its
  requests) and `models` (one entry per model, each with its own tokens and cost).

## Design

- ESS first: `ess/intake/domains/events.yaml` gains a session event holding the `trace-ir/1`
  document's `transcript_digest`, its `source` and the `trace-ir-summary/1` fields.
- The summary is computed by `metaharness::protocol::summarize` (metaharness 0.13.0, the function
  `metaharness project --to trace-ir-summary` calls, `crates/metaharness-cli/src/lib.rs:325` at
  tag `0.13.0`). `crates/loom-session-executor` moves its `metaharness` and `metaharness-b10x`
  pins from `0.10.1` to `0.13.0` together; no new metaharness crate and no `metaharness` binary
  is run (tests have no such program, and Loom's tests start no harness program). Decision of
  wave 2026-10-22-w2, replacing the earlier binary design.
- The session executor projects each session's record to `trace-ir/1`, summarizes it and returns
  the summary on its step, as it returns the transcript digest today; the command line writes the
  event from it. A session whose record cannot be projected writes the event with the summary
  fields `null` and says why, and never fails the run.
- A new record kind breaks `docs/contracts/run-events-v1.json` (`a_new_record_kind_is_a_break`):
  the story moves the committed contract and carries a `**Breaking:**` CHANGELOG bullet.

## ESS first

The session event in `ess/intake/domains/events.yaml`; `task intake-generate` and
`task docs-generate` regenerate. Red: the conformance scenario below and `task docs-check`.

## Acceptance

- Before: a run whose agent work goes through `loom-session-executor` writes no event naming the
  session's `trace-ir/1` document. After: it writes one session event per agent session with the
  document's `transcript_digest`, `source` and the `trace-ir-summary/1` fields, and no turn,
  tool-call or usage event for that session's own turns.
- Loom's own model-call turns, route, approval and terminal events are unchanged.
- A conformance scenario over a committed recorded session shows the event's fields equal the
  summary's.

## Not in scope

- Changing the `trace-ir/1` format or the summary's fields (metaharness owns them).

## Scope

- `crates/loom-cli/src/events.rs`, `crates/loom-cli/src/main.rs`
- `crates/loom-session-executor` (its `Cargo.toml` pins), `Cargo.lock`
- `ess/intake/domains/events.yaml`, `generated/rust/intake/`, `docs/contracts/run-events-v1.json`,
  `website/docs/reference/run-events.md`, `website/docs/reference/ess/`, `website/data/ess/`
- their tests, `CHANGELOG.md`, `AGENTS.md`
- Shares `CHANGELOG.md` and `AGENTS.md` with `story:checked-timeout-deadlines`; merged one at a
  time.
