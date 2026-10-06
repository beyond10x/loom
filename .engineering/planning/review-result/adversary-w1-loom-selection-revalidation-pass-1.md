---
format: aep.planning-md/3
id: review-result:adversary-w1-loom-selection-revalidation-pass-1
kind: review-result
status: active
title: Wave 2026-10-06-w1 adversary, loom story:selection-revalidation, pass 1
relations:
- reviews: story:selection-revalidation
revision: 1
---
unit: story:selection-revalidation
verdict: red
cases: executed 595→608, red 2
origin: introduced 2, pre-existing 0, undecided 0
wrote-outside-worktree: ~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-1/suite.log, ~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-1/callers-check.log, ~/.cache/b10x-target/loom-w1-selection-revalidation (assigned build dir, grew 2.0G→2.5G)
needs-coordinator: yes

The findings cover the worktree at head `aa7e04e`, plus my two untracked test files.

**1. Diff.** `git --no-pager diff --stat` prints nothing because I added only untracked files. `git status --short`:
```
?? crates/loom-executor/tests/adversary_w1_runtime_stale.rs        (297 lines)
?? crates/loom-executor/tests/adversary_w1_selection_boundaries.rs (659 lines)
```
Both are test files. I changed no implementation or existing test file, made no commits and wrote nothing under `.engineering/`.

**2. Cases I added**

| File | Cases | Now |
|---|---|---|
| `adversary_w1_runtime_stale.rs` | 2: a governed Loom used as the executor of Commission's `run_until_blocked`, with the case moving while arguments are generated | **red 2** |
| `adversary_w1_selection_boundaries.rs` | 11 boundary, ordering and concurrency cases | green 11 |

The red cases were run alone, first:
`CARGO_TARGET_DIR=~/.cache/b10x-target/loom-w1-selection-revalidation CARGO_INCREMENTAL=0 cargo test -p b10x-loom-executor --locked --test adversary_w1_runtime_stale` → exit 101
```
---- a_case_moved_while_arguments_are_generated_is_not_judged_on_the_left_frontier stdout ----
thread '...' panicked at crates/loom-executor/tests/adversary_w1_runtime_stale.rs:279:5:
with a governor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }), expected NoAdmissibleAction(Unit(true))
---- a_case_completed_while_arguments_are_generated_ends_completed stdout ----
thread '...' panicked at crates/loom-executor/tests/adversary_w1_runtime_stale.rs:255:5:
with a governor: NoAdmissibleAction(Unit(true)), expected Completed(RunOutcomeCompleted { outcome: "done" })
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```
After rustfmt the same assertions sit at `:270` and `:296`. Each test runs the same scenario twice: once with `Loom::new` and once with `.with_governor`. Only the governed half fails; the ungoverned half reaches the outcome Commission documents.

**3. Suite run**, after the cases existed: `cargo test -p b10x-loom-executor --locked --no-fail-fast` → exit 101.
- 608 executed: 606 passed, 2 failed, 1 ignored.
- The only failing binary is `adversary_w1_runtime_stale` (`FAILED. 0 passed; 2 failed`).
- `<before>` = 595, taken from the same run with my two binaries left out (608 − 13).
- Full log: `~/.cache/loom-wave-2026-10-06-w1/selection-revalidation/adversary-1/suite.log`.

**4. Findings**

