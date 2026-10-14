---
format: aep.planning-md/3
id: review-result:adv-w10-u1-pass-1
kind: review-result
status: active
title: Adversary, loom w10 U1 change evidence order, pass 1
relations:
- reviews: story:change-evidence-order
revision: 1
---
unit: U1 `story:change-evidence-order`, tree `lw10-u1` on `impl/change-evidence-order` (findings on `71f2cb6`, cases committed in `49c89cc`)
verdict: NEEDS-CHANGE; 2 cases added, both red; a third case added with the fix
origin: introduced 2 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-intake-slice/tests/adversary_w10_u1.rs` (2, then 3).

Red on `71f2cb6`:

| case | why |
|---|---|
| `adversary_a_facts_producer_canon_cannot_read_never_decides_whether_the_record_applies` | a `producer` member in a record's facts that Canon cannot read (null, a number, an object) refused the record, so model-shaped facts decided whether a trusted record applied |
| `adversary_an_edit_before_the_regression_test_is_never_shown_first_by_a_clock_step_back` | a host clock that stepped back gave an edit made before the regression test an earlier instant, so `regression.shown_first` read true |

**Findings:**

- F1 (blocker, introduced): a host clock step-back reorders the evidence of a bugfix.
- F2 (warning, introduced): a facts producer decides whether a record applies.

**Fixed** in `0a89ded` (spec: an observation's instant is strictly later, in whole seconds, than
the previous one in the run), `e247dc9` (the executor waits at most 2 s for the clock to pass the
previous second and otherwise fails the step naming both instants; a new case records two
observations read within one second in consecutive seconds) and `800ad3c` (the facts producer
is removed before Canon reads the facts). All 3 cases pass on `800ad3c`.
