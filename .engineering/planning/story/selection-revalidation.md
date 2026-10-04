---
format: aep.planning-md/3
id: story:selection-revalidation
kind: story
status: draft
title: Revalidate a selected action at the execution boundary
refs:
- provider: taskboard
  reference: L-004
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:argument-generator
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Immediately before a selected action is handed to the execution adapter, Loom revalidates it against
the current frontier, case revision and authority, obtained through Commission ports (`Governor`,
`AuthorityProvider`) and never from the model. An action id that is not in the current frontier, or
a selection made at an older case revision, is refused at this boundary with the reason named, and
the adapter is not called. Loom asks for authority and obeys the answer; it decides none. Real
adapters are `epic:effect-bindings`; here the adapter is a fake that records calls.

## ESS first

Add the revalidation command on `loom.run.Selection` (lifecycle `Selected` to `Admitted` or
`Refused`) with its refusal outcomes (not in frontier, stale revision, authority denied); validate
with `ess specify validate --path ess`; regenerate the synthesized model.

## Domain relations

- `loom.run.Selection -> loom.run.ActionCatalogue` (relation `catalogue`) together with
  `ActionCatalogue.case_revision` give the revision a selection was made at — inferable from
  `ess/domains/run.yaml`, entities `loom.run.Selection` and `loom.run.ActionCatalogue`.

## Depends on, outside this store

Commission M-005 (`AuthorityProvider`) and M-008 (stale-revision action request).

## Acceptance

In a `b10x-loom` test, an action id absent from the frontier the fake governor returns at execution
time and a selection made at an older case revision are each refused at the execution boundary with
the reason named, while the fake execution adapter records zero calls.

## Source

TASKBOARD L-004; Atlas ADR 0072 (revalidate before every effect) and ADR 0073 step 3; the
execution-boundary clause of `epic:loom-native-harness`.
