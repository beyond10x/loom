---
format: aep.planning-md/3
id: story:tool-decisions-on-step
kind: story
status: draft
title: A session's tool decisions reach its step's observation
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 2
---
## Outcome

A step a session executor ran carries the session's tool decisions on its `ExecutorStep` and in the
step's observation, beside `transcript_digest`, so a host reading a run's observations sees which
tool calls the harness allowed or denied without opening the `SessionReport`.

## Why

Decision D5 of wave 2026-10-22-w3 (approval-record:wave-proposal-2026-10-22-w3):
`story:tool-decisions-read-back` put the decisions on `SessionReport` only. Adding them to
`ExecutorStep` changes `ess/commission/` and about 45 construction sites in 22 files, outside that
story's scope.

## Acceptance

- `ess/commission/` declares an optional list of tool decisions on `ExecutorStep` and on the step's
  observation, absent (unknown) where the executor reports none, never an empty list for unknown.
- `b10x-loom-session-executor` fills it from `SessionReport::tool_decisions`; every other executor
  leaves it absent.
- A tool decision on a step is never evidence and never authority (AGENTS.md § Rules).
- A named test holds that a session's decisions reach the step's observation unchanged, and that an
  unknown census stays absent.

## ESS first

The first commit adds the field in `ess/commission/`; `task commission:drift` fails until the
generated types and the construction sites follow.

## Not in scope

Changing how decisions are read from trace-ir/1; runtime events (`story:runtime-events`).

## Scope

`ess/commission/`, `generated/rust/commission/`, `crates/loom-commission/`,
`crates/loom-session-executor/`, the construction sites of `ExecutorStep` across the workspace,
CHANGELOG.md.
