---
format: aep.planning-md/3
id: review-result:adversary-w9-loom-run-pipeline-skeleton-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w9 adversary, loom story:run-pipeline-skeleton, pass 1
relations:
- reviews: story:run-pipeline-skeleton
revision: 1
---
```
unit: loom/run-pipeline-skeleton, working tree of b358cee plus uncommitted phase 2 (~/.local/state/worktree/trees/b10x/loom/loom-w9-run-pipeline-skeleton)
verdict: NEEDS-CHANGE
cases: executed 84→86, red 2
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (scratch/adv1/, ~8M)
needs-coordinator: the fix for finding 2 must decide how to handle the ESS-SYNTH-003 limit, because the story says "stop and report", and the spec used `external:` instead
```

**Two red cases. The spec admits exactly the selections that `story:selection-revalidation` acceptance items 1 and 2 require it to refuse.** The synthesized conformance suite mandates that admission too.

### 1. `git --no-pager diff --stat`
The tracked diff is the implementor's phase 2 only: 10 files, +923/−28, unchanged by me. My only change is one new untracked file, `crates/loom/tests/adversary_run_revalidation.rs`. I touched no non-test path.

### 2. Cases added
Both live in `crates/loom/tests/adversary_run_revalidation.rs`. They drive the generated `b10x_loom::model::behaviour::Generated`: `ProjectCatalogue` and `RevalidateSelection` are generated. `SelectAction` is an obligation, so a stand-in implements it exactly as `run.yaml` declares it.

| Test | Asserts | Now |
|---|---|---|
| `selection_on_an_older_catalogue_is_never_admitted` (:210) | catalogue projected at revision 7, selection made with `case_revision` 8, revalidated at 8 → `StaleRevision{catalogue_revision:7, case_revision:8}` | red |
| `not_in_frontier_follows_the_frontier_actions` (:240) | for either fixed `external` answer, a frontier that lists the action → `Admitted`, one without it → `NotInFrontier` | red |

Red output, from running this file alone first (`scratch/adv1/red-alone.log`, EXIT=101):
```
external answer false: frontier without repository.merge gave Admitted { ... 000000000021 ... }
external answer true: frontier listing repository.merge gave NotInFrontier { ... 000000000020, action: "repository.merge" }
---
  left: Admitted { selection_admitted: SelectionAdmitted { selection_id: ...000000000010 } }
 right: StaleRevision { selection_stale: SelectionStale { ...010, catalogue_revision: 7, case_revision: 8 } }
```
**Mutant proof:** I made a copy in `scratch/adv1/mutant-tree` with two changes: the `not-in-frontier` branch reads `!input.frontier_actions.contains(&held.data.action)`, and the stand-in refuses `input.case_revision != catalogue.case_revision`. Both cases pass there: `2 passed`, EXIT=0 (`scratch/adv1/mutant-fixed.log`). `--list` in the worktree shows both tests.

### 3. Suite (run after the cases existed)
`cargo test --workspace --locked --no-fail-fast` gave EXIT=101 (`scratch/adv1/suite.log`).
- 84 passed, as handed over.
- `adversary_run_revalidation`: `0 passed; 2 failed`.

Other checks, all exit 0:
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `loom-xtask drift`: no drift.
- `loom-xtask no-hand-model`: 68 reserved names.
- `loom-docs generate --check`: 3 generated files current.
- `ess verify conform synthesize`: 14 scenarios, 0 refusals.

### 4. Findings

