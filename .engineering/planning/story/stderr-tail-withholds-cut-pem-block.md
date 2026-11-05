---
format: aep.planning-md/3
id: story:stderr-tail-withholds-cut-pem-block
kind: story
status: implemented
title: The stderr tail withholds a PEM block whose opening the window cut
tags:
- from-finding
- security
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-session-executor/tests/security_w20261105w2_u4.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T09:33:25Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
- {from: "proposed", to: "active", at: "2026-11-05T09:33:25Z", actor: "human:timo", revision: 4, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
- {from: "active", to: "implemented", at: "2026-11-05T12:07:42Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
---
## Problem

When a `-----BEGIN` line falls before the 4096-byte stderr tail window and the block has no `-----END`,
the window holds only body lines, so the session error and the failure record keep them. The
transcript is right here, because its walk withholds from the opening on.

## Acceptance

- The case `security_a_pem_block_the_tail_window_cuts_from_its_opening_is_withheld_from_the_record`
  in `crates/loom-session-executor/tests/security_w20261105w2_u4.rs` is green: the error and the failure
  record hold no line of a PEM block whose opening the tail window cut.

## Origin

Pre-existing finding of the wave 2026-11-05-w2 U4 security review (`stderr_tail`, unchanged since 0.57.0).
