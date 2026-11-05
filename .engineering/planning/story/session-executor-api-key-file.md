---
format: aep.planning-md/3
id: story:session-executor-api-key-file
kind: story
status: archived
title: A session executor reads its endpoint's API key from a file
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-session-executor/src/lib.rs
revision: 4
transitions:
- {from: "draft", to: "archived", at: "2026-11-05T06:22:05Z", actor: "human:timo", revision: 4, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-w2-loom"}
---
## Outcome

A Loom session executor reads its endpoint's API key from a file, beside the environment route it
has today, so a headless pod can hand the key to the session as a staged Secret file (mode 0600,
owned by the pod user) instead of an environment variable.

## Why

The orchestrator, 2026-10-26, from the takt 0.13.1 release: "the session executor has no route for
an API-key file (only env)". The kind trial staged its endpoint token as a file through
`spec.secretFiles` and had to unset the environment copy; `story:endpoint-api-key-secret`
(implemented) reads the key from the operator's secret store by name, which a pod without a
Secret Service does not have.

## Acceptance

- `SessionExecutor` takes an API-key file path; the child is launched with the key read from that
  file and never placed in Loom's argv or a log line.
- `an_api_key_file_reaches_the_session_without_entering_argv` (recorded fixture, no network).
- A file another user owns or that group or others may read is refused, as the gates' reader
  refuses it.

## Scope

Derived 2026-10-26 by the coordinator from the release note quoted above; not read in the tree.

- `crates/loom-session-executor/src/lib.rs` (`Endpoint`, `SessionExecutor::spec`) — inferred
- metaharness `RunSpec` (whether it carries a key file today) — inferred, may need a metaharness story
- README.md, AGENTS.md § Session executor, CHANGELOG.md — inferred

## Archived

Rejected on 2026-11-05 by the orchestrator, on the operator's decision of 2026-10-21 recorded when
the metaharness twin `story:b10x-api-key-from-file` was archived: "the key lives in the secret
store, never in a file". A session executor keeps reading its endpoint's key from the secret store
(`SessionExecutor::api_key_secret`).
