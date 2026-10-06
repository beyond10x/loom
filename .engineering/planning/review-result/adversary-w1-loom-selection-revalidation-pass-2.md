---
format: aep.planning-md/3
id: review-result:adversary-w1-loom-selection-revalidation-pass-2
kind: review-result
status: active
title: Wave 2026-10-06-w1 adversary, loom story:selection-revalidation, pass 2
relations:
- reviews: story:selection-revalidation
revision: 1
---
unit: story:selection-revalidation, head 7d013c9 plus two untracked test files
verdict: green
cases: executed 608→614, red 0
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: 3 logs in ~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-2/, plus build output in the assigned ~/.cache/b10x-target/loom-w1-selection-revalidation (3.0G→3.1G)
needs-coordinator: yes

No red case: I could not break the implementation. All four findings are about the correction's text and the test names. One of them (D1) needs a text change before this ships.

**1. Diff.** `git --no-pager diff --stat` prints nothing, because my two files are untracked. `git status --short`:
```
?? crates/loom-executor/tests/adversary_w1p2_runtime_windows.rs   (580 lines)
?? crates/loom-executor/tests/adversary_w1p2_selection_edges.rs   (216 lines)
```
Both are test files. I changed no implementation or existing test, made no commit and wrote nothing under `.engineering/`.

**2. Cases added.** Each file was run alone first. All six cases were green on that first run. They are written to pin behaviour, not to break it.

