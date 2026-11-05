---
format: aep.planning-md/3
id: story:a2a-stderr-line-escapes-error-text
kind: story
status: active
title: A b10x-loom A2A line on standard error stays one line whatever the error text holds
tags:
- from-finding
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-agent/src/serve/a2a.rs
- confidence: cited
  path: crates/loom-agent/tests/a2a.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T06:22:00Z", actor: "human:timo", revision: 2, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-w2-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T06:22:00Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-w2-loom"}
---
## Outcome

A line `b10x-loom` writes to standard error for an A2A failure is one line whatever the error text
holds: a newline or another control character in that text is escaped, so text a model produced
cannot forge a further `b10x-loom: A2A:` line.

## Why

Filed from the security review of wave 2026-11-05-w1, U1 (`story:a2a-turn-error-names-no-host-path`):
`failed()` in `crates/loom-agent/src/serve/a2a.rs` prints the turn's error unescaped, and some of
that text is model output (an intent named at `crates/loom-plugin/src/classify.rs:133`). No model
was shown to emit a newline there; the reviewer judged it a note.

## Acceptance

- A turn whose error text holds a newline writes exactly one `b10x-loom: A2A:` line to standard
  error, with the newline escaped.
