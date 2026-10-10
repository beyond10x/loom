---
format: aep.planning-md/3
id: story:fallback-selection-recording
kind: story
status: implemented
title: A confidence fallback is recorded as two linked selections
relations:
- decomposes: epic:fast-selector
- depends_on: story:confidence-fallback
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:ess-057-upgrade
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-conformance/src/codec.rs
- confidence: cited
  path: crates/loom-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/arguments.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/revalidation.rs
- confidence: cited
  path: crates/loom-executor/src/selection.rs
- confidence: cited
  path: crates/loom-executor/tests/action_selector.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary2_run_identity.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_run_revalidation.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w2_conformance_select_action.rs
- confidence: inferred
  path: crates/loom-executor/tests/fallback_selection_recording.rs
- confidence: cited
  path: crates/loom-selector-laya/tests/laya_arguments_slice.rs
- confidence: inferred
  path: docs/contracts/loom-action-selection.md
- confidence: cited
  path: ess/domains/run.yaml
- confidence: cited
  path: generated/rust/loom/PLAN.md
- confidence: cited
  path: generated/rust/loom/plan.json
- confidence: cited
  path: generated/rust/loom/src/behaviour.rs
- confidence: cited
  path: generated/rust/loom/src/run.rs
- confidence: cited
  path: website/data/ess/loom-run.domain-graph.json
- confidence: inferred
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/ess
revision: 29
transitions:
- {from: "draft", to: "proposed", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 26}
- {from: "proposed", to: "active", at: "2026-10-10T02:04:40Z", actor: "human:timo", revision: 27}
- {from: "active", to: "implemented", at: "2026-10-10T04:21:19Z", actor: "human:timo", revision: 29, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A confidence fallback is recorded as `decision-blocker:fallback-selection-record` (option B)
decided: two `loom.run.Selection`s, the fast selection and the stronger selector's selection that
replaced it. The fast one references its replacement, zero or one; a fast selection that was not
overruled has none. Both belong to the run's turn. Each recorded selection carries the strategy of
the selector that made it (the fast selector's, or the stronger selector's), so Metaharness can
tell which selector each pick came from.

## ESS first

Settled at scoping (2026-10-10) by a trial on a copy of `ess/` with `ess` 0.57.0; validate
`--strict-requires`, compile and `ess verify conform synthesize` pass with 53 scenarios and
0 refusals (46 unchanged). The system is `format: ess/22`. The first commit changes only
`ess/domains/run.yaml`:

- `loom.run.Selection` gains field `replaced_by: Optional<loom.run.SelectionId>` and relation
  `replacement` (kind `references`, target `loom.run.Selection`, cardinality `one`, via
  `replaced_by`). ESS has only `one` and `many`; the `Optional` field makes it zero or one.
- Its lifecycle gains the terminal state `Overruled` and the transition `overrule` from
  `[Selected]`. An overruled selection is never revalidated and never given arguments
  (`RequestArguments` refuses a selection not in `Selected`).
- New command `loom.run.OverruleSelection` (input `selection_id`, `replacement_id`): outcome
  `overruled` moves `Selection.overrule`, sets `replaced_by`, emits new event
  `loom.run.SelectionOverruled {selection_id, replacement_id}`; outcome `wrong-state` is
  `SelectionStateConflict`. A `when_related` guard on the replacement beside `wrong_state` is refused
  by ESS (`ESS-COMMAND-004`), so the `OverruleSelection` behaviour on the record checks that the
  replacement exists and refuses with `SelectionNotFound` naming it.
- The `loom.run.Selections` view publishes `replaced_by`.

Red on that commit: `crates/loom-conformance/tests/conform.rs` (`ess_conformance_report`, the new
`OverruleSelection` scenarios reach the target's unknown-command arm) and `task drift`.

## Implementation notes

- `ActionSelector::select` returns one `Choice`; the hybrid drops the fast pick
  (`crates/loom-executor/src/selection.rs`, `HybridSelector`). Add a provided method on
  `ActionSelector` that returns the chosen pick and, when it overruled one, the overruled pick; its
  default wraps `select`, so no other selector changes.
- `Loom::prepare` (`crates/loom-executor/src/lib.rs`) records the fast selection, then the
  replacement, then overrules the fast one; only the replacement goes on to arguments and
  revalidation. A second selection id is minted beside the first.
- `crates/loom-selector-laya/tests/laya_arguments_slice.rs` expects one `Hybrid` selection; on a
  fallback it now sees two, with their selectors' strategies. The CHANGELOG names the change.

## Acceptance

Named test `crates/loom-executor/tests/fallback_selection_recording.rs`: when the fast selection is
overruled, Loom's record holds both selections, the fast one in `Overruled` naming the
replacement's `selection_id`; when it is not overruled, the record holds one selection with no
replacement; a fast selector that errs or names an action outside the candidates leaves no fast
selection (`selection::chosen` refuses it before a `Selection` exists); `OverruleSelection` naming
an unknown replacement is refused with `SelectionNotFound`. `task conform` and `task drift` pass.

## Dependencies

`story:confidence-fallback` (the hybrid selector this records); `story:ess-057-upgrade` (the trial
needed 0.57.0). `story:selection-telemetry` records `fell_back_to` on the same selections and
follows this story.

## Source

Split out of `story:confidence-fallback` at scoping, wave 2026-10-09-w1: the story's acceptance
checks only which choice is returned, and the recording needs a specification change, shown to
validate with ESS 0.57.0 on 2026-10-10.

## Settled at implementation (2026-10-10)

- The record implements the generated `OverruleSelectionBehavior` unchanged, and the conformance
  target dispatches `OverruleSelection` through it: the synthesized `overruled` scenario names a
  replacement no step creates. The host check is a separate record method,
  `RequestRecord::overrule`, refusing an unknown replacement with `SelectionNotFound`; `Loom::prepare`
  and the acceptance test use it.
- An accepted fast pick under a hybrid records the fast selector's strategy (`FastTyped`), not
  `Hybrid`; laya's test expects that.
- When the stronger selector errs or its choice is refused, no fast selection is recorded.
- The replacement's id is minted in `Loom::prepare` from the first selection id; the proposed
  selection keeps the existing id.
- Nested hybrids record only the outermost overrule (the relation is zero or one).
