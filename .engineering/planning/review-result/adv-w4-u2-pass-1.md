---
format: aep.planning-md/3
id: review-result:adv-w4-u2-pass-1
kind: review-result
status: active
title: Adversary, loom w4 U2 fallback selection recording, pass 1
relations:
- reviews: story:fallback-selection-recording
revision: 1
---
unit: U2 `story:fallback-selection-recording`, tree `lw4-u2` on `impl/fallback-selection-recording` (HEAD `3fc3645`, unit `8a3526f..3fc3645`)
verdict: PASS on the acceptance; 4 red cases, fixed in the unit
cases: executed 784→801, red 4
origin: introduced 4 / pre-existing 0 / undecided 0

**Cases added** (untracked, then committed with the fix): `crates/loom-executor/tests/adversary_w4_fallback_overrule.rs` (5), `crates/loom-executor/tests/adversary_w4_fallback_pipeline.rs` (9), `crates/loom-conformance/tests/adversary_w4_fallback_conformance.rs` (3).

Red:

| case | failed at |
|---|---|
| `adversary_w4_a_selection_overruled_by_itself_is_refused` | `RequestRecord::overrule` accepted a selection as its own replacement |
| `adversary_w4_a_replacement_that_is_itself_overruled_is_refused` | an `Overruled` replacement was accepted (two selections overruling each other) |
| `adversary_w4_a_replacement_from_another_catalogue_is_refused` | a replacement from another catalogue was accepted |
| `adversary_w4_loom_select_carries_the_strategy_of_the_selector_that_made_the_pick` | `Loom::select` returned the reasoning model's pick under `Hybrid` while `Loom::prepare` records `ReasoningModel` |

Suite after the cases: `cargo test --locked -p b10x-loom-executor -p b10x-loom-conformance -p b10x-loom-selector-laya --no-fail-fast`: 797 passed, 4 failed, 1 ignored.

**Findings**

| # | file:line | finding | verdict | what reaches it |
|---|---|---|---|---|
| F1 | `crates/loom-executor/src/arguments.rs:140` | `RequestRecord::overrule` checks only that the replacement is held | INFEASIBLE through `Loom::prepare` | direct callers of the public method only |
| F2 | `crates/loom-executor/src/lib.rs:260` | `Loom::select` and `Loom::prepare` disagree on a fallen-back pick's strategy | CONFIRMED (note) | callers of `Loom::select` |

Decided for the unit: `RequestRecord::overrule` refuses a replacement equal to the selection, not `Selected`, or from another catalogue (host-side; the generated behaviour stays unchanged); `Loom::select` goes through `selection::resolve`.

**Could not break:** the overruled pick never reaches arguments or revalidation; the chosen selection keeps its id; a stronger selector that errs or leaves the catalogue leaves no selection; an out-of-range fast confidence falls back and is recorded as none; nested hybrids record leaf strategies; the `Selections` view publishes `replaced_by`; the codec writes `Overruled`; no hand-written type shadows an ESS one; the first commit changes only `ess/`.
