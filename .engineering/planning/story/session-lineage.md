---
format: aep.planning-md/3
id: story:session-lineage
kind: story
status: draft
title: A session records the earlier session it continues
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-domain
revision: 1
---
## Outcome

`SessionExecutor::continuing(previous: &AgentSession)` records that a session continues an earlier
one; `loom.session` keeps the lineage. Resuming the harness session itself waits on metaharness.

## Why

A long-lived agent's sessions are unrelated records today; `with_session_ids`
(`crates/loom-session-executor/src/lib.rs:331`) only mints ids. ADR 0098 lists continuity across
executor calls as a metaharness gap.

## Acceptance

- `crates/loom-session-executor/tests/lineage.rs`
  (`a_continuing_session_names_its_predecessor`): the second session's record names the first.
- A predecessor of another commission is refused, naming it.
- The session request to metaharness is unchanged (no resume flag) until metaharness offers one; I
  don't know whether metaharness 0.13.0 does.

## ESS first

`loom.session` declares the predecessor relation; `task drift` is red first.

## Not in scope

Harness session resume.

## Scope

Crates: `loom-session-executor`, `ess/domains/session.yaml` (shared with
`story:session-executor-from-spec` and `story:tool-decisions-read-back`: one at a time).

## Ready

Waits on `story:agent-domain`.
