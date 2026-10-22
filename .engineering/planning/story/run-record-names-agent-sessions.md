---
format: aep.planning-md/3
id: story:run-record-names-agent-sessions
kind: story
status: draft
title: The run record names each agent session by its trace-ir/1 digest and summary
summary: One jsonl event per metaharness session, holding its trace-ir-summary/1 (ADR 0096)
relations:
- serves: vision:governed-autonomy
revision: 2
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
- The `metaharness` binary is pinned at 0.13.0; no metaharness crate is added for this (metaharness
  `AGENTS.md`: binary first).
- A new record kind breaks `docs/contracts/run-events-v1.json` (`a_new_record_kind_is_a_break`):
  the story moves the committed contract and carries a `**Breaking:**` CHANGELOG bullet, or adds
  the event within `schema_version` 1 only if the contract allows it.

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
- `crates/loom-session-executor`
- `ess/intake/domains/events.yaml`, `generated/rust/intake/`, `docs/contracts/run-events-v1.json`,
  `website/docs/reference/run-events.md`
- their tests, `CHANGELOG.md`
