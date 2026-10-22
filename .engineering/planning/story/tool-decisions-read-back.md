---
format: aep.planning-md/3
id: story:tool-decisions-read-back
kind: story
status: implemented
title: A session's report lists each tool decision its trace-ir/1 record holds
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T08:58:30Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-22T08:58:30Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-22T10:13:07Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

After a session, its `SessionReport` lists each tool call's decision and who made it, read from the
session's `trace-ir/1` record. Loom makes none of these decisions.

## Why

ADR 0104 points 2-3: metaharness decides each call, and the product learns from the record after
the session. metaharness 0.13.0, pinned since wave 2026-10-22-w2, records the run's
`DecisionCensus` in `trace-ir/1` (`metaharness-protocol/src/trace_ir.rs:180`) and each call's
`DecidedBy` (`event.rs:121`: embedder, frame, deadline, adapter). Loom reads only the summary and
digest today (`SessionReport::trace`).

## Acceptance

- `crates/loom-session-executor/tests/tool_decisions.rs`
  (`each_tool_decision_of_the_record_is_in_the_report`): a scripted session whose record holds an
  allowed and a denied call reports both, with `DecidedBy`, in record order.
- (`a_record_without_decisions_says_so`): a record with no census reports the decisions as
  unknown, never as none.
- Decisions travel on the report and the step's observation, never as evidence.
- `SessionReport` gains a field: a `**Breaking:**` CHANGELOG bullet.

## ESS first

`loom.session` declares the tool decision record; `task drift` is red first.

## Not in scope

Deciding or seeing a call while it happens (ADR 0104 point 1); emitting decisions as events
(`story:runtime-events` references the session only).

## Scope

Crates: `loom-session-executor`, `ess/domains/session.yaml`. `crates/loom-session-executor/src/lib.rs`
(`SessionReport`), the new test, `generated/rust/loom/`, CHANGELOG.md.

## Ready

Ready now.
