---
format: aep.planning-md/3
id: story:substrate-execution-binding
kind: story
status: archived
title: Effects of bound actions run inside Substrate confinement
summary: Run the effect of a selected consequential action inside Substrate, observed from its operation ledger, with named refusal when a capability is missing (L-015).
refs:
- provider: taskboard
  reference: L-015
relations:
- decomposes: epic:effect-bindings
- depends_on: story:connector-action-binding
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-04T02:41:03Z", actor: "human:timo", revision: 3}
---
## Archived: taken over by Commission

Archived on 2026-10-04 after Atlas ADR 0082 (operator decision, option A;
`decision-blocker:effect-invocation-owner`, cleared): the Commission runtime, not Loom, invokes the
effect of a selected action, through its binding, Connectors inside Substrate. Nothing Loom-side
remains: Loom returns a `ProposedAction` and hands nothing to Substrate. The effect running inside
Substrate is taken over by commission `story:effect-invocation`;
`decision-blocker:connector-substrate-containment` stays open as a Commission follow-up. The text
below is the story as drafted for Loom and is kept unchanged as the record.

## Outcome

The effect of a selected consequential action runs inside Substrate confinement and is observed from
the Substrate operation ledger. When Substrate cannot provide the capability the effect needs, the
run ends with the named Substrate refusal, never with a weaker or unconfined execution.

## Acceptance

In a Loom run against a local Substrate daemon, the effect of a selected consequential action appears
as a `substrate.operations.AcceptedOperation` that reaches `Terminal`, and a run against a daemon that
does not serve the required capability ends with that refusal name and no effect.

## Blocked

Not ready to schedule. Two open decisions stop it, and neither answer is written here:

- `decision-blocker:connector-substrate-containment`: whether Substrate is the Connector provider the
  effect invokes, or the confinement the Connector invocation runs in.
- `decision-blocker:effect-invocation-owner`: whether Loom or the Commission runtime makes the
  invocation.

The acceptance observes only the Substrate ledger and refusal, so it holds under either reading of
the first decision.

## Domain relations

- `substrate.operations.AcceptedOperation` moves `Accepted` -> `Unknown` | `Terminal` - an entity
  lifecycle, not a relation; substrate `spec/domains/operations.yaml` at `aeeca78d`.
- `connectors.mutations.AttemptRecord` <-> `substrate.operations.AcceptedOperation`: UNMAPPED,
  `decision-blocker:connector-substrate-containment`.
- Loom run -> effect invocation: UNMAPPED, `decision-blocker:effect-invocation-owner`.

## Boundaries this story keeps

- A missing isolation or capability guarantee is a named refusal, never silent degradation; operations
  are durable before driver dispatch (substrate `AGENTS.md` invariants 3 and 5).
- Substrate embeds no sibling component: any binding code lives on the consumer side, never in
  Substrate (substrate `AGENTS.md` invariant 2). Substrate runs no agent loop (§ Out of scope).
- Loom holds no provider credential at any point (`epic:effect-bindings` § Acceptance); whether a
  Substrate credential is one, and who holds it, follows the first decision above.

## Sequencing

Depends on `story:connector-action-binding`: under either reading the effect is a bound Connector
operation first.

## Source

Governed Autonomy build pack `TASKBOARD.md` § Loom, L-015 "Add Substrate execution binding";
`epic:effect-bindings`; Atlas `epic:ga-governed-effects`.
