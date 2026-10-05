---
format: aep.planning-md/3
id: story:host-git-hardening
kind: story
status: implemented
title: Loom's host-side git runs no hooks and no fsmonitor
relations:
- decomposes: epic:effect-bindings
- informed_by: architecture-design:effect-isolation
- serves: vision:O1
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/loom-cli
- confidence: inferred
  path: crates/loom-intake-slice
- confidence: inferred
  path: website
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T13:03:32Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T13:03:32Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T13:50:51Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

Every git command Loom runs on the host for a run (inspect, edit and commit) runs with
`core.hooksPath` pointed at an empty directory and `core.fsmonitor=false`, so code a model edited,
or a test wrote into `.git/`, is never executed by host-side git.

## Why

architecture-design:effect-isolation, decision 3 (operator, 2026-10-05), and threat T4 there: a
confined command could write `.git/hooks` and `.git/config` (observed 2026-10-05).

## Acceptance

A run whose workspace carries a `pre-commit` hook and a `core.fsmonitor` command in `.git/config`
commits without executing either; the test asserts both left no trace.
