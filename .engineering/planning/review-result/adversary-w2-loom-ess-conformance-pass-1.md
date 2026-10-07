---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-ess-conformance-pass-1
kind: review-result
status: active
title: Wave 2026-10-07-w2 adversary, loom story:loom-ess-conformance, pass 1
relations:
- reviews: story:loom-ess-conformance
revision: 1
---
unit: story:loom-ess-conformance, branch impl/loom-ess-conformance at 0706e2a (worktree loom-20261007-w2-conformance) plus 1 untracked test file of mine
verdict: CONFIRMED. No red case: the code under test is correct today, but `task conform` stays green under 5 mutants of the new `select_action`, and my 3 new cases kill all 5.
cases: executed 657→660, red 0
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 6 (5 harness output files and 1 lease record, listed in part 6)
needs-coordinator: (a) whether the implementor's closing report says the target rewrites `frontier_actions` (finding 2); (b) whether to keep my test file

**1. Diff**
`git --no-pager diff --stat` prints nothing, because my only change is a new untracked file. `git status --short` shows one line:
`?? crates/loom-executor/tests/adversary_w2_conformance_select_action.rs`
It is a test path. I changed no implementation file.

**2. Case added: `crates/loom-executor/tests/adversary_w2_conformance_select_action.rs`**

| test | asserts | real code | killed mutant |
|---|---|---|---|
| `adversary_w2_a_revision_on_either_side_of_the_catalogues_is_refused_and_nothing_stored` | revisions 6 and 8 against a catalogue at 7 both give exactly `RevisionMismatch{catalogue_id, case_revision, catalogue_revision: 7}`, and nothing is stored | green | M3, M4, M5 |
| `adversary_w2_an_unlisted_action_at_another_revision_is_not_in_catalogue` | an unlisted action at revision 8 gives `NotInCatalogue`, because outcomes are chosen in declaration order (`generated/rust/loom/src/behaviour.rs:330`) | green | M7 |
| `adversary_w2_a_selection_keeps_the_confidence_and_strategy_it_was_made_with` | the stored `SelectionData` equals the input, including `confidence: Some("0.25")` and `FastTyped` | green | M2 |

Run alone, before any suite run: `test result: ok. 3 passed; 0 failed`, EXIT=0.

Red output against the mutants, verbatim from `scratch/adversary-1/mutant-logs/M{2,3,4,5,7}-1.log`:
- M3: `claimed revision 8 on a catalogue at 7: expected RevisionMismatch {… case_revision: 8, catalogue_revision: 7 } }, got Selected {…}`
- M4: `… got RevisionMismatch { … case_revision: 6, catalogue_revision: 6 } }`
- M5: `claimed revision 6: the refusal stored [SelectionSnapshot { state: Selected, … }]`
- M7: `left: RevisionMismatch { … } right: NotInCatalogue { error: ActionNotInCatalogue { action: "repository.merge" } }`
- M2: `left: [… confidence: None, …] right: [… confidence: Some(Decimal("0.25")), …]`

**3. Suite runs (worktree, after the case existed)**
- `cargo test --locked -p b10x-loom-conformance`: conform.rs `2 passed; 0 failed` (`taskfile_check_runs_conform`, `ess_conformance_report`); unit tests and doc-tests ran 0 cases. EXIT=0.
- `cargo test --locked --no-fail-fast -p b10x-loom-executor`: 658 passed, 0 failed over 55 result lines. EXIT=0. The before count of 655 is this run's 658 minus my 3 cases. Log: `scratch/adversary-1/suite-executor.log`.

**4. Findings (they cover 0706e2a)**

