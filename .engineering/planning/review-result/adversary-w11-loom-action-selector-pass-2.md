---
format: aep.planning-md/3
id: review-result:adversary-w11-loom-action-selector-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w11 adversary, loom story:action-selector, pass 2
relations:
- reviews: story:action-selector
revision: 1
---
unit: loom/action-selector, worktree loom-w11-action-selector (ecf204c + uncommitted phase 2 + pass 1's and pass 2's test files)
verdict: NEEDS-CHANGE
cases: executed 105→110, red 2
origin: introduced 1 / pre-existing 3 / undecided 0
wrote-outside-worktree: 12 paths (part 6)
needs-coordinator: yes. The introduced fix belongs in `website/docs/`, which is outside the story's typed scope.

The code holds on everything you listed. The one defect this change introduced is on two site pages, which now describe the old selector input.

**1. `git --no-pager diff --stat`**
This is the implementor's diff and it is unchanged: 7 files, 274 insertions, 90 deletions. I added one untracked file, `crates/loom/tests/adversary2_action_selector.rs`. I touched no non-test path.

**2. Cases added** in `~/.local/state/worktree/trees/b10x/loom/loom-w11-action-selector/crates/loom/tests/adversary2_action_selector.rs`

| case | what it asserts | now | can it fail? |
|---|---|---|---|
| `the_generator_sees_the_entry_the_catalogue_showed_and_commission_decides` | Over 985 frontiers listing merge 0–3 times (6 kinds × positions), each catalogue entry is proposed. The generator gets an entry with the catalogue's status and the capability Commission will ask for. Admissible proposals derive `Continue`. | green | yes: red under mutant A, where every other existing case stays green |
| `the_first_admissible_selector_follows_the_catalogue_not_the_raw_frontier_status` | `FirstAdmissibleSelector` through `run` proposes the first Admissible catalogue entry, or returns `NoUsefulAction`. Reversing merge's duplicate entries does not change the result. | green | yes: red under mutant B |
| `a_selection_always_carries_its_catalogues_revision_and_id` | `Selection.case_revision` and `catalogue_id` equal the catalogue's, for revisions from i64::MIN to MAX, on projected and hand-built catalogues | green | yes: red under mutant C |
| `site_pages_do_not_say_a_selector_sees_the_frontier` | No site page says the selector sees the frontier | **red** | — |
| `the_status_page_does_not_say_approval_gated_actions_suspend` | `run` proposes an approval-gated action, and the status page does not claim it suspends | **red** | — |

Red output from running the file alone, before the suite (`test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s`):
```
pages still describe the bootstrap selector seam:
…/website/docs/status.mdx:19: {label: 'ActionSelector and ArgumentGenerator traits', status: 'shipped', detail: 'In a bootstrap shape: a selector sees the frontier and a prompt; arguments are a JSON string.'},
…/website/docs/concepts/action-selection.md:25: Both traits exist today in a bootstrap shape: a selector sees the frontier and a prompt, and
Loom answered ProposedAction(ExecutorOutcomeProposedAction { action: "repository.merge", arguments: ProposedActionArguments(Object([])) }), and the status page says:
…/website/docs/status.mdx:17: {label: 'Approval-gated actions suspend the run', status: 'shipped', detail: 'Loom returns Suspended naming the capability required, and proposes nothing.'},
```

Mutants, run on scratch copies:

| mutant | result |
|---|---|
| A: `lib.rs:125`, the `NeedsAuthority` arm of `deciding_entry` becomes `true` | Only my case fails: `60 disagreements; … catalogue shows CatalogueEntry { action: "repository.merge", status: ApprovalRequired } needing "cap.a", generator handed FrontierAction { … status: Admissible, capability: None … }`. On the base export (3c05788) the whole `b10x-loom` suite stays green: exit 0, 79 passed. |
| B: projection maps `NeedsAuthority` to `Admissible` | My two property cases fail, and so do the existing projection cases |
| C: `selection.rs:117` `case_revision.max(0)` | Only my case fails. The mutant is contrived, so I do not raise it as a finding. |
| D: delete the Refused recheck at `lib.rs:188` | The whole suite stays green. The guard is dead: a choice in the catalogue is never refused on the same frontier. It is defensive, not a defect. |

**3. Gate**, run in the unit's build dir after the cases existed. "Summary line" below is the line pasted verbatim from that step's output.

