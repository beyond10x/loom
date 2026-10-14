---
format: aep.planning-md/3
id: story:dw-workflow-into-protocol
kind: story
status: draft
title: After measurement, the workflow definition moves into a protocol and runs with the same counts
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-bugfix-workflow
- depends_on: story:dw-incident-workflow
revision: 1
---
## Outcome

Once the two reference workflows are measured, the parts of a Loom workflow definition that a
protocol should own are moved into a protocol construct, and Loom runs the protocol's definition
with the same executor and the same measurements: no reference workflow gains a model call, and a
re-run proposes the same deterministic steps.

## Why

The workflow definition is a Loom host construct today: a Loom-side definition sequences the actions
the protocol's frontier admits, and Canon still decides admission and completion. That keeps the
first workflows inside Loom; a definition that every host of a protocol should share belongs with
the protocol once measurement shows which parts are stable.

## Design

To be written when the story is ready, from the measurements of `story:dw-bugfix-workflow` and
`story:dw-incident-workflow`: which fields of `loom.workflow.WorkflowDefinition` move (step order,
conditions, typed outputs, concurrent reads), which stay host configuration (commands, sources,
credentials references), and how Loom reads a protocol's definition from the catalog
(`crates/loom-protocols`).

## Domain relations

Not enumerated: they depend on where the definition finally lives (Open).

## Acceptance

The bug-fix and incident workflows run from a protocol-held definition with the same model-call
counts per step as from the Loom definition (test names to be set when the story is ready:
`crates/loom-sdk/tests/bugfix_workflow.rs` and `incident_workflow.rs` run once per definition
source and compare the reports).

## ESS first

To be set when the story is ready: the Loom side is a change to `ess/domains/workflow.yaml` (the
definition read from the catalog), red on `task drift`; the protocol side is outside Loom.

## Scope

Not assessed yet. Likely `ess/domains/workflow.yaml`, `generated/rust/loom/`,
`crates/loom-protocols/src/lib.rs`, `crates/loom-executor/src/workflow.rs`, the reference
workflows' tests; and, if the definition moves to Canon or engineering-protocols, the pins in
`Cargo.toml` and `Cargo.lock`.

Not ready.

## Open

- Where the definition finally lives (Canon, engineering-protocols, or a Loom-owned protocol such as
  `inbound-answer@1`) is decided when this story is ready, from the measurements of the two
  reference workflows.

## Not in scope

The Loom host definition itself (`story:dw-workflow-definition-executor`); new workflows.
