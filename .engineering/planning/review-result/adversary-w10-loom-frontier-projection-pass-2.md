---
format: aep.planning-md/3
id: review-result:adversary-w10-loom-frontier-projection-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w10 adversary, loom story:frontier-projection, pass 2
relations:
- reviews: story:frontier-projection
revision: 1
---
unit: loom/frontier-projection, covering working tree e4194ec plus the uncommitted phase 2, the pass-1 fixes and my 3 test files
verdict: CONFIRMED (note only). No case is red against the tree. The rule that a repeated action keeps its first position was untested, and a mutant that breaks it survived every existing test.
cases: executed 92→101, red 0 (2 red against the mutant copy)
origin: introduced 1 / pre-existing 0 / undecided 1
wrote-outside-worktree: 7 paths, under `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary2/` (part 6)
needs-coordinator: none

**1. `git --no-pager diff --stat`**
```
 crates/loom/src/projection.rs | 57 ++++++++++++++++++++++++++++++++++++++++++-
 ess/domains/run.yaml          | 10 ++++----
```
Both lines are the implementor's phase 2. My files are untracked, all test files: `crates/loom/tests/adversary2_projection_{position,empty,executor_agreement}.rs`. I touched no implementation file.

**2. Cases added** (all ran alone first, all green on the tree)

| file | asserts |
|---|---|
| `adversary2_projection_position.rs` (4) | Repeats placed between other actions land at their first position: `[merge Adm, inspect, merge AR(cap)]` → `[merge AR, inspect]`. Blocked first then Admissible later appears nowhere, and so does Admissible first then Blocked later. |
| `adversary2_projection_empty.rs` (4) | An empty frontier and an all-refused frontier both give empty entries, still carrying the frontier id, revision and turn. The empty catalogue is stored through the generated `ProjectCatalogue` as `Projected`, and a second projection is refused with `CatalogueExists`. The executor returns `NoUsefulAction` for every action named, including an unlisted one. |
| `adversary2_projection_executor_agreement.rs` (1) | Over 156 exhaustive frontiers: an action is listed once exactly when `Loom::run` with a selector naming it proposes it. The check uses the executor's outcome, not `admit`. |

Red run against the last-position mutant (scratch copy, `.rev()` … `entries.reverse()`):
```
panicked at crates/loom/tests/adversary2_projection_position.rs:65:5:
  left: [("repository.inspect", Admissible), ("repository.merge", ApprovalRequired)]
 right: [("repository.merge", ApprovalRequired), ("repository.inspect", Admissible)]
```
In that same run every pre-existing test passed, including `frontier_projection` (1) and `adversary_projection_duplicates` (5). Only my 2 position cases failed (EXIT=101, `mutant-last-position.log`).

**3. Gate, run after the cases existed** (worktree, assigned build dir)
- `cargo fmt --check`: EXIT=0, after I ran rustfmt on my own two files.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: EXIT=0
- `cargo test --workspace --locked`: EXIT=0, 101 passed, 0 failed, 2 ignored. `--list` shows all 9 new names in this tree.

**4. Findings**

| # | file:line | verdict | origin | what was measured / what reaches it |
|---|---|---|---|---|
| 1 | `crates/loom/src/projection.rs:4` | CONFIRMED (note) | introduced | **Measured:** the doc promises "at its first position". The last-position mutant passed all 92 earlier tests and was caught only by my position cases. **Reaches it:** any frontier that repeats an action with another action in between, which Commission's contract allows. The fix is to keep `adversary2_projection_position.rs`. |
| 2 | `crates/loom/src/projection.rs:35` | INFEASIBLE (note) | undecided | **Measured:** `admit` scans the whole frontier for each distinct action, so cost grows quadratically. Debug probe: 1k actions 9 ms, 10k 785 ms, 20k 3.13 s; 20k entries of 1 action 1.7 ms. **Reaches it:** nothing found; the example frontiers list 4 actions. I did not run the base, so origin is undecided. The executor's `admits_nothing` (`lib.rs:119`) has the same quadratic shape (not measured). |

**5. Attacked and could not break**
- **Blocked first, Admissible later:** refused by `admit`, so it is in neither position, and the order of the other actions is kept.
- **Empty or all-refused frontier:** gives a valid empty catalogue that can be stored. `ProjectCatalogue` declares no refusal for an empty list.
- **Projection against the executor:** they agree action by action over all 156 frontiers. In theory, not run: the test-only `FirstAdmissibleSelector` (`lib.rs:72`) picks from the raw frontier statuses, so it may return `NoUsefulAction` where the catalogue shows another admissible action. That is a selector, not the model.
- **Commission pin:** `admission.rs` is unchanged between 174bf07 and remote `main` c83b7c5 (18 commits ahead; GitHub compare). The only `src` change in between is `runtime.rs`.
- **`run.yaml` comments:** all three edited comments match the behaviour. `RevalidateSelection` checks membership only, but at the same revision that implies admission.

**6. Paths written outside the worktree**
- `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary2/tree/` (2.2M): source copy. `projection.rs` was restored from the worktree, byte-identical by `cmp`, and touched. It also holds the probe `crates/loom/tests/zz_probe_projection_timing.rs`.
- `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary2/target/` (65M): the mutant copy's build dir.
- `~/.cache/ga-wave-2026-10-04-w10/loom-frontier-projection/scratch/adversary2/{mutant-last-position,probe-timing-debug,fmt,clippy,suite}.log`
- I took no worktree session lease; the brief named none.

```findings
- file: crates/loom/src/projection.rs
  line: 4
  category: mutant
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The promise that a repeated action keeps its first position was untested: a last-position mutant passed all 92 earlier tests and is caught only by adversary2_projection_position.rs."
- file: crates/loom/src/projection.rs
  line: 35
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: undecided
  message: "Projection calls admit, a full frontier scan, once per distinct action, so cost is quadratic (debug: 10k actions 785 ms, 20k 3.13 s); no frontier anywhere near that size was found."
```
