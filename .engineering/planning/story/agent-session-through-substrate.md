---
format: aep.planning-md/3
id: story:agent-session-through-substrate
kind: story
status: draft
title: An agent node's harness process starts and stops through Substrate under the action's limits
summary: The session executor's process runner is served by the engine's runner; no process of a run starts outside Substrate
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:bound-agent-node
- depends_on: story:action-limits-enforced
- depends_on: story:named-egress-per-action
revision: 1
---
## Outcome

An agent node's session starts and stops through Substrate like every other process of a run.
The `claude-code` adapter's harness process (metaharness and the Claude Code it starts) is
launched by the engine's runner, in the staging directory that holds the action's inputs, under
the action's limits and executable policy, with egress only to the aperture of the configured
model endpoint and the credential delivered by Substrate, never in the environment. A run then
starts no process outside Substrate, and stopping a tree stops its agent sessions with their
whole process trees.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "every process the runtime starts
goes through Substrate". `epic:protocol-tree-on-bindings` leaves the agent session outside it:
`story:bound-agent-node` starts Claude Code through the session executor's `SpawnRunner`
(`crates/loom-session-executor/src/lib.rs:1398-1414`), limited by tool policy only
(`~/beyond10x/sdc/docs/layers.md` § 4, violation 5). The session executor already takes a
caller's process runner (`with_runner(Box<dyn ProcessRunner>)`, `lib.rs:1123`), which is the
seam. Open, read 2026-11-21: whether metaharness's `ProcessRunner` can be served by a Substrate
child with live standard input and output, and how the operator's Claude login reaches a
confined process (`~/beyond10x/sdc/docs/layers.md`, "Not verified"); Substrate delivers secrets
as sealed slots (Substrate ADR 0012).

## ESS first

`ess/domains/bindings.yaml`: `AgentRun` gains the applied confinement record of its session.
`ess/domains/tree.yaml`: the engine configuration gains the model endpoint's aperture name and
the credential slot. On that commit `task drift` fails and
`crates/loom-bindings/tests/agent_confined.rs` does not compile.

## Acceptance

With a recorded harness program in place of metaharness, no model call:

- The harness is started by the engine's runner: its applied record names the adapter, and
  `crates/loom-bindings` and `crates/loom-session-executor` reach no `Command::new` on this path
  (a test greps the adapter's module and asserts the runner was called).
- The harness sees the staging directory as its workspace and cannot read a file of the real
  workspace that is not among the action's inputs.
- A harness that runs past the action's wall limit is stopped with its whole process tree and
  the step is refused naming the limit.
- A harness that connects anywhere but the model endpoint's aperture fails to connect.
- The credential is not in the harness's environment or argv; it is readable at the slot path.
- Stopping `tree run` with SIGTERM while a session runs leaves no process of that session
  (checked by the session's cgroup or process group being empty).
- `task check` exits 0.

## Out of scope

Changes in metaharness. Codex and b10x-harness sessions. The Connectors command-line invoker.
