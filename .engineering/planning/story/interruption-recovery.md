---
format: aep.planning-md/3
id: story:interruption-recovery
kind: story
status: draft
title: Define interruption and recovery
refs:
- provider: taskboard
  reference: L-013
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:session-transcript-streaming
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-loop-port
scope:
- confidence: cited
  path: crates/loom/src/lib.rs
- confidence: inferred
  path: crates/loom/src/recovery.rs
- confidence: inferred
  path: crates/loom/tests/interruption_recovery.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/
revision: 7
---
## Outcome

A Loom run can be cancelled at any point and recovered from its filed session, and a run suspended
at an approval resumes from its checkpoint before the exact effect (the Harness approval checkpoint,
`harness-loop/src/approval.rs`, and `LoopStop::Cancelled`, at `798325f0`). Recovery trusts nothing
that was in flight: it re-projects from the frontier current at resume and revalidates
(`story:selection-revalidation`) before returning any `ProposedAction`, so a selection made before
the interruption cannot be proposed against a moved case.

## Shared surface

Behavioural edges: `story:session-transcript-streaming` (recovery resumes the filed session by
id), `story:selection-revalidation` (recovery revalidates before proposing) and
`story:harness-loop-port` (a resumed run re-projects and selects through the wired loop). Depends on
`story:run-pipeline-skeleton` for the `recovery` module. The former edge to
`story:compaction-contract` was ordering-only (both edit `loom.run.Session`) and was dropped on
2026-10-04; the shared `ess/` and `generated/` paths still keep the two in separate waves.
`story:loom-ess-conformance` depends on it. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

- **First commit:** add the interrupt and resume commands and their outcomes on `loom.run.Session`
  in `ess/domains/run.yaml`; `ess specify validate --path ess` passes; nothing else changes. Whether
  this resume is the resume `story:session-transcript-streaming` declares, extended, or a second
  command is settled before that commit, and the story says which.
- **Red on it:** `task drift` fails, naming the first file of `generated/rust/loom/` that differs
  from the model regenerated from the changed specification.
- **Then:** `task generate`; the test `interruption_recovery`; the implementation that makes it
  pass.

## Domain relations

- `loom.run.Session -> loom.run.Turn` (relation `turns`, owns) and `loom.run.Selection ->
  loom.run.ActionCatalogue` (relation `catalogue`) for the revision a pre-interruption selection was
  made at — inferable from `ess/domains/run.yaml`.

## Depends on, outside this store

`commission:story:run-outcomes` (M-007) for the Run's suspend and resume, and
`commission:story:authority-provider-port` (M-005) for the fake that grants the approval.

## Scope

- `crates/loom/src/recovery.rs` (created empty by `story:run-pipeline-skeleton`, filled here),
  `crates/loom/src/lib.rs` (where cancellation and resume hook into the run)
- `crates/loom/tests/interruption_recovery.rs` (new)
- `ess/domains/run.yaml`, `generated/rust/loom/` (its own `Session` declarations)

## Acceptance

The test `interruption_recovery` in `crates/loom/tests/interruption_recovery.rs` passes. With the
Commission fakes, it checks:

1. A run cancelled after a selection, and resumed by session id after the fake governor advanced
   the case revision, projects its first catalogue at the new revision.
2. That resumed run returns no `ProposedAction` for the pre-interruption selection; the
   revalidation refusal names the stale revision.
3. A run suspended at the merge approval, resumed by session id after the fakes grant the approval
   with the frontier otherwise unchanged, returns a `ProposedAction` for `repository.merge`, and the
   selector is not called again between the suspension and that return.
4. A run suspended at the merge approval, resumed after the fake governor advanced the case
   revision, re-projects at the new revision before returning anything.

## Source

TASKBOARD L-013; Atlas ADR 0071 and 0072; Harness `harness-loop/src/approval.rs` at `798325f0`.

## From wave 2026-10-04-w9 (run-pipeline-skeleton, adversary pass 2)

`SelectAction` and `RequestArguments` declare no `existing_instance:` refusal, so a retried or
replayed `SelectAction` with the id of a refused selection resets it to `Selected`, and revalidation can
admit it. The fix (`existing_instance: true`) makes ESS 0.52.0 refuse the synthesized suite
(ESS-SYNTH-004: the route through `SelectAction/selected` is reached by no input). Needs an ESS change
before recovery can rely on selection identity; the adversary case
`a_refused_selection_is_never_selected_or_admitted_again` is ignored until then.

## Carried from story:argument-generator (wave 2026-10-04-w15)

Selection and argument-request ids are derived from the frontier id and a per-`Loom` run counter
(`run_id` in `crates/loom/src/lib.rs`), so they are unique only within one `Loom`. A recovered
`Loom` restarts the counter at 0: recovery must not reuse a run id an earlier `Loom` gave, for
example by adding a per-`Loom` namespace to the hash. The adversary case that shows the collision
is `two_looms_give_their_different_selections_on_one_frontier_different_ids`.
