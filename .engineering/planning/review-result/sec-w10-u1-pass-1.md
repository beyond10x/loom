---
format: aep.planning-md/3
id: review-result:sec-w10-u1-pass-1
kind: review-result
status: active
title: Security review, loom w10 U1 change evidence order, pass 1
relations:
- reviews: story:change-evidence-order
revision: 1
---
unit: U1 `story:change-evidence-order`, tree `lw10-u1` on `impl/change-evidence-order` (findings on `71f2cb6`, cases committed in `49c89cc`)
verdict: NEEDS-CHANGE; 6 cases added, 2 red, 4 green
origin: introduced 1 / pre-existing 0 / undecided 0

**Cases added:** `crates/loom-intake-slice/tests/security_w10_u1.rs` (6).

Red on `71f2cb6`:

| case | why |
|---|---|
| `security_a_tests_write_that_commits_nothing_makes_no_run_a_regression_result` | a `tests.write` that wrote what the files already held still marked later runs as after a tests write |
| `security_a_tests_write_that_commits_nothing_leaves_red_first_open` | so an already failing suite submitted a `regression_test_result` and discharged `red_first.required` |

Green, held: model arguments naming trusted record fields are refused; `observed_at` and the
producer come from the host, never from model text; an observation yields its records once; a
regression result needs a bugfix and a performed `tests.write`.

**Findings:**

- F1 (blocker, introduced): a no-op `tests.write` discharged `red_first.required`.

**Fixed** in `0a89ded` (spec: a regression result follows a committing `tests.write`) and
`7ca0e83`: later runs are regression results only after a `tests.write` that committed. All 6
cases pass on `7ca0e83`.
