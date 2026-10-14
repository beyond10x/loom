---
format: aep.planning-md/3
id: epic:loom-foundation-for-consumers
kind: epic
status: draft
title: Two external consumers build their agents on a released Loom
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Loom's harness and SDK are the foundation two external consumers build on: an agent platform
(consumer A) and an engineering factory (consumer C). Each pins a released Loom tag and runs its
agents through Loom's governed loop, with no fork and no copy of Loom code.

## Why

Loom replaced Harness as the one runtime repository. Consumers that built on Harness, or that
would otherwise write their own model/tool loop, take Loom's governed loop, Commission's contracts
and the governor from a release instead.

## Acceptance

- Each consumer's manifest pins a Loom release by tag (`tag = "<version>"`), and its build resolves
  one copy of each Loom crate it uses.
- Each consumer runs at least one agent of its own through `loom_sdk` (or `b10x-loom`) in its own
  repository, and its CI exercises that path on the pinned tag.
- A Loom release names, in its CHANGELOG, every break a consumer's pinned surface sees, and a
  consumer moves to it without patching Loom.

## Decomposition (to be drafted)

- The SDK surface each consumer needs, read from the consumer, not assumed: the embedding entry
  points, the ports a host implements (model, authority, suspension, case store, effects) and
  the stability Loom promises for them.
- A consumer-facing migration guide from Harness to Loom (`website/docs/guides/move-to-loom.md`
  is the start).
- A pinned-consumer check in Loom's CI: a minimal external crate that pins the workspace by path
  and builds the documented embedding, so a break shows before a release.

## Open questions

- Which consumer goes first, and which of its repositories is the integration point: unclear, to
  be asked when the epic is scoped.
- Whether consumers A and C read Loom only as Rust crates, or also through `b10x-loom` as a
  command: unclear, to be asked.

## Not in scope

Changes inside the consumers' repositories; hosted operation of Loom for them.
