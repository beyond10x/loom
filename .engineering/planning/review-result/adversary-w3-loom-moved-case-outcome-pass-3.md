---
format: aep.planning-md/3
id: review-result:adversary-w3-loom-moved-case-outcome-pass-3
kind: review-result
status: active
title: Wave 2026-10-07-w3 adversary, loom story:moved-case-outcome, pass 3
relations:
- reviews: story:moved-case-outcome
revision: 1
---
unit: story:moved-case-outcome, impl/moved-case-outcome at 43de0b0 (base b8af5c2), plus 1 untracked adversary test file
verdict: NEEDS-CHANGE
cases: executed 880→883, red 3
origin: introduced 0 / pre-existing 0 / undecided 2
wrote-outside-worktree: none
needs-coordinator: (1) confirm both origins against b8af5c2: no base tree was assigned and I did not build one; (2) decide who reports a move that happens after the executor's last read: `LoopExecutor`, with one more governor read, or the runtime, by reloading before it ends a run on the handed frontier

**1. `git --no-pager diff --stat`**: empty, so no tracked file changed. `git status --short`:
```
?? crates/loom-executor/tests/adversary3_w3_moved_case_loop.rs
```

**2. Cases added.** I wrote all three before anything ran, then ran the file alone. All three were red on that first run, and they are red now. Fixture: the case at revision 7 lists one approval-gated action, `repository.edit`, with `tests-pass` open. It moves to revision 8, where it is either complete or open with `review-approved`.

| Case (`adversary3_w3_moved_case_loop.rs`) | Scenario | Expected | Measured |
|---|---|---|---|
| `:490` `…completed_during_the_models_only_turn…in_prose` | The case completes during the model's only turn, and the model answers in prose. | `Completed` | `NeedsExternalEvidence(["tests-pass"])` |
| `:526` `…moved_on_during_the_models_only_turn…in_prose` | Same window; the case moves on to `review-approved`. | `NeedsExternalEvidence(["review-approved"])` | `NeedsExternalEvidence(["tests-pass"])` |
| `:563` `…pipeline_selector_finds_nothing_ends_completed` | The pipeline Loom (`Loom::run`) with a governor: the case completes while the selector is asked, and the selector returns `NothingAdmissible`. | `Completed` | `NeedsExternalEvidence(["tests-pass"])` |

Preconditions held in the two loop cases: the case moved, the loop offered one catalogue, at revision 7, and nothing was revalidated. The pipeline reference, with the case moving during argument generation, reached the decided outcome both times. Red output of the first run (`scratch/adversary-3/red-alone.log`), verbatim:
```
LoopExecutor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }) after 0 step(s), 0 request(s), expected Completed(RunOutcomeCompleted { outcome: "done" }): ... Catalogues projected at revisions [7]; revalidations []
LoopExecutor: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }) after 0 step(s), 0 request(s), expected NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["review-approved"] }): ... Catalogues projected at revisions [7]; revalidations []
the pipeline Loom: NeedsExternalEvidence(RunOutcomeNeedsExternalEvidence { requirements: ["tests-pass"] }) after 0 step(s), 0 request(s), expected Completed(RunOutcomeCompleted { outcome: "done" }): ... Catalogues projected at revisions []; revalidations []
test result: FAILED. 0 passed; 3 failed   EXIT=101
```

**3. Suite run, after the cases existed.**
- Command: `cargo test --locked --no-fail-fast -p b10x-loom-executor -p b10x-loom-commission -p b10x-loom-commission-testkit`, with ESS 0.55.0 first on `PATH`.
- Result: 117 result lines, passed 880, failed 3, ignored 1, EXIT=101 (`error: 1 target failed: -p b10x-loom-executor --test adversary3_w3_moved_case_loop`).
- The only failures are my three cases. The `before` figure of 880 is this run with my binary left out; it matches the implementor's round-3 gate count.
- My first clippy run failed on my own file (`enum_variant_names`). I renamed the test enum (`Moves` became `During`) without touching any assertion. After that, clippy on `b10x-loom-executor --all-targets -D warnings` exited 0, `rustfmt --check` exited 0, and the file run alone is still 3 of 3 red.