| Case | Asserts | Now |
|---|---|---|
| `runtime_windows.rs:348` `a_case_completed_while_the_selector_selects_is_judged_on_the_left_frontier` | Case completes during selection; generation saw `[true]` (moved). Ungoverned ends `Completed`, governed ends `NoAdmissibleAction` (today's outcome) | green |
| `runtime_windows.rs:386` `a_case_moved_while_the_selector_selects_is_judged_on_the_left_frontier` | Same window. Ungoverned ends `NoAdmissibleAction`, governed ends `NeedsExternalEvidence(["tests-pass"])` | green |
| `runtime_windows.rs:557`, `:571` | Nobody else moves the case; 3 steps whose effects move it, ending `Completed` and `NeedsAuthority`. The governed `LoopEnd` equals the ungoverned one | green 2 |
| `selection_edges.rs:105` | Stale-revision check is exact: 9 neighbour pairs (2^53, both ends of i64) refused, naming both revisions; 5 equal revisions proposed | green |
| `selection_edges.rs:175` | `with_governor` on a Loom that already ran keeps the record and the run numbering | green |

First-run output: `adversary_w1p2_runtime_windows`: `test result: ok. 4 passed; 0 failed` (EXIT=0). `adversary_w1p2_selection_edges`: `test result: ok. 2 passed; 0 failed` (EXIT=0). `rustfmt` on my two files only; fmt check exit 0; clippy `-D warnings` on both targets is clean.

**3. Suite**, run after the cases existed: `CARGO_TARGET_DIR=~/.cache/b10x-target/loom-w1-selection-revalidation CARGO_INCREMENTAL=0 cargo test -p b10x-loom-executor --locked --no-fail-fast` exited 0.
- Result: 45 binaries, 614 passed, 0 failed, 1 ignored. Both of my binaries ran from this tree's target.
- `<before>` is 608, from the implementor's correction gate log `c1-gate.log` (608 passed, 1 ignored).

**4. Findings**, covering 7d013c9:

| # | file:line | finding | verdict / origin / severity |
|---|---|---|---|
| D1 | `CHANGELOG.md:12-13`, `website/data/status.json:16` | The limitation names only "while arguments are generated". The cases at `:348` and `:386` show the same outcome when the case moves while the selector selects, and the slice's `ModelSelector` is a model call too | NEEDS-CHANGE / introduced (7d013c9) / warning |
| D2 | same lines, plus `adversary_w1_runtime_stale.rs:20-22`, `:279`, `:313` | "until a run outcome for a moved case exists" is not enough to end the limitation (reasoning below) | CONFIRMED / introduced / note |
| F3 | `adversary_w1_runtime_stale.rs:262`, `:290`, `:14-15` | Names `…_ends_completed` and `…_is_not_judged_on_the_left_frontier` pass while their governed half asserts the opposite, and the module doc says each case asserts the documented outcome. A green test list reads as if F1 were fixed | CONFIRMED / introduced (e8ae29f) / note |
| F5 | `website/docs/reference/ess/loom-run.md:447` | The generated reference says no input reaches `not-in-frontier`. Since this unit the executor decides it from `input.frontier_actions` alone (proven by `adversary_run_revalidation.rs:263`) | INFEASIBLE / introduced / note |

- **What reaches D1 and D2:** the same as pass 1 F1, an embedder that combines `with_governor` with `run_until_blocked`. No caller in this repo does that.
- **Fix for D1, not applied:** name the window as "between Commission reading the frontier and Loom's revalidation (while the selector selects or arguments are generated)". Also move the limitation after the sentence saying what Loom does. Right now it sits between the entry's first sentence and its elaboration (`CHANGELOG.md:11-17`).
- **Why D2 holds** (from reading the code, no test):
  - A governed Loom returns `NoUsefulAction`, and the runtime derives the outcome from the left frontier in the same iteration (`runtime.rs:549`).
  - Option A (a stale `RunOutcome`) only fires where the runtime itself sees the move, which it does not here.
  - Option B (a moved-case `SuspensionReason`) helps only if Loom maps `stale-revision` to `Suspended`, which reopens decision 2. Even then, a completed case ends `Suspended`, not `Completed`, because `Suspended` returns at `runtime.rs:461` before completion is consulted.
- **Why F5 cannot be fixed here:** the wording is ESS's generated text for every `external:` outcome, and decision 1 refused the spec-side change.

**5. Attacked, could not break**
- With nobody else moving the case, a governed Loom changes nothing about how a run ends (whole `LoopEnd` equal, two scenarios).
- The stale-revision guard compares decimal strings exactly, with no rounding through `f64`.
- `with_governor` after earlier runs: no reused selection id or argument-request id.
- `CanonGovernor::frontier` only reads (`loom-governor/src/lib.rs:556`), so Loom's extra call does not change what Commission's revalidation sees.
- A `not-in-frontier` refusal at the same revision ends the run the same way with or without a governor: rule 6 of `outcome.rs` handles `NoUsefulAction` and a refused proposal alike. The divergence happens only for stale revisions (F1).
- The status row's "b10x-loom run does not enable it yet" is true: `loom-intake-slice/src/run.rs:267` uses `Loom::new`.
- Seven mutants of the diff each break an existing case. This is from reading the code; I did not run mutated copies.

**6. Paths written outside the worktree**
- `~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-2/windows-alone.log`
- `~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-2/edges-alone.log`
- `~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-2/suite.log`
- Build output into `~/.cache/b10x-target/loom-w1-selection-revalidation`.
- Session lease `loom-w1-adv2`: started and ended, exit 0.

```findings
[
  {"file": "CHANGELOG.md", "line": 12, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The CHANGELOG and the status row (website/data/status.json:16) limit the left-frontier judgement to a case that moves while arguments are generated, but a case that moves while the selector selects ends the same way (adversary_w1p2_runtime_windows.rs:348, :386)."},
  {"file": "CHANGELOG.md", "line": 13, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "'until a run outcome for a moved case exists', repeated in status.json:16 and adversary_w1_runtime_stale.rs:279/:313, is not enough: the runtime derives from the left frontier after Loom's NoUsefulAction (runtime.rs:549), and a moved-case Suspended would return before completion is consulted (runtime.rs:461), so neither blocker option alone yields Completed/NoAdmissibleAction."},
  {"file": "crates/loom-executor/tests/adversary_w1_runtime_stale.rs", "line": 262, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "The tests named ..._ends_completed (:262) and ..._is_not_judged_on_the_left_frontier (:290), and the module doc at :14-15, state the documented outcome while the governed half now asserts the opposite, so a green test list reads as if F1 were fixed."},
  {"file": "website/docs/reference/ess/loom-run.md", "line": 447, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "The generated reference says no input reaches not-in-frontier, while the executor now decides it from input.frontier_actions alone; the wording is ESS's generated text for external outcomes and the spec route was refused by decision 1."}
]
```
