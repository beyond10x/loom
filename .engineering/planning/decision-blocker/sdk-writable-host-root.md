---
format: aep.planning-md/3
id: decision-blocker:sdk-writable-host-root
kind: decision-blocker
status: open
title: Timo has not decided how a confined child gets a writable mount outside /workspace (Q2)
relations:
- blocks: story:writable-host-root
withholds: test_result
revision: 2
---
## Question

How does a confined child get a writable mount outside `/workspace`: a linked worktree's common
`.git`, a harness configuration home? Options in the design (Q2): A adopted workspaces (cannot
reach a primary checkout's `.git`), B operator-declared named writable roots, bound by descriptor,
C none.

## Who decides

Timo, through a Substrate ADR. The design and the orchestrator recommend B. Substrate ADRs 0023,
0010 and 0033 hold the workspace as the one writable mount today, so B needs a new Substrate ADR.
Session `b10x-chef-orchestrator`, 2026-10-22: "Q2 (writable root) waits on Timo's substrate ADR
answer."

## What it withholds

`story:writable-host-root` and the `writable_roots` field of `story:confinement-spec`. The
Loom side (`ChildCommand::with_writable_root(name)`) is the same for A and B; C closes the story.

## Cleared when

A Substrate ADR on writable roots is accepted and a Substrate release implements it.

## Answer

Session `b10x-chef-orchestrator`, 2026-10-22, quoting Timo verbatim: "either name physical paths, or
provide git source". The orchestrator's reading: "A writable mount outside /workspace is either a
host path the operator names, or a git source given to the run as a writable checkout. Substrate
writes the ADR and the daemon side now. … shape the loom-sdk side to those two kinds once
substrate's ADR lands. Don't build ahead of it."

- Loom's side gets two kinds of writable mount: a host path the operator names, and a git source
  given to the run as a writable checkout.
- No Loom code changes before Substrate's ADR is accepted; the blocker stays open until then, as
  its § Cleared when says.
