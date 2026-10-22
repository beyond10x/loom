---
format: aep.planning-md/3
id: story:session-executor-from-spec
kind: story
status: draft
title: SessionExecutor is built from an AgentSpec, its tools from the spec's ToolPolicy
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-domain
revision: 1
---
## Outcome

`SessionExecutor::from_spec(&AgentSpec, workspace, files)` builds the session executor from an
agent's data: its brief, session kind, model route and `ToolPolicy`. The constants `TOOLS` and
`CODEX_TOOLS` (`crates/loom-session-executor/src/lib.rs:90`, `:94`) become
`ToolPolicy::claude_default()` and `ToolPolicy::codex_default()`.

## Why

The tool list is fixed in code today; ADR 0104 puts tool policy with the product (L3) and the
per-call decision with metaharness (L2).

## Acceptance

- `crates/loom-session-executor/tests/from_spec.rs`
  (`a_spec_with_the_default_policy_runs_as_new_does`): on a scripted session, `from_spec` with the
  default policy and `SessionExecutor::new` give the same session request and the same proposal.
- (`a_denied_tool_is_never_offered`): a tool in `deny` is absent from the session request.
- The policy's decision mode is handed to metaharness's session request; Loom answers no call.
- `TOOLS` and `CODEX_TOOLS` stay, deprecated, for one release (`story:stability-version-rule`).

## ESS first

`loom.session` relates `AgentSession` to the `AgentSpec` it ran under; `task drift` is red first.

## Not in scope

Reading decisions back (`story:tool-decisions-read-back`).

## Scope

Crates: `loom-session-executor`, `ess/domains/session.yaml`. `crates/loom-session-executor/src/lib.rs`,
the new test, CHANGELOG.md.

## Ready

Waits on `story:agent-domain`.