- **F1. Commission ends the run from the frontier of a revision the case has already left** (`crates/loom-executor/src/lib.rs:387`)
  - What happens: a governed Loom answers `stale-revision` with `NoUsefulAction`. Commission's runtime then derives the run's outcome from that old frontier (`runtime.rs:549` → `outcome.rs:102-104`) instead of loading the case again.
  - What Commission documents: its runtime docs, item 8 (`runtime.rs:52`), say a stale proposal leads to a reload, which ends `Completed` if the case is complete and `NoAdmissibleAction` otherwise.
  - Measured: if the case is completed during argument generation, the run ends `NoAdmissibleAction` instead of `Completed`. If the left revision had an open obligation, the run ends `NeedsExternalEvidence(["tests-pass"])` for that superseded obligation.
  - The divergence needs the left frontier to admit nothing outright (only approval-gated actions, or open obligations), or the previous iteration to have been idle.
  - What reaches it: an embedder combining `Loom::with_governor` (announced in the CHANGELOG) with `run_until_blocked`; the SDK re-exports both (`loom-sdk/src/lib.rs:43-44`). No caller in this repo does that yet. The only production executor, `loom-intake-slice/src/run.rs:267`, uses `Loom::new` even though a `CanonGovernor` is in scope.
  - Fix, not applied: there are two options, both outside this unit's files:
    - Commission's runtime reloads `current_revision` before deriving after a non-proposal.
    - Decision 2's `NoUsefulAction` mapping for `stale-revision` is revisited. No `ExecutorOutcome` currently means "stale".
  - Verdict PLAUSIBLE, origin introduced.
- **F2. Suite gap: nothing pinned that revalidation runs after argument generation** (`selection_revalidation.rs:150`)
  - Both governed tests (`:150`, `:413`) use `EmptyObjectArguments`, which cannot observe or move the governor. Moving `revalidate` above `generate` would therefore leave the suite green.
  - I worked this out by reading the tests; I did not build a mutated copy, to save disk.
  - The new case `a_case_moving_during_argument_generation_is_refused_stale` (boundaries file `:407`) now pins the order.
  - Verdict CONFIRMED, origin introduced.

**5. Attacked, could not break** (all green)
- A current frontier at an older revision is refused stale.
- An empty current frontier refuses the selection.
- Duplicate Admissible and Blocked entries, in either order, are refused.
- Action ids differing only in case or whitespace (4 variants) are refused.
- Approval-gated actions: the same gate is proposed; a missing, blank or conflicting capability is refused.
- The frontier handed to `run` does not leak into revalidation, neither an action only it lists nor a revision only it claims.
- A panicking governor unwinds, nothing is admitted, and the same Loom works afterwards.
- A revision that moves between runs is read once per run.
- `Arc`, `Rc` and `Box<dyn Governor>` all work as governor holders.
- 8 threads × 25 runs on one Loom, mixing stale and current cases: every outcome and recorded revalidation stays with its own run.
- `Loom::new` callers still compile: `cargo check -p b10x-loom-intake-slice -p b10x-loom-sdk --all-targets --locked --offline` exits 0.
- The implementor's tests contain no "cannot fail" shapes.
- After formatting my files, `cargo fmt -p b10x-loom-executor --check` exits 0 and clippy `-D warnings` on my two test targets is clean.

**6. Paths written outside the worktree**
- The two logs named in the header.
- Build output into the assigned `~/.cache/b10x-target/loom-w1-selection-revalidation`.
- Nothing else. One stray `python3 -` with empty stdin ran, executed nothing and wrote nothing.
- Session lease `loom-w1-adv1`: started and ended (exit 0).

```findings
[
  {"file": "crates/loom-executor/src/lib.rs", "line": 387, "category": "contract-drift", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "A governed Loom answers stale-revision with NoUsefulAction, so Commission's run_until_blocked derives the run outcome from the frontier of the revision the case left (NoAdmissibleAction instead of Completed; NeedsExternalEvidence for a superseded obligation) instead of reloading as runtime.rs item 8 documents."},
  {"file": "crates/loom-executor/tests/selection_revalidation.rs", "line": 150, "category": "mutant", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "No case pinned revalidation after argument generation (every governed test uses EmptyObjectArguments), so moving revalidate above generate stays green; adversary_w1_selection_boundaries.rs:407 now pins it."}
]
```

Coordinator edit at recording: the findings block said `"verdict": "PLAUSIBLE"` for F1; the store accepts only CONFIRMED, NEEDS-CHANGE, INFEASIBLE, approve or needs-revision. F1 is recorded as CONFIRMED because two red cases measure it; the adversary's PLAUSIBLE qualified reachability (no caller in this repository composes `with_governor` with `run_until_blocked`), which the prose above keeps.
