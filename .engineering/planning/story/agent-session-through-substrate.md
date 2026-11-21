---
format: aep.planning-md/3
id: story:agent-session-through-substrate
kind: story
status: draft
title: An agent node's Claude Code process starts and stops through Substrate under the action's limits
summary: The session's one child process through the engine's runner; held by decision-blocker:agent-harness-confinement-design
tags:
- sdc
relations:
- decomposes: epic:tree-daemon-under-enforced-limits
- serves: vision:O1
- depends_on: story:bound-agent-node
- depends_on: story:action-limits-enforced
- depends_on: story:named-egress-per-action
revision: 6
---
## Outcome

An agent node's session starts and stops through Substrate like every other process of a run: the
`claude-code` vendor program, the one child process of a session, is launched by the engine's
runner under the action's limits, after the runner's executable check admitted it, with egress only
to the model endpoint and with its credential delivered by Substrate, never in the environment. A
run then starts no process outside Substrate. How the session gets its files, its credential and its
model endpoint inside a confined view is not decided: `decision-blocker:agent-harness-confinement-design`
holds this story, and its acceptance is written once the decision is recorded.

## Why

Requirement from Timo, relayed by session `sdc` on 2026-11-21: "every process the runtime starts
goes through Substrate". `epic:protocol-tree-on-bindings` leaves the agent session outside it.
Verified on 2026-11-21 (Loom at the tree of this plan, metaharness at tag `0.23.0`, Substrate at
tag `0.21.0`):

- Metaharness is linked into Loom's process; only the vendor program is a child.
  `story:bound-agent-node` starts it through metaharness's `SpawnRunner`
  (`crates/loom-session-executor/src/lib.rs:1398-1414`), and the session executor takes a caller's
  runner (`with_runner(Box<dyn ProcessRunner>)`, `lib.rs:1121-1125`), which is the seam.
- Metaharness starts processes of its own, outside any injected runner: a `--version` probe under
  strict version checks (`crates/metaharness/src/builder.rs:2565-2566`) and `git` for declared
  context files (`crates/metaharness/src/context.rs:245`).
- The session's working directory is the executor's scratch copy of the staging directory
  (`crates/loom-session-executor/src/lib.rs:3329-3346`); metaharness keeps its hook program,
  settings, MCP configuration and transcript in a separate temporary directory
  (`crates/metaharness/src/builder.rs:926`), outside the `/workspace` a confined child sees, and
  Loom's runner refuses a writable root outside it (`check_record`,
  `crates/loom-intake-slice/src/confinement.rs:2833-2840`).
- A Claude Code session is pointed only at a keyless loopback endpoint whose far end adds the
  credential (`crates/loom-session-executor/src/lib.rs:1270-1297`); under `Kind::Claude` metaharness
  hands the runner credential copies to perform at the spawn (`crates/metaharness/src/process.rs:53`,
  `:86`).
- Substrate delivers a secret as a sealed descriptor named in `SUBSTRATE_SECRET_SLOTS`
  (`/proc/self/fd/N`), never as a path (Substrate ADR 0012); Loom's runner sends no slot
  (`secret_slots: Vec::new()`, `confinement.rs:1565`) and refuses a record naming one (`:2833`); the
  in-process `local` adapter refuses any slot (`crates/substrate-host/src/confinement/local.rs:367`);
  a daemon serves only slots its operator declared.
- An agent executor has no result table: `story:bound-agent-node` ends a session stopped at its
  timeout as `AgentBudgetSpent`, and the `timeout` key belongs to the `program` executor
  (`story:bound-program-result-table`).
- `tree run` handles signals as `stop_on_signals` does in the CLI's other hosts: the first signal
  lets the current item finish, the second exits 130 (`crates/loom-cli/src/main.rs:747-775`), which
  skips the `Drop` that kills a live `ConfinedChild`
  (`crates/loom-intake-slice/src/confinement/child.rs:100-110`).

## Waits on

- `decision-blocker:agent-harness-confinement-design`: the design of a confined session.
- `story:named-egress-per-action`: the story before this one in the chain, which edits
  `ess/domains/engine.yaml` and `ess/domains/bindings.yaml` before it.
- `story:action-limits-enforced`: the limits the session runs under.
- `story:bound-agent-node`: the `SpawnRunner` seam and `AgentRun`.

## ESS first

Written once the decision is recorded. It will extend `AgentRun` in `ess/domains/bindings.yaml`
with `confinement` (`loom.engine.StepConfinement`) and add to `ess/domains/engine.yaml` what the
decision names.

## Acceptance

Waiting on `decision-blocker:agent-harness-confinement-design`. What it must check, at least: the
vendor program is started by the engine's runner and no process of the session starts outside it,
metaharness's own `--version` and `git` included; the session cannot read a workspace file that is
not among the action's inputs; a session past its `timeout_seconds` is stopped with its whole
process tree and gives a `wall-time` `LimitHit` line, and the step ends as `story:bound-agent-node`
ends it; its credential is not in its environment or argv; a connection anywhere but the model
endpoint fails; stopping `tree run` leaves no process of the session; `task check` exits 0.

## Out of scope

Changes in metaharness made by this story. Codex and b10x-harness sessions. The Connectors
command-line invoker.
