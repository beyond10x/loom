---
format: aep.planning-md/3
id: story:selection-revalidation
kind: story
status: draft
title: Revalidate a selected action at the execution boundary
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-004
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:argument-generator
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/src/revalidation.rs
- confidence: inferred
  path: crates/loom/tests/selection_revalidation.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 6
---
## Outcome

Before Loom returns a `ProposedAction`, it revalidates the selection against the current frontier
and case revision, obtained through Commission's `Governor` port and never from the model. Loom's
own pre-check refuses two things, each with the reason named, and returns no `ProposedAction` for
either:

- an action id that is not in the catalogue projected from the frontier current at revalidation;
- a selection made on a catalogue at an older case revision than that frontier's.

A selection that passes both is returned as a `ProposedAction`. **The story ends there.** Loom
invokes no effect and calls no execution adapter. Who invokes the effect of a selected
consequential action, and so who rechecks authority immediately before it, is the open
`decision-blocker:effect-invocation-owner`; that blocker gets no new `blocks` edge, because this
story ends at returning the proposal and is not stopped by the answer. An approval-gated action
does not reach this check: it returns `Suspended` (`story:agent-executor`). Commission revalidates
again on its side (commission `story:stale-revision-action-request`, M-008).

## Shared surface

Link 5 of the `epic:loom-native-harness` chain over `ess/domains/run.yaml`, `generated/rust/loom/`
and `crates/loom/src/lib.rs`. It depends on `story:argument-generator`, and
`story:harness-loop-port` depends on it. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

Add the revalidation command on `loom.run.Selection` (lifecycle `Selected` to `Admitted` or
`Refused`) with its two refusal outcomes (not in frontier, stale revision); validate with
`ess specify validate --path ess`; regenerate with `task generate`.

## Domain relations

- `loom.run.Selection -> loom.run.ActionCatalogue` (relation `catalogue`) together with
  `ActionCatalogue.case_revision` give the revision a selection was made at — inferable from
  `ess/domains/run.yaml`, entities `loom.run.Selection` and `loom.run.ActionCatalogue`.
- The current case revision is that of `commission.responsibility.Frontier` (commission
  `ess/domains/responsibility.yaml:182-203` at `013e392`), whose actions commission
  `story:ess-hard-gate` declares as `Frontier.actions: List<FrontierAction>` (one frontier, many
  actions).

## Depends on, outside this store

`commission:story:ess-hard-gate` for the frontier's actions; `commission:story:governor-port`
(M-003) for the port that returns it; `commission:story:agent-executor-port` (M-004) for
`ProposedAction`.

## Scope

- `crates/loom/src/revalidation.rs` (new), `crates/loom/src/lib.rs`
- `crates/loom/tests/selection_revalidation.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (chain surface)

## Acceptance

The test `revalidation_refuses_before_proposing` in `crates/loom/tests/selection_revalidation.rs`
passes. With the Commission fake governor, it checks:

1. A selection whose action id is absent from the frontier the fake governor returns at
   revalidation is refused, the refusal names the id and the reason "not in frontier", and Loom
   returns no `ProposedAction`.
2. A selection made on a catalogue at case revision `n` while the fake governor returns revision
   `n + 1` is refused, the refusal names both revisions, and Loom returns no `ProposedAction`.
3. A selection that passes both checks is returned as a `ProposedAction` for that action id.
4. The fake governor is asked for the frontier once between the selection and Loom's return, in
   each of the three cases.

## Source

TASKBOARD L-004; Atlas ADR 0072 (revalidate before every effect) and ADR 0073 step 3; the
execution-boundary clause of `epic:loom-native-harness`; `docs/design/loom-design.md:48-70`.