| step | exit | summary line |
|---|---|---|
| `cargo fmt --check` | 0 | none (no output) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | ``Finished `dev` profile [unoptimized] target(s) in 0.10s`` |
| `cargo test --workspace --locked` | 101 | `test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s` (this stops at the first failing target) |
| the same with `--no-fail-fast`, for the count | 101 | 27 result lines, 108 passed, 2 failed, 2 ignored |
| `ess specify validate` + `conform synthesize` | 0 | `loom v1 — 2 file(s), valid` / `18 scenario(s) (0 authored), 0 refusal(s), written to ~/.cache/b10x-target/loom-w11-action-selector/suite.json` |
| `cargo run -p loom-docs -- generate --check` | 0 | `website/data/ess: 1 generated files current` / `website/docs/reference/ess: 2 generated files current` |

`--list` in this tree shows all 5 new test names. My first fmt run failed on my own file; I reformatted only that file with `rustfmt`.

**4. Findings** (cover the worktree as it stands)

| # | file:line | finding | verdict / origin | what reaches it |
|---|---|---|---|---|
| 1 | `website/docs/status.mdx:19`, `website/docs/concepts/action-selection.md:25` | Both pages say "a selector sees the frontier and a prompt". That was true at base and this change made it false. Suggested fix: "the selection context and the projected catalogue's entries". | NEEDS-CHANGE / introduced | Anyone writing a selector from the published site. `generate --check` cannot catch it because both pages are hand-written. |
| 2 | `website/docs/status.mdx:17` | The page lists "Approval-gated actions suspend the run" as shipped. At base and now, Loom proposes the action (ADR 0082). | CONFIRMED / pre-existing | Site readers. The page is unchanged since base. |
| 3 | `crates/loom/src/lib.rs:125` | The capability filter in `deciding_entry` is pinned by no existing case (mutant A survives at base and now). My first case now kills it. | CONFIRMED / pre-existing | A frontier listing one action as both Admissible and ApprovalRequired |
| 4 | `website/docs/status.mdx:23` | The page lists catalogue projection as "planned". It shipped at base (`projection.rs`) and `run` uses it now. I wrote no test for this one. | CONFIRMED / pre-existing | Site readers |

**5. Attacked, not broken**
- **A catalogue choice that is Blocked or ApprovalRequired:** Blocked cannot be expressed in the catalogue, which has 2 entry statuses and drops actions Commission refuses. ApprovalRequired entries are proposed with the right capability entry, on all 985 frontiers.
- **Status changing between projection and admit:** this cannot happen inside `run`. It projects and admits from one borrowed, immutable frontier, and `admit` is pure. `Loom::select` given an out-of-date catalogue accepts by design; revalidation is a separate story.
- **`deciding_entry` vs the catalogue when an action is listed twice:** they agree on every frontier, whatever the order.
- **Outcomes vs Commission:** the outcomes match the `AgentExecutor` port doc and `outcome::derive`. Mapping not-in-catalogue to `NoUsefulAction` is unchanged from base.
- **ESS:** membership ignores status, in the spec and in the code. `revision-mismatch` cannot occur, because the selection copies the catalogue's revision at every value tested. `Selection.case_revision` never differs from the catalogue's.
- **Other pages:** `README.md:16` and `one-run.mdx:44` say "frontier" but are still true, since the catalogue is a subset of the frontier.

**6. Written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w11/loom-action-selector/scratch/adversary-p2-new-cases.log`
- `~/.cache/ga-wave-2026-10-04-w11/loom-action-selector/scratch/adversary-p2/{mA-needs-authority-any-entry,mA-base,mB-projection-needs-authority-admissible,mC-revision-clamped,mD-no-refused-recheck,docs-check,gate-fmt,gate-clippy,gate-test,count-test,gate-ess}.log`
- `suite.json` went to the unit's build dir, at the path the gate names.
- The scratch copy, the base export and their `target/` are deleted.
- I took no worktree session lease; the brief did not ask for one.

```findings
- file: website/docs/status.mdx
  line: 19
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "status.mdx:19 and concepts/action-selection.md:25 still say a selector sees the frontier and a prompt; since this change it is handed the selection context and the projected catalogue's entries (site_pages_do_not_say_a_selector_sees_the_frontier is red)"
- file: website/docs/status.mdx
  line: 17
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: "the status page lists approval-gated actions as suspending the run while Loom proposes them for Commission to authorize (the_status_page_does_not_say_approval_gated_actions_suspend is red)"
- file: crates/loom/src/lib.rs
  line: 125
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: "replacing deciding_entry's NeedsAuthority filter with true survives the whole suite at base and now; the_generator_sees_the_entry_the_catalogue_showed_and_commission_decides now kills it"
- file: website/docs/status.mdx
  line: 23
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: pre-existing
  message: "catalogue projection is listed as planned although projection.rs shipped at base and Loom::run selects over it"
```
