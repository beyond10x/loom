---
format: aep.planning-md/3
id: review-result:adversary-w15-loom-argument-generator-pass-1
kind: review-result
status: active
title: Wave 2026-10-04-w15 adversary, loom story:argument-generator, pass 1
relations:
- reviews: story:argument-generator
revision: 1
---
```
unit: loom/argument-generator, working tree on b1f916f (phase 2 uncommitted) plus crates/loom/tests/adversary_argument_generator.rs
verdict: CONFIRMED
cases: executed 540→550, red 1
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 2 (scratch/adversary-p1/gate.log, scratch/adversary-p1/mA.log)
needs-coordinator: whether Loom::selections should keep both selections from two runs on one frontier, or its doc should say a re-run replaces the earlier one
```

I found one real defect: a doc comment that doesn't match the code. I also found a gap in the test suite, which my new cases now close. The full gate is red only on my new failing case.

**1. `git --no-pager diff --stat`** — this matches the implementor's 9 files exactly. My only addition is the untracked `crates/loom/tests/adversary_argument_generator.rs`. I touched no non-test path.

**2. Cases added** (in `adversary_argument_generator.rs`; I ran this file alone before anything else):

| case | asserts | now |
|---|---|---|
| `the_request_is_recorded_before_the_generator_is_called` | the generator reads its own Loom's record during `generate()` and the request is already there (this also shows the lock is not held during the call) | green |
| `a_failing_generator_suspends_and_leaves_its_request_recorded` | the run suspends with the generator's message; the request stays `Requested`, the only state run.yaml declares | green |
| `duplicate_frontier_entries_hand_the_generator_one_entry` | duplicate actions in the frontier lead to one entry, one generator call, and no other action id | green |
| `an_approval_required_entry_is_handed_as_approval_required` | the generator gets `{merge, ApprovalRequired}` and the action is proposed | green |
| `a_selector_error_records_nothing_and_calls_no_generator` | `Unavailable` gives Suspended and `NothingAdmissible` gives NoUsefulAction; with either, no selection, no request, no generator call | green |
| `a_selection_outside_the_catalogue_records_nothing` | NoUsefulAction, and nothing is recorded | green |
| `two_frontiers_on_one_loom_keep_their_requests_apart` | 2 selections and 2 requests; each request names its own selection | green |
| `a_second_run_on_the_same_frontier_proposes_again` | no refusal on a re-run | green |
| `a_panicking_generator_does_not_break_the_next_run` | after `catch_unwind`, the next run proposes and records | green |
| `every_selection_made_on_one_frontier_is_listed` | two runs on one frontier choose INSPECT and then TESTS_RUN; `selections()` lists both | **red** |

The red output, from running the file alone (9 passed, 1 failed):
```
assertion `left == right` failed: every selection this Loom has made, in the order it made them
  left: ["tests.run"]
 right: ["repository.inspect", "tests.run"]
```

**Mutant A** was run in a scratch copy with its own target dir; it makes `run()` record the request only after the generator succeeds. Every pre-existing b10x-loom target stayed `0 failed` (log: `mA.log`). Three of my cases caught it: the ordering case (`left: 0 right: 1`), the failing-generator case (`left: 0 right: 1`) and the panic case (`left: 1 right: 2`). The implementor's m2 only checked that a request exists after the run, so nothing in the suite was testing the order before my cases.

**3. Gate** (worktree, `CARGO_TARGET_DIR=~/.cache/b10x-target/loom-w15-argument-generator`; I added `--no-fail-fast` to step 3 so every target runs):

| step | exit |
|---|---|
| `cargo fmt --check` | 0 |
| clippy `-D warnings` | 0 |
| `cargo test --workspace --locked` | **101** |
| `ess_gate` | 0 (`10 passed; 0 failed`) |
| ess validate + synthesize | 0 (`loom v1 — 2 file(s), valid`; `18 scenario(s) (0 authored), 0 refusal(s)`) |
| drift / no-hand-model / docs `--check` / cargo doc | 0 / 0 / 0 / 0 |

- Workspace summary lines: 33, with 549 passed, 1 failed, 2 ignored.
- The only failing line: `test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` (adversary_argument_generator, `:440`).
- `--list` shows all 10 new cases in this tree.

**4. Judgement findings** (on the working tree above):
- **`lib.rs:70` — CONFIRMED, introduced (warning).**
  - *What was measured:* the red case at `:440`. In `run()`, the selection id and the request id are both the frontier id (`:169`). So the second selection's `put` (`:189`) replaces the first. The INSPECT selection, which Loom made and proposed, is gone from the record.
  - *What reaches it:* any second `run()` on the same frontier, for example a retry after `Suspended`. Commission's own testkit calls `run` twice on one frontier (`adversary2_executor_fake.rs:63-86`). I found no production caller in Loom or Commission.
  - *Possible fixes:* give each run its own selection and request ids, or change the doc to "the latest selection per frontier".
- **`arguments.rs:119` — INFEASIBLE through `Loom::run`, introduced (note).** `run()` always stores the selection as `Selected` just before the request, and nothing moves the private record. So the `selection-not-selected` refusal and the `no_useful_action` branch after it (`lib.rs:213`) can't be reached from `run()`. They can be reached only through a `RequestRecord` someone uses directly. A re-run also overwrites a held selection with `Selected` regardless of its previous state. That would bring a `Refused` selection back to life once story:selection-revalidation starts moving the record.
- **`lib.rs:57` — CONFIRMED, introduced (note).** The record only grows: a long-lived Loom keeps one selection and one request per frontier, and nothing ever prunes them.

**5. Attacked and not broken:**
- The generator only ever saw the selected entry: with duplicate frontier entries, with an `ApprovalRequired` entry, and with selector errors.
- `RequestArguments` matches run.yaml and the PLAN contract: both refusals and `requested` behave as declared, nothing undeclared happens, and `ArgumentRequest` is built from the generated type.
- The request is recorded before the generator is called. A failed generator leaves the request `Requested`, which is the only state the spec declares.
- Ids from two different frontiers stay apart.
- Lock poisoning: the guard is never held across selector or generator code, and `PoisonError::into_inner` handles it anyway.
- Docs: the contract and concept pages and both status.mdx rows match the behaviour.

**6. Paths written outside the worktree:**
- `~/.cache/ga-wave-2026-10-04-w15/loom-argument-generator/scratch/adversary-p1/gate.log`
- `~/.cache/ga-wave-2026-10-04-w15/loom-argument-generator/scratch/adversary-p1/mA.log`
- The scratch copy `adversary-p1/tree` and its target dir `adversary-p1/target` are deleted.
- `$CARGO_TARGET_DIR/suite.json` was rewritten by the gate step, in the unit's build dir.

My worktree lease is released.

```findings
- file: crates/loom/src/lib.rs
  line: 70
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "Loom::selections promises every selection made in order, but a second run on the same frontier replaces the first selection because the selection id is the frontier id (red case adversary_argument_generator.rs:440)"
- file: crates/loom/src/arguments.rs
  line: 119
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "the selection-not-selected refusal and run()'s non-Requested branch cannot be reached from Loom::run, and a re-run puts a held selection back to Selected whatever its prior state"
- file: crates/loom/src/lib.rs
  line: 57
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "the per-Loom RequestRecord grows by one selection and one request per frontier and is never pruned"
```
