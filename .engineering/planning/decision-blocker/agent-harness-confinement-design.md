---
format: aep.planning-md/3
id: decision-blocker:agent-harness-confinement-design
kind: decision-blocker
status: open
title: Nobody has decided how a Claude Code session gets its files, credential and model endpoint inside a Substrate confinement
summary: Six questions for the metaharness and Substrate owners, through session sdc; holds story:agent-session-through-substrate
tags:
- e2r4
relations:
- blocks: story:agent-session-through-substrate
revision: 1
---
## Question

How does a Claude Code session of an agent node run inside a Substrate confinement? Each of these
needs an answer from the owners of metaharness and Substrate:

1. How the operator's Claude login, or another credential, reaches a confined vendor program:
   metaharness's credential copies performed at the spawn, a Substrate secret slot (a sealed
   descriptor named in `SUBSTRATE_SECRET_SLOTS`, read at `/proc/self/fd/N`, never a path), or only
   the keyless loopback endpoint whose far end adds the credential.
2. If a slot: which Loom runner member sends it (it sends none today) and what the in-process
   `local` adapter, which refuses any slot, does with an agent step.
3. Where the model endpoint's far end runs, and which egress aperture the session selects to reach
   it.
4. How metaharness's own processes, the `--version` probe and `git` for context files, go through
   the injected runner or are switched off for a confined session.
5. Where metaharness's hook program, settings, MCP configuration and transcript live, given that a
   confined child sees `/workspace` and its declared read-only roots, and Loom's runner refuses a
   writable root outside the workspace.
6. Whether the session's working directory is the staging directory itself or the executor's
   scratch copy of it, as `/workspace`.

## Why it is open

Read on 2026-11-21, metaharness at tag `0.23.0`, Substrate at tag `0.21.0`:

- metaharness runs in Loom's process; `story:bound-agent-node` starts only the vendor program, through
  `SpawnRunner` (`crates/loom-session-executor/src/lib.rs:1398-1414`), and the executor takes a
  caller's runner (`lib.rs:1121-1125`);
- metaharness starts `--version` (`crates/metaharness/src/builder.rs:2565-2566`) and `git`
  (`crates/metaharness/src/context.rs:245`) itself, and keeps its session files in a temporary
  directory of its own (`builder.rs:926`);
- under `Kind::Claude` it hands the runner credential copies to perform at the spawn
  (`crates/metaharness/src/process.rs:53`, `:86`), while Loom accepts for a Claude Code session only
  a keyless loopback endpoint (`crates/loom-session-executor/src/lib.rs:1270-1297`);
- the session's working directory is the executor's scratch copy (`lib.rs:3329-3346`);
- Loom's runner sends no secret slot (`crates/loom-intake-slice/src/confinement.rs:1565`) and
  refuses an applied record naming a slot or a writable root (`:2833-2840`); Substrate's `local`
  adapter refuses any slot (`crates/substrate-host/src/confinement/local.rs:367`); Substrate
  delivers slots as sealed descriptors (Substrate ADR 0012).

Every way of confining the session answers these together; none is settled in Loom's plan.

## What it stops

`story:agent-session-through-substrate`: its ESS change and its acceptance are written once the
answer is recorded. `story:tree-daemon` does not wait on it; until it is answered, `tree serve`
refuses a tree with an agent step.

## Who

The owners of metaharness and of Substrate, through session `sdc`, which carries Timo's
requirement "every process the runtime starts goes through Substrate" (2026-11-21). Not yet sent.

## Source

Story scoper report on `story:agent-session-through-substrate`, 2026-11-21; plan revision e2r4 of
`epic:tree-daemon-under-enforced-limits`.
