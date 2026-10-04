---
format: aep.planning-md/3
id: decision-blocker:effect-invocation-owner
kind: decision-blocker
status: open
title: Nobody has decided whether Loom or the Commission runtime invokes the effect of a selected consequential action
relations:
- blocks: story:connector-action-binding
- blocks: story:substrate-execution-binding
revision: 1
---
## Question

After Loom selects a consequential action and generates its arguments, who invokes the Connector
operation that carries the effect: Loom, inside its tool round trip, or the Commission runtime,
after Loom returns `ExecutorOutcome::ProposedAction`?

## Relation

Loom run -> effect invocation. Ownership: unknown. Cardinality and lifecycle coupling follow from the
owner and are unknown with it. No `ess/1` document declares it: `ess/domains/run.yaml` (`loom.run`)
has no effect entity, and the sources below disagree.

## Evidence

Loom invokes:

- Atlas `architecture/adr/0071-loom-is-the-native-harness.md` § Decision: Loom owns "tool round trips".
- `AGENTS.md:33`: "Revalidate every selected action against case revision, frontier and authority
  before execution."
- Atlas `epic:ga-governed-effects` § Ordering: L-014 and L-015 "land in the loom repository at the
  execution boundary".

The Commission runtime invokes:

- `docs/design/loom-design.md:48-70` § Commission integration: Loom projects, selects and fills
  arguments; then "Commission/runtime revalidates current frontier + authority", then "trusted adapter
  executes".
- Build pack `docs/contracts/commission-executor.md:31`: executor outcomes end at `ProposedAction`;
  no outcome reports an executed effect.
- `crates/loom/src/lib.rs:101` returns `ExecutorOutcome::ProposedAction` and invokes nothing
  (inferred from current code; nobody recorded this as a decision).

## What it stops

`story:connector-action-binding` (L-014) and `story:substrate-execution-binding` (L-015): where the
binding is consulted, where the invocation is made, and whether Loom ever sees a Connector attempt
reference for attribution all depend on the answer.

## Clears when

An accepted decision (Atlas ADR, or an amendment to ADR 0070/0071) names the component that invokes
the effect of a selected consequential action.
