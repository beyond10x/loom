---
format: aep.planning-md/3
id: story:loom-cli
kind: story
status: implemented
title: b10x-loom run replaces b10x-intake run with the same flags and exit codes
relations:
- decomposes: epic:runtime-consolidation
- depends_on: story:runtime-merge
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/intake-cli
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T01:26:17Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T01:26:17Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T01:37:13Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"verification":1}}}
---
## Outcome

`b10x-loom run --workspace <dir> "<intent>"` replaces `b10x-intake run`, with the same flags,
output and exit codes, built with clap derive.

## Acceptance

- `cargo install --path crates/<cli>` installs `b10x-loom`; `b10x-loom run --help` lists
  `--workspace`, `--test-cmd`, `--max-steps`.
- Exit codes: 0 ApprovalRequired, 3 other stops, 1 errors, 2 usage, each by a named test.
- The offline slice test runs through `b10x-loom` and ends `stopped: ApprovalRequired
  (repository.merge)`.
- One live run against the Codex backend on a scratch workspace, transcript recorded under
  `docs/qualification/` (operator's hands).

## Depends on

`story:runtime-merge`.

## Scope (inferred)

loom: `crates/intake-cli` → the CLI crate.
