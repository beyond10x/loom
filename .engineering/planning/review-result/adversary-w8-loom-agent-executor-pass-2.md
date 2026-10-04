---
format: aep.planning-md/3
id: review-result:adversary-w8-loom-agent-executor-pass-2
kind: review-result
status: active
title: Wave 2026-10-04-w8 adversary, loom story:agent-executor, pass 2
relations:
- reviews: story:agent-executor
revision: 1
---
unit: loom/agent-executor, working tree on fad4bb1 plus uncommitted phase 2, pass-1 fixes and my two test files
verdict: NEEDS-CHANGE
cases: executed 71→79, red 2
origin: introduced 6 / pre-existing 0 / undecided 0
wrote-outside-worktree: 4 paths (part 6)
needs-coordinator: story text still says Loom suspends at merge approval (F6); whether Loom should check the frontier's case against the commission's

**Pass 2 verdict:** 2 new red cases, both `introduced`, plus 4 findings with no red case.

1. **Diff, test files only.** `git --no-pager diff --stat` shows only the implementor's 8 tracked files, unchanged by me. My two files are untracked, so `--stat` does not list them:
   - `crates/loom/tests/adversary2_executor_seams.rs` (new)
   - `crates/loom-xtask/tests/adversary2_checks.rs` (new)

   I changed nothing else in the worktree.

2. **Cases added.** Each was run alone before the suite.

| Test | Asserts | Now | Mutant that turns it red |
|---|---|---|---|
| `generator_is_given_the_same_entry_whatever_the_order` | the proposal does not depend on frontier entry order when `admit` gives `NeedsAuthority` either way | **red** | — |
| `no_hand_model_refuses_a_model_type_defined_through_a_macro_parameter` | `model!(Selection)` from a `macro_rules!` with `pub struct $name` is refused | **red** | — |
| `refused_selections_are_no_useful_action` | the pass-1 refused frontiers plus one admissible sibling return literally `NoUsefulAction` | green | M1 (`lib.rs:153` Refused → fall through) |
| `a_selector_outage_carries_its_message_text` | the message text is inside `ExternalAvailability` | green | M2 (outage drops the message) |
| `an_argument_generator_failure_is_an_outage_with_its_message` | a generator `Err` becomes an outage carrying its text | green | M2, M3 (`lib.rs:169` → `NoUsefulAction`) |
| `a_frontier_that_admits_nothing_is_no_useful_action_even_when_the_selector_is_down` | `lib.rs:131` doc: a frontier that admits nothing is `NoUsefulAction` | green | M4 (drop `lib.rs:140-142`) |
| `no_hand_model_refuses_cfg_gated_and_nested_module_types` | `#[cfg(any())]` and nested-module-file types are refused | green | — |
| `bootstrap_then_generate_twice_leaves_no_drift_and_no_debris` | the Taskfile bootstrap line, then `generate` twice, gives no drift, no `.loom.*` leftovers, and a tree equal to the committed one | green | — |

Red output, verbatim:
```
assertion `left == right` failed: the proposal for repository.merge depends on the order of its frontier entries
  left: ProposedAction(... arguments: ProposedActionArguments(Object([("status", Text("Admissible")), ("capability", Null)])) })
 right: ProposedAction(... arguments: ProposedActionArguments(Object([("status", Text("ApprovalRequired")), ("capability", Text("repository.write"))])) })
```
```
no-hand-model passed a `Selection` defined through `model!(Selection)` at .../crates/loom/src/lib.rs:289: (exit Some(0)):
```

**The pass-1 cases can no longer fail.** M1 makes Loom propose refused actions. Under M1 the existing suite catches it only in `blocked_selection_is_not_proposed` and `merge_seeking_model_never_gets_merge_proposed`.
- These three stay green under M1, and stay green whatever Loom proposes:
  - `blocked_entry_beside_approval_entry_does_not_ask_for_authority`
  - `conflicting_capabilities_do_not_depend_on_order`
  - `approval_without_capability_does_not_ask_for_nothing`

  They assert only that Loom does not return `Suspended(Authority)`, and Loom no longer builds that value. Their frontiers list only the merge action, so `admits_nothing` (`lib.rs:140`) returns before the selector is called.
- `loom_agrees_with_commission_admission` also stays green under M1. Its `Refused` branch is decided by `admits_nothing`, not by the guard at `lib.rs:153`, and its `authority_capability(..).is_none()` clause can never be false.
- `merge_seeking_model_never_gets_merge_proposed` still catches M1, but its stop on `Suspended` can no longer happen, so it always runs all 4 iterations.

My first version of `refused_selections` had the same blind spot. I added an admissible sibling to each frontier after the first M1 run, so the mutant red comes from the second run. Logs: `adv2-mutants.log` and `adv2-mutants-m1.log`.

3. **Suite**, run after my cases existed (`scratch/adv2-suite.log`):
   - `cargo fmt --all --check`: exit 1. The only diff is in `generated/rust/loom/src/lib.rs`, the ess output. `cargo fmt --check`, the gate's step, exits 0.
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit 0.
   - `cargo test --workspace --locked --no-fail-fast`: exit 101. 77 passed, 2 failed, which are the two red cases above.
   - `--list` shows all 8 new tests plus `executor_proposes_merge_and_commission_asks_authority` in this tree. The suite log shows `Compiling b10x-loom (…/loom-w8-agent-executor/crates/loom)`.

