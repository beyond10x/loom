---
format: aep.planning-md/3
id: story:connector-action-binding
kind: story
status: draft
title: Consequential actions leave Loom only as a ProposedAction
summary: Loom returns a ProposedAction for a selected consequential action, invokes no Connector operation and links no credential crate; the invocation is Commission's (ADR 0082, L-014).
refs:
- provider: taskboard
  reference: L-014
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 3
---
## Outcome

A consequential action selected in a Loom run leaves Loom only as a `ProposedAction`. **Loom's side
ends there.** Loom makes no Connector invocation, holds no handle to Connectors, and no provider
credential or unrestricted provider API reaches Loom merely because a Connection exists.

The invocation itself is not Loom's. Atlas ADR 0082 (operator decision of 2026-10-04, option A,
`decision-blocker:effect-invocation-owner`, cleared): the Commission runtime rechecks frontier, case
revision and authority immediately before the effect and then invokes through the action's binding.
The binding lookup, the invocation and the named refusal of an unbound action are taken over by
commission `story:effect-invocation`.

## Acceptance

In a Loom run that selects a consequential action, the executor returns a `ProposedAction` for that
action id and performs no invocation, and `cargo tree -p b10x-loom -e normal` lists no crate
implementing Connectors `auth.custody/v1alpha1` or `auth.capability/v1alpha1`.

## Blocked

One open decision touches the Loom side: `decision-blocker:action-operation-binding`. After ADR 0082
the question itself is Commission's (commission `decision-blocker:action-operation-binding`); what
stays open for Loom is whether an unbound action is still offered in the catalogue Loom projects, or
filtered out before Loom sees it. The acceptance holds under either answer.

## Domain relations

- Loom run -> effect invocation: none. Loom returns a `ProposedAction` and invokes nothing (Atlas
  ADR 0082).
- Frontier action -> Connector operation: UNMAPPED, commission
  `decision-blocker:action-operation-binding` (Loom's `decision-blocker:action-operation-binding`
  records the Loom-side remainder).

## Boundaries this story keeps

- Loom owns no connector credentials (`AGENTS.md:20-21`; `docs/design/loom-design.md:38-46`).
- Loom's revalidation before returning a `ProposedAction` is `story:selection-revalidation`
  (TASKBOARD L-004), not this story. The recheck immediately before the effect is Commission's
  (ADR 0082).
- An approval-gated action suspends the run (`crates/loom/src/lib.rs:93-97`). Obtaining a Mandate
  approval and resuming afterwards are Commission work (TASKBOARD M-007, I-003), not this story.

## Sequencing

Needs the executor (TASKBOARD L-002, `story:agent-executor`) and revalidation (L-004,
`story:selection-revalidation`) from `epic:loom-native-harness`. No `depends_on` edge to them is
recorded yet.

## Source

Governed Autonomy build pack `TASKBOARD.md` § Loom, L-014 "Add connector action binding";
`epic:effect-bindings`; Atlas `epic:ga-governed-effects`; Atlas ADR 0082.
