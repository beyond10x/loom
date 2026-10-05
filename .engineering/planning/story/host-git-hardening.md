---
format: aep.planning-md/3
id: story:host-git-hardening
kind: story
status: draft
title: Loom's host-side git runs no hooks and no fsmonitor
relations:
- decomposes: epic:effect-bindings
- informed_by: architecture-design:effect-isolation
- serves: vision:O1
revision: 1
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
