---
format: aep.planning-md/3
id: story:connector-action-binding
kind: story
status: active
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
scope:
- confidence: cited
  path: crates/loom-executor/tests/connector_boundary.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 7, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
- {from: "proposed", to: "active", at: "2026-10-07T02:51:19Z", actor: "human:timo", revision: 8, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
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

The test `consequential_actions_leave_loom_only_as_a_proposal` in
`crates/loom-executor/tests/connector_boundary.rs` passes:

1. A Loom run on a frontier admitting `repository.merge`, whose scripted selector picks it, returns a
   `ProposedAction` for `repository.merge` with its arguments; Loom is handed no effect port, so
   nothing can be invoked.
2. The dependency graph of `b10x-loom-executor` (normal edges, transitively, read from
   `cargo metadata --locked --offline`) holds no package whose name starts with `connectors` or
   `b10x-connectors`. The same check, applied to a recorded metadata fixture that adds such a
   package, fails and names it.

No specification change: the story guards behaviour that exists (`cargo tree -p b10x-loom-executor
-e normal --offline` on `657c8fa` lists no Connectors crate). The red test is the fixture half of
item 2, written first against a check that does not exist yet.

## Blocked

Not blocked since 2026-10-07: `decision-blocker:action-operation-binding` is cleared with option B. The
host's Commission composition binds an action to one Connector operation, and the Commission runtime
removes every unbound action from the frontier before an executor runs (`story:effect-invocation`),
so Loom never projects an unbound action into its catalogue. The acceptance below is unchanged by
that answer; nothing in Loom filters bindings.

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