4. **Findings:**
   - **Generator gets the first listed entry** (`crates/loom/src/lib.rs:155-159`). On a frontier listing `[Admissible, ApprovalRequired]` for one action, the generator sees a different entry depending on order. Commission's `admit` gives `NeedsAuthority` either way. What reaches it: duplicate entries are allowed by Commission's frontier contract, and `ArgumentGenerator` is public. No generator in the tree reads the entry, so I marked it INFEASIBLE. Fix: pass the entry that decided admission, or the `Admission` itself.
   - **`no-hand-model` misses a type defined through a macro parameter** (`crates/loom-xtask/src/main.rs:610-636`). The macro scan only looks for a type keyword directly followed by a reserved name. The module doc at `main.rs:6-9` says the check fails on any defined type. Defining a model type through a macro takes intent, so no-one hits this by accident. CONFIRMED, note.
   - **Story text still describes the old behaviour** (`.engineering/planning/story/agent-executor.md:78,119,174-176`). Those lines still say `Suspended` on ApprovalRequired and name `executor_suspends_at_merge_approval`, which contradicts F6. That file is the coordinator's to change.
   - **`NOTHING_ADMISSIBLE` is an exact-match string** (`lib.rs:31,145`). Nothing else in loom or in commission (174bf07 / 938801c) produces that text, so there is no collision today. But a future selector that adds context to the message would turn "nothing to pick" into an `ExternalAvailability` outage, which is the F5 defect again. INFEASIBLE, note. Fix: a typed selector error.
   - **Frontier for another case:** `Loom::run` ignores `_commission` (`lib.rs:137`) and proposes on a frontier for any case. Commission's `run_until_blocked` (938801c `runtime.rs:333-336`) asks the governor by the commission's case but compares only the revision, and `action_request.rs:48` binds the request to `frontier.case_id`. So a governor that answers for another case gets through both sides. No such governor was found. INFEASIBLE, note. A different revision cannot be checked in Loom, because the commission carries none.
   - **Fit with commission 938801c (read only):** only `runtime.rs` changed since 174bf07, so the port and model types are the same.
     - Loom's outcomes map cleanly onto the loop: a `NeedsAuthority` proposal leads to `check_authority`; `NoUsefulAction` and refused proposals lead to `derive`; Loom's own outages lead to `SuspendRun`.
     - With `FirstAdmissibleSelector`, the loop ends after 3 iterations with `NoAdmissibleAction`: the frontier does not change, so the repeated admission counts as idle.
     - The outage reason shapes differ: Loom sends `{"error": Text}`, Commission's own governor outage is `Text("governor failed: …")` (`runtime.rs:262`).
     - The acceptance test calls `admit` itself rather than going through the loop.

5. **Attacked and could not break:**
   - `pub use loom as model` exposes only `primitives` and `run`. The sealed traits stay private, the same pattern as Commission.
   - `no-hand-model` with `cfg` attributes and nested module files: refused correctly (test above).
   - Taskfile bootstrap, then `generate` twice: idempotent, no debris, and equal to the committed tree.
   - Doc comments at `lib.rs:1-15`, `:29-41` and `:131-134` match behaviour, apart from the finding above.
   - The mutants `.all`→`.any` in `admits_nothing` and `NeedsAuthority`→`NoUsefulAction` are caught by the acceptance and unit tests (by reading the tests; I did not run them as mutants).

6. **Paths written outside the worktree:**
   - `~/.cache/ga-wave-2026-10-04-w8/loom-agent-executor/scratch/adv2-mutant/` — a copy of the workspace with the M1–M4 mutants gated by `LOOM_MUTANT`.
   - `…/scratch/adv2-mutants.log`, `…/scratch/adv2-mutants-m1.log`, `…/scratch/adv2-suite.log`.
   - **The mutant build overwrote this worktree's artifacts** in `~/.cache/b10x-target/loom-w8-agent-executor`, because the binary hashes are the same. The suite run after it recompiled `b10x-loom` from the worktree path, so its results are this tree's. Rebuild before reusing those binaries for anything else.

```findings
- file: crates/loom/src/lib.rs
  line: 155
  category: property
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Loom hands the argument generator the first frontier entry for the selected action, so on a frontier listing it Admissible and ApprovalRequired the proposal depends on entry order while Commission's admit does not (red: generator_is_given_the_same_entry_whatever_the_order)."
- file: crates/loom-xtask/src/main.rs
  line: 610
  category: boundary
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "no-hand-model passes a reserved model type defined through a macro_rules! parameter, e.g. model!(Selection) expanding to pub struct $name (red: no_hand_model_refuses_a_model_type_defined_through_a_macro_parameter)."
- file: crates/loom/tests/adversary_executor_admission.rs
  line: 196
  category: mutant
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "Three pass-1 cases assert only the absence of Suspended(Authority), which Loom no longer builds, and their merge-only frontiers are decided by admits_nothing before the selector runs; with lib.rs:153 mutated to propose refused actions they and loom_agrees_with_commission_admission stay green."
- file: .engineering/planning/story/agent-executor.md
  line: 174
  category: contract-drift
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "The story's Outcome seam (line 78), ESS-first red test (line 119) and Acceptance 2 (lines 174-176) still require Suspended at merge approval and name executor_suspends_at_merge_approval, contradicting decision F6 and the renamed test."
- file: crates/loom/src/lib.rs
  line: 145
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "NOTHING_ADMISSIBLE is matched by exact string equality, so a selector that adds context to it turns nothing-to-select into an ExternalAvailability outage; no producer of a colliding or wrapped string exists today."
- file: crates/loom/src/lib.rs
  line: 137
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "Loom::run ignores the commission and proposes on a frontier for another case; Commission's loop checks only the revision (runtime.rs:333-336 at 938801c), so only a governor answering for the wrong case reaches it."
```