**4. Findings** (they cover 43de0b0)

- **A. A move during the model's last turn is not reported** (`crates/loom-executor/src/harness/governed.rs:313`).
  - What fails: the rule only knows a move it happened to read, either from a stale-revision refusal or from a newer catalogue. A model that answers in prose produces neither. The run ends `CompletedLocalReasoning`, and Commission judges it on the frontier the case left.
  - Result: a case the governor holds complete ends asking for evidence for `tests-pass`, which is exactly the defect the story's Outcome names. The commit title ("reports a move whenever its run left the handed revision") does not hold.
  - What reaches it: the model's turn, the longest window in a run, followed by a prose answer. Anyone can compose this through the SDK's re-exports (`loom::harness::governed::LoopExecutor`, `run_until_blocked`, `CanonGovernor`). No caller in the repo composes them.
  - Fix, not applied: at the end of `governed()`, read `current_revision` once and count any revision other than the handed one as a move. Alternatively, the runtime could reload before it ends a run on the handed frontier after `CompletedLocalReasoning` or `NoUsefulAction`; that would also cover B.
  - Origin: undecided. From reading the code, the base returns the same outcome.
- **B. The pipeline Loom has the same window** (`crates/loom-executor/src/lib.rs:477`).
  - What fails: `NothingAdmissible` becomes `NoUsefulAction` without a governor read. The acceptance line "a case completed while the selector selects … ends `Completed`" holds only when the selector actually selects something.
  - What reaches it: a selector supplied by the embedder (`FirstAdmissibleSelector` answers `NothingAdmissible` for every catalogue that holds only gated actions). Nothing in the repo composes a governed Loom with `run_until_blocked`.
  - Severity: note. Origin: undecided; this line is unchanged by the unit's diff.

**5. Attacked, not broken.** All of these come from reading the code; I ran no probes.
- Order of checks: the generated `RevalidateSelection` tests stale-revision before not-in-frontier, so a selection on a moved case is always reported as `CaseMoved`.
- A `ProposedAction` admitted at the handed revision but still reported as moved would need revisions to go backwards. With a consistent governor I could not construct it.
- No loop path produces `NeedsHumanJudgment`, so nothing outside the rule slips through that way. A suspension after a move comes from a governor or wire outage, so suspending the Run is correct.

**6. Paths written outside the worktree:** none.
- Logs are in `<worktree>/.engineering/drafts/scratch/adversary-3/`: `red-alone.log`, `suite.log`, `clippy.log`, `clippy-2.log`, `red-after-rename.log`.
- `TMPDIR` pointed at `scratch/adversary-3/tmp`, and builds went to the tree's own `target/`.
- I took the session lease `adversary3-w3-moved` and released it.

Case file: `~/.local/state/worktree/trees/b10x/loom/loom-20261007-w3-moved/crates/loom-executor/tests/adversary3_w3_moved_case_loop.rs`

```findings
- file: crates/loom-executor/src/harness/governed.rs
  line: 313
  category: acceptance
  severity: warning
  verdict: NEEDS-CHANGE
  origin: undecided
  message: a case that moves while the model takes its last turn and the model answers in prose triggers neither half of the rule, so LoopExecutor answers CompletedLocalReasoning and a case the governor holds complete ends NeedsExternalEvidence(["tests-pass"])
- file: crates/loom-executor/src/lib.rs
  line: 477
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: undecided
  message: the pipeline Loom answers a selector's NothingAdmissible with NoUsefulAction without reading the governor, so a case completed while the selector was asked ends NeedsExternalEvidence(["tests-pass"]) instead of Completed
```