| # | file:line | verdict / origin | measured | reached by |
|---|---|---|---|---|
| 1 | `crates/loom-executor/src/selection.rs:172` (also :165, :167, :177, :187) | CONFIRMED / introduced | Five mutants each leave `--test conform` at EXIT=0: M2 drops the confidence; M3 changes `!=` to `<`; M4 reports the claimed revision as the catalogue's; M5 stores the selection on refusal; M7 checks the revision before membership. With M2–M5 applied together, no other executor test fails; the only 3 failures came from my copy lacking `.git` and `.engineering/planning`. | Only `crates/loom-conformance/src/lib.rs:668` calls it. Production uses `selection::select`, so the suite is `select_action`'s only test. |
| 2 | `crates/loom-conformance/src/lib.rs:763` | INFEASIBLE / introduced | All 14 synthesized RevalidateSelection steps send `frontier_actions: []`. The target removes or adds the selection's action depending on whether the outcome was forced, so the executor's membership rule (`revalidation.rs:62`) only ever sees `[]` or `[action]`. Mutant M1 (`!contains` changed to `is_empty()`) leaves `task conform` green; `adversary_run_revalidation.rs` `not_in_frontier_follows_the_frontier_actions` kills it. | Every `task conform` run. |
| 3 | `crates/loom-conformance/tests/conform.rs:187` | CONFIRMED / introduced | Mutant M11 removes `"unsupported"` from the skip check and `--test conform` still exits 0. The acceptance-4 self-check only adds a `skipped` scenario, which the Rust producer cannot emit (ess-conformance `counts.rs:459`). The `unsupported` category LoomTarget actually emits (`lib.rs:178`, `:317`) is never self-checked. The report's own `conformance_status` is never read either. | Nothing today: 0 unsupported scenarios and `SKIPPED.md` is empty. |

More on finding 2. The specification contradicts itself:
- `ess/domains/run.yaml:675` says `not-in-frontier` is "answered by the executor from the input's frontier_actions".
- ESS describes forcing as being for "an outcome the input cannot decide" (`target.rs:274`).
- The synthesized `admitted` scenario sends `[]` and expects `admitted`.

The implementor's second red run (`target/tmp/conform-3386778-…/report.json`) failed exactly those 3 `admitted` scenarios. That is a scenario showing the specification wrong; the story says such a scenario leads to a specification change, and the rules say anything the specification cannot express is reported instead. ESS 0.55 refuses to express the membership check (ESS-SYNTH-003/004), so this cannot be fixed in this repository.

The AGENTS.md row in `docs-proposal.md` ("The executor answers every command of `ess/` as specified | `task conform`") would make this overclaim permanent for `not-in-frontier`.

**5. Attacked and held**
- **Verdict source:** the test reads the report's counts and outcomes in all 5 categories, never an exit code. `CountReport::from_run` refuses a run whose verdict contradicts its results (`counts.rs:213-224`).
- **Stale suite:** each run synthesizes into a fresh `target/tmp/conform-<pid>-<nanos>/`. No suite or report file is committed (`git ls-files`), and `ess/` is read at run time.
- **Target answering from its own state:** views and events come from `TurnRecord`, `RequestRecord` and the executor's outcome payloads. The one exception is finding 2.
- **`select()` unchanged:** only the membership rule moved into `chosen()`. `strategy()` is now called before the membership check, but every implementation is a pure getter.
- **New `TurnRecord` paths:** ProjectCatalogue and ReleaseSession go through `generated()` like the six existing commands. Production stores catalogues only through `offered` (`governed.rs:654`), with unique ids.
- **M9 and M10:** M9 (RequestArguments stores no request) and M10 (RecordTurn stores no turn) both leave `task conform` green; 12 and 8 executor test targets kill them. The specification has no Turn or ArgumentRequest view, so the suite cannot see them; that predates this unit.
- **Acceptance 1–3:** hold, with 11 compiled commands, 46 scenarios and 0 failed.

**6. Paths written outside the worktree**
- 4 harness task output files and 1 harness tool-result file under the session's harness directories (harness)
- The worktree lease record for session `adversary-1-w2-conformance`, acquired and released through `worktree hook`.

Inside the tree and git-ignored: `<worktree>/.engineering/drafts/scratch/adversary-1/` is 2.9G, almost all `mutant-tree/target`. I left it in place, since build directories are not mine to delete.

**7. Findings block**
```findings
- file: crates/loom-executor/src/selection.rs
  line: 172
  category: mutant
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: five select_action mutants (confidence dropped, != to <, wrong catalogue_revision, stored on refusal, revision checked before membership) leave task conform green, and nothing but the conformance target calls select_action
- file: crates/loom-conformance/src/lib.rs
  line: 763
  category: judgement
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: the target decides frontier membership by the forced flag over an always-empty input list, so task conform never tests the executor's not-in-frontier rule (mutant M1 survives) and the spec's external text contradicts ESS external semantics
- file: crates/loom-conformance/tests/conform.rs
  line: 187
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the acceptance-4 self-check exercises only the skipped category the Rust producer cannot emit, so dropping the unsupported branch leaves task conform green, and the report's conformance_status is never read
```
