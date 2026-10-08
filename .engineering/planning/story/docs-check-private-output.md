---
format: aep.planning-md/3
id: story:docs-check-private-output
kind: story
status: implemented
title: The docs check writes ESS output to a directory it owns alone
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-docs/src/main.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T07:33:43Z", actor: "human:timo", revision: 3, executor: "agent:loom", correlation: "wave/2026-10-07-w3"}
- {from: "proposed", to: "active", at: "2026-10-07T07:33:43Z", actor: "human:timo", revision: 4, executor: "agent:loom", correlation: "wave/2026-10-07-w3"}
- {from: "active", to: "implemented", at: "2026-10-08T00:33:49Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"verification":1}}}
---
## Outcome

`task docs-check` does not fail because another process holds a lock on the system temporary
directory.

`loom-docs` runs `ess generate --kind docs --out <temp dir>/loom-docs-<pid>-ess`, with the
directory taken from `TMPDIR`. ESS takes an output-ownership lock on the parent of the output
directory, so when `TMPDIR` is shared, an `ess` run elsewhere makes the check fail. Measured in the
gate of wave 2026-10-07-w2 on f0cb704: `ess generate --kind docs ... failed: error: output ownership
busy at <TMPDIR>: Resource temporarily unavailable (os error 11)`, exit 1; the same step with a
private `TMPDIR` exited 0. CI is not affected (a fresh runner); a workstation running several
sessions with one `TMPDIR` is.

## Acceptance

`loom-docs` writes the ESS output under a directory it owns alone (for example inside the
workspace's own `target/`), and a test holds that the output directory's parent is not the system
temporary directory.

## ESS first

No specification change.

## Source

The wave 2026-10-07-w2 gate log (pre-existing; the docs generator predates the wave).

## Scope confirmed

Read from `git diff --stat b8af5c2 f544d9f` at the close of wave 2026-10-07-w3: the inferred `crates/loom-docs/src/` is one file, `crates/loom-docs/src/main.rs` (78 insertions, 10 deletions).