| # | file:line | Verdict / origin | What was measured | What reaches it |
|---|---|---|---|---|
| 1 | ess/domains/run.yaml:243 | NEEDS-CHANGE / introduced | `SelectAction.case_revision` is caller input that nothing checks against the catalogue's revision. `Selection.case_revision` (:274) copies it, and the stale guard (:316) compares only that copy. Case 1 is red. | The synthesized suite's `admitted` scenario projects a catalogue at 194041, selects with `case_revision` 1 (expects `selected`), then revalidates at 1 with `frontier_actions: []` (expects `admitted`). The story listed no such input. `selection-revalidation` § Domain relations takes the revision from the catalogue relation. ESS 0.52.0 can express the fix: `when_related: {via: input.catalogue_id, predicate: case_revision != input.case_revision}` validates and synthesizes 15 scenarios with 0 refusals (scratch probeB). |
| 2 | ess/domains/run.yaml:326 | NEEDS-CHANGE / introduced | `not-in-frontier` is `external:`. Generated `behaviour.rs:167` never reads `frontier_actions`, and `Context::external` receives only the command and outcome names. Case 2 is red for both answers. | The story says to stop if ESS cannot express a refusal. The membership predicate does validate, but synthesis refuses it: ESS-SYNTH-003/004, 3 refusals (scratch probeA3). So the stop rule applied. The `admitted` scenario's empty frontier makes the input meaningless in conformance. |
| 3 | crates/loom/src/arguments.rs:1 | CONFIRMED / introduced, note | All 8 module docs describe built behaviour in the present tense ("Argument generation for the selected action only"). None says the module is empty. | rustdoc readers of `b10x_loom::arguments` and the other modules |
| 4 | ess/domains/run.yaml:283 | INFEASIBLE / introduced, note | `RequestArguments` has no `selection-unknown` or state guard, unlike `SelectAction`'s `catalogue-unknown`. The generated behaviour creates a request for any `selection_id`. | Nothing found: Loom passes its own fresh selection id. |
| 5 | ess/domains/run.yaml:305 | INFEASIBLE / introduced, note | Neither `SelectAction` nor `RevalidateSelection` binds to a turn, session or run. `ProjectCatalogue` does not check that the turn exists, nor that the turn has only one catalogue (the `Turn.catalogue` relation, :97). So a selection can be made against another session's catalogue. | Nothing found: no caller builds that state. |
| 6 | ess/domains/run.yaml:397 | CONFIRMED / introduced, note | The `Selections` view omits `confidence`. `ess` warns twice that a refusal changing `confidence` goes unchecked. | The conformance suite |

### 5. Attacked and could not break
- **Lifecycle:** `Selected→Admitted|Refused` are both reachable. A second revalidation gets `wrong-state`, and suite scenarios cover both terminal states.
- **`case_revision` in commands, events and the view:** fields and types match across `SelectAction`, `Selection`, `SelectionStale` and `Selections`.
- **no-hand-model:** reserves every new generated name (`CatalogueEntry`, `Context`, `Generated`, `SelectionStorage`, …), probed with `--src`.
- **drift:** covers `behaviour.rs` and `obligation.rs`.
- **Website:** the domain graph and reference match the compiled spec, and `docs-check` is current.

### 6. Paths written outside the worktree
- `~/.cache/ga-wave-2026-10-04-w9/loom-run-pipeline-skeleton/scratch/adv1/`, which holds:
  - spec copies: `ess-base`, `probeA`, `probeA3`, `probeB`, plus their suite JSON files
  - `mutant-tree/`, `mutant-target/`, `nhm-src/`
  - logs: `red-alone.log`, `mutant-fixed.log`, `suite.log`
- I acquired and released a worktree session lease named `adversary-w9-run-pipeline-skeleton-p1`.

### 7. Findings block
```findings
- file: ess/domains/run.yaml
  line: 243
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "SelectAction takes case_revision as unchecked input and the stale guard compares only that copy, so a selection made on a revision-7 catalogue is admitted at revision 8, and the synthesized admitted scenario requires exactly that"
- file: ess/domains/run.yaml
  line: 326
  category: acceptance
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "not-in-frontier is external, so the generated RevalidateSelection never reads frontier_actions and admits an action absent from the current frontier, even though the story's stop rule applied (the predicate validates but synthesis refuses it with ESS-SYNTH-003)"
- file: crates/loom/src/arguments.rs
  line: 1
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the eight empty module files describe their behaviour in the present tense and none says it is not built yet"
- file: ess/domains/run.yaml
  line: 283
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "RequestArguments declares no selection-unknown or state refusal, so the generated behaviour records a request for any selection id, unlike SelectAction's catalogue-unknown"
- file: ess/domains/run.yaml
  line: 305
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "no command binds a selection or revalidation to a turn, session or run, and ProjectCatalogue enforces neither that the turn exists nor that it has only one catalogue"
- file: ess/domains/run.yaml
  line: 397
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the Selections view omits confidence, so conformance cannot observe a refusal that changes it, as ess warns twice"
```
