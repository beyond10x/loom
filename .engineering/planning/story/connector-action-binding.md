---
format: aep.planning-md/3
id: story:connector-action-binding
kind: story
status: draft
title: Consequential actions execute only through their bound Connector operations
summary: Bind a selected consequential frontier action to the Connector operations it may invoke, refuse unbound actions by name, keep provider credentials out of Loom (L-014).
refs:
- provider: taskboard
  reference: L-014
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A consequential action selected in a Loom run reaches the world only through the Connector operations
bound to it. An action with no binding produces no effect and ends the run with a named refusal, and
no provider credential or unrestricted provider API reaches Loom merely because a Connection exists.

## Acceptance

In a Loom run against a recording Connectors fake, a selected consequential action invokes only the
Connector operations bound to it, an unbound one ends the run with a named refusal and no invocation,
and `cargo tree -p b10x-loom -e normal` lists no crate implementing Connectors `auth.custody/v1alpha1`
or `auth.capability/v1alpha1`.

## Blocked

Not ready to schedule. Two open decisions stop it, and neither answer is written here:

- `decision-blocker:action-operation-binding`: who declares which Connector operations an action
  binds to, and how many.
- `decision-blocker:effect-invocation-owner`: whether Loom or the Commission runtime makes the
  invocation.

The acceptance is stated so that it holds under every answer; the design is not. Once both clear,
extend the `loom.run` ESS domain (`ess/domains/run.yaml`) with the binding before implementing
(planning guardrail 7).

## Domain relations

- Connector invocation -> Connection: one `connectors.mutations.AttemptRecord` references exactly one
  `connectors.auth_bindings.Connection` (relation `connection`, references, cardinality one, via
  `connection_ref`) and one `connectors.declarations.ServiceConfiguration` (relation `instance`) -
  inferable, connectors `ess/domains/mutations.yaml:120-133` at `c7a9d5b1d`.
- Approval spend -> Connector invocation: one `connectors.delegation.ApprovalRedemption` references one
  `AttemptRecord` (relation `attempt`, cardinality one, via `attempt_id`) - inferable, connectors
  `ess/domains/delegation.yaml:262-272` at `c7a9d5b1d`.
- Frontier action -> Connector operation: UNMAPPED, `decision-blocker:action-operation-binding`.
- Loom run -> effect invocation, and whether Loom holds a reference to the resulting `AttemptRecord`:
  UNMAPPED, `decision-blocker:effect-invocation-owner`.

## Boundaries this story keeps

- Loom owns no connector credentials (`AGENTS.md:20-21`; `docs/design/loom-design.md:38-46`).
- Every selected action is revalidated against case revision, frontier and authority before any
  effect (`AGENTS.md:33`; Atlas ADR 0072 § Rule). Revalidation itself is TASKBOARD L-004, not this
  story.
- An approval-gated action suspends the run (`crates/loom/src/lib.rs:93-97`). Obtaining a Mandate
  approval and resuming afterwards are Commission work (TASKBOARD M-007, I-003), not this story.

## Sequencing

Needs the executor (TASKBOARD L-002) and revalidation before execution (L-004) from
`epic:loom-native-harness`. No story for either existed in the store when this one was drafted, so
no `depends_on` edge to them is recorded yet.

## Source

Governed Autonomy build pack `TASKBOARD.md` § Loom, L-014 "Add connector action binding";
`epic:effect-bindings`; Atlas `epic:ga-governed-effects`.
