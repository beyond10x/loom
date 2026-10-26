---
format: aep.planning-md/3
id: story:session-executor-b10x-ask
kind: story
status: draft
title: A hand-built b10x session under ask is refused or decided by b10x rules, never by Claude file-tool rules
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-session-executor/tests
revision: 3
---
## Outcome

A `SessionExecutor` built by hand (its public `tools` field) for a b10x-harness session under
`ask` either decides each call by rules made for b10x's operations, or is refused before the spawn
as `from_spec` refuses such a spec. It never answers a b10x call with Claude Code's file-tool rules.

## Why

The wave 2026-10-26-q13 adversary over `story:metaharness-0-19-pin`: metaharness 0.19.0 runs
`b10x --decisions frame|ask` (Atlas ADR 0104 point 3), so the public `tools` field with `Ask` on
`Kind::B10x` now starts a session, and `decide_among` answers it with Claude file-tool rules. No
in-repository caller does this, and nothing tests it.

## Acceptance

- `a_hand_built_b10x_executor_under_ask_is_refused_or_decided_by_b10x_rules`: a red test first,
  then either a refusal before the spawn or b10x rules (the story decides which, recorded).
- `from_spec` keeps refusing b10x under `frame` and `ask` until the rules exist.

## Scope

Derived 2026-10-26 by the coordinator from `93ee8137`.

- `crates/loom-session-executor/src/lib.rs` (`decide_among`, `SpecError::DecisionModeUnenforced`) — cited
- `crates/loom-session-executor/tests/` — cited
- CHANGELOG.md — inferred
