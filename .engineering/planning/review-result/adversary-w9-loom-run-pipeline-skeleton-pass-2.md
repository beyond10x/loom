---
format: aep.planning-md/3
id: review-result:adversary-w9-loom-run-pipeline-skeleton-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w9 adversary, loom story:run-pipeline-skeleton, pass 2
relations:
- reviews: story:run-pipeline-skeleton
revision: 1
---
unit: loom/run-pipeline-skeleton, working tree on b358cee plus uncommitted phase 2 and the pass-1 fixes, in ~/.local/state/worktree/trees/b10x/loom/loom-w9-run-pipeline-skeleton
verdict: NEEDS-CHANGE
cases: executed 85→87, red 2
origin: introduced 4 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory (~/.cache/ga-wave-2026-10-04-w9/loom-run-pipeline-skeleton/scratch/adv2/)
needs-coordinator: yes. F1 needs a spec change in this unit, because later stories may not edit `ess/`. F2 needs an ESS change.

**1. Diff stat.** `git --no-pager diff --stat` is the same 11 files as before my pass. My only addition is the untracked file `crates/loom/tests/adversary2_run_identity.rs`. I changed no implementation file and nothing in `ess/` or `generated/`.

**2. Cases added** (`crates/loom/tests/adversary2_run_identity.rs`). Both are red now. They run against the generated `ProjectCatalogue`, `RevalidateSelection` and `Selections`. `SelectAction` is implemented in the test exactly as `PLAN.md`'s contract states it.

| case | asserts | now |
|---|---|---|
| `projecting_a_stored_catalogue_id_again_keeps_its_selections_true` | a second `ProjectCatalogue` for catalogue 1 (another turn, revision 8, no merge entry) leaves the existing selection's revision equal to its catalogue's, and its action still listed | red |
| `a_refused_selection_is_never_selected_or_admitted_again` | once selection 10 is refused as stale, a second `SelectAction` with id 10 is not accepted, and selection 10 is never admitted | red |

Red output from the first run of this file alone; line numbers are from before rustfmt:
```
SelectAction for the refused selection id 10 answered Selected { action_selected: ActionSelected { ... catalogue_id: CatalogueId(Uuid("...000000000004")), action: "repository.merge" } }
the second revalidation of selection 10 answered Admitted { selection_admitted: SelectionAdmitted { ... } }
selection 10 now rests in Some(Admitted), not Refused
---
the generated ProjectCatalogue replaced catalogue 1 under a selection made on it:
selection case_revision 7 but its catalogue now holds 8 (turn TurnId(Uuid("...000000000003")))
selection names repository.merge but its catalogue now lists ["repository.read"]
test result: FAILED. 0 passed; 2 failed   EXIT=101
```
`--list` shows both names in this tree. After rustfmt, the assertions are at lines 269 and 321.

**3. Suite**, run after the cases existed: `cargo test --workspace --locked --no-fail-fast` gave 85 passed, 2 failed (both mine), 1 ignored, exit 101. Also run:
- `cargo fmt --check`: 0
- `cargo clippy -p b10x-loom --all-targets -- -D warnings`: 0
- `loom-xtask drift`: no drift
- `loom-xtask no-hand-model`: 69 names checked, no hand-written type
- `loom-docs generate --check`: generated pages current

**4. Findings**

| # | file:line | verdict / origin | finding | what reaches it |
|---|---|---|---|---|
| F1 | ess/domains/run.yaml:215 | NEEDS-CHANGE / introduced | `ProjectCatalogue` has no `existing_instance:` branch, so the generated behaviour stores a new catalogue over an existing one without checking (`behaviour.rs` `project_catalogue`). This breaks the pass-1 claim that a stored selection's revision always equals its catalogue's, and the selection can then name an action its catalogue no longer lists. **Fix:** `- {name: catalogue-exists, existing_instance: true, error: loom.run.CatalogueExists}` plus a `Catalogues` view showing `catalogue_id` and state. In a scratch copy this validates, synthesizes with 0 refusals, and the regenerated code refuses the second call. | Nothing calls it today. `story:frontier-projection` will, and it may not edit `ess/`, so only this unit can fix it. |
| F2 | ess/domains/run.yaml:245 | INFEASIBLE / introduced | `SelectAction` has no `existing_instance:` branch, so a second call with the same selection id replaces a `Refused` selection with a fresh `Selected` one. Revalidation then admits it, against the spec's "revalidated once" and its terminal states. `RequestArguments` replaces an existing argument request the same way. The same `existing_instance:` fix on `SelectAction` makes ESS 0.52.0 refuse the suite (`ESS-SYNTH-004 … route runs through SelectAction/selected, which no input reaches`), so it can't pass the gate here. | No caller today. A retried or replayed `SelectAction` (interruption-recovery) would reach it. Needs an ESS change first. |
| F3 | ess/domains/run.yaml:306 | CONFIRMED / introduced | `RequestArguments` is not state-guarded: it accepts a `Refused` or `Admitted` selection. The guard `when_related: {via: input.selection_id, predicate: state != Selected}` synthesizes with 0 refusals in the probe. | Not reached in the normal order (arguments, then revalidation). Note only. |
| F4 | ess/domains/run.yaml:187 | CONFIRMED / introduced | `CatalogueRevisionMismatch` names only the revision the caller claimed, not the catalogue's actual one. `SelectionStale` (run.yaml:415) names both. | Anyone reading the refusal. Note only. |

**5. Checked and found sound**
- The revision guard compares the catalogue's stored `case_revision` with the input's, and `!=` works in both directions.
- `SelectAction` is the only command that creates a selection; F1 is how the invariant breaks after creation.
- drift and no-hand-model derive their names from the compiled spec, so they cover the new error types and owed traits.
- The website reference matches the spec (`revision-mismatch` and `selection-unknown` at loom-run.md:287 and :330) and docs-check passes.
- The ignored test's reason is accurate. In the probe, a membership guard over `input.frontier_actions` validates but synthesis refuses `admitted` with ESS-SYNTH-003, plus a follow-on ESS-SYNTH-004.
- Nothing besides the ignored test reads `frontier_actions`.
- Two catalogues for one turn is already deferred to `story:selection-revalidation`, so I did not raise it again.

**6. Written outside the worktree**, all under `~/.cache/ga-wave-2026-10-04-w9/loom-run-pipeline-skeleton/scratch/adv2/`: `suite.json`, `probe-suite.json`, `fix-suite.json`, `ess-probe/`, `ess-fix/`, `fix-gen/`, `red-identity.log`, `suite.log`, `suite-nff.log`. Builds went only to the assigned target dir. My worktree lease is released.

```findings
- file: ess/domains/run.yaml
  line: 215
  category: property
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "ProjectCatalogue declares no existing_instance refusal, so the generated behaviour replaces a stored catalogue and a selection made on it no longer matches its catalogue's case_revision or entries; the fix (existing_instance plus a Catalogues identity view) synthesizes with 0 refusals and only this unit may change ess/"
- file: ess/domains/run.yaml
  line: 245
  category: property
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: "SelectAction declares no existing_instance refusal, so re-sending a refused selection's id resets it to Selected and revalidation admits it, breaking the terminal Refused state; ESS 0.52.0 refuses that fix with ESS-SYNTH-004"
- file: ess/domains/run.yaml
  line: 306
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "RequestArguments accepts a Refused or Admitted selection; a when_related state != Selected guard is expressible at ess/20 and synthesizes with 0 refusals"
- file: ess/domains/run.yaml
  line: 187
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "CatalogueRevisionMismatch reports only the caller's claimed revision, not the catalogue's actual one, unlike SelectionStale which reports both"
```
