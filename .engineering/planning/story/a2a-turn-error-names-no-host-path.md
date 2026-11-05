---
format: aep.planning-md/3
id: story:a2a-turn-error-names-no-host-path
kind: story
status: implemented
title: An A2A turn's own error reaches the caller without naming a host path
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
  path: crates/loom-a2a/src/server.rs
- confidence: cited
  path: crates/loom-agent/src/serve/a2a.rs
- confidence: cited
  path: crates/loom-agent/tests/a2a.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:40:09Z", actor: "human:timo", revision: 2, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T02:40:09Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "active", to: "implemented", at: "2026-11-05T04:37:32Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1},"asserted":{"test_result":1}}, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
---
## Outcome

An error from an A2A turn itself reaches the caller (the stream's last `statusUpdate`, `GetTask`,
`SendMessage`) as a fixed reason that names no directory of the machine; the full error goes to
standard error only, as the record read and append errors already do since wave 2026-11-04-w1.

## Why

Filed from the U1 fix round of wave 2026-11-04-w1 (`story:a2a-record-and-streaming`): the
implementor noted that an error from the turn itself still passes through as
`the turn could not run: {why}`, and `{why}` may name a host path (for example the plugin state
directory). That pipe existed before the wave; the fix round made only the record errors fixed.

## Acceptance

- A turn whose plugin or executor fails with an error naming an absolute path answers the A2A caller
  with a reason that names no path, on `SendMessage`, `GetTask` and a stream's last `statusUpdate`.
- The full error is written to standard error.
