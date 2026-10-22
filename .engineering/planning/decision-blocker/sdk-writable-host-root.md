---
format: aep.planning-md/3
id: decision-blocker:sdk-writable-host-root
kind: decision-blocker
status: cleared
title: Timo has not decided how a confined child gets a writable mount outside /workspace (Q2)
relations:
- blocks: story:writable-host-root
withholds: test_result
revision: 5
transitions:
- {from: "open", to: "cleared", at: "2026-10-22T14:06:55Z", actor: "human:timo", revision: 4}
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

## Decision (2026-10-22)

Timo, quoted by session `b10x-chef-orchestrator` on 2026-10-22: "either name physical paths, or
provide git source". Substrate accepted it as ADR 0038, "a writable root is a named host path or a
Git checkout" (`adr/0038-writable-roots-are-named-host-paths-or-git-checkouts.md`, status
`accepted`, beyond10x/substrate `main` at `44aacec7b0f593527e348100366ae30823aa30af`).

- Option B, narrowed to two kinds: a host path the operator names, or a Git source given to the run
  as a writable checkout.
- The orchestrator cleared this blocker on the accepted ADR (2026-10-22): "Close the blocker in
  your store with that citation (store-only, aep validate). Then stay idle until substrate 0.10.0
  ships."
- `story:writable-host-root` waits for the Substrate release that implements ADR 0038 (0.10.0).

## Landing (2026-10-22)

The branch carrying this clear began before the 0.34.0 release, and its pull request conflicted
with main on this file. It was brought up to date by a merge commit that takes main's version
and applies the decision again through `aep`; nothing was force-pushed. The orchestrator wrote
on 2026-10-22: "Accepted: merge main into the branch (bc41ec3e), a 1-file diff, no force-push."
