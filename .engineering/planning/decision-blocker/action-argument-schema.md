---
format: aep.planning-md/3
id: decision-blocker:action-argument-schema
kind: decision-blocker
status: open
title: Nobody has decided where a frontier action's argument schema comes from
relations:
- blocks: epic:fast-selector
revision: 1
---
## Question

Where does the argument schema of a frontier action come from — the protocol definition, the
frontier action itself, or the Connector operation it is bound to — and is it one schema per action?

## Why it is open

Atlas ADR 0073 § Decision step 2 says "Arguments are schema validated", and
`docs/examples/laya-fast-selection.md` shows a schema for `release.inspect`, but the frontier action
Loom receives carries only an id and a status (inferred from Canon `crates/canon/src/lib.rs:41-44` at
`cf29c4b`, the revision Loom's `Cargo.lock` pins), and no ess/1 document declares an action
schema. The relation action → schema has no cardinality and no owner.

## What it stops

Schema validation of generated arguments in the fast-selection path. `story:laya-arguments-slice`
checks that arguments are generated for the selected action only and leaves validation out. The same
question likely touches `epic:loom-native-harness` (projecting actions into model tools, TASKBOARD
L-003 and L-006); that epic is not related here.

## Source

Decomposition of `epic:fast-selector`; Atlas ADR 0073; `docs/examples/laya-fast-selection.md`.
