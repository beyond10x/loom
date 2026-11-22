---
format: aep.planning-md/3
id: review-result:bound-program-result-table-security-2
kind: review-result
status: active
title: 'Security review, pass 2: bound-program-result-table (wave 2026-11-21-w7, U2, head 3a016448)'
relations:
- reviews: story:bound-program-result-table
revision: 1
---
unit: story:bound-program-result-table pass 2, `git diff 6b36644b..3a016448`, worktree at 3a016448 plus 4 cases added to my file (uncommitted)
verdict: green
cases: executed 156→160, red 0
origin: introduced 2 (both notes), pre-existing 0, undecided 0
wrote-outside-worktree: 4 paths: `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w7/u2/sec2/{new-cases.log,fmt.log,suite.log}` and `…/u2/sec2-report.md`
needs-coordinator: no

1. **F1 and F2 are closed.** My two cases pass unchanged.
   - Between `run_child` (`program.rs:219`) and the observation (`:248-256`), nothing returns early. A blob failure is held until after the observation is written, and the clock falls back to `started_at`. The `?` at `:254` only serializes known fields, and I found no way for it to fail. The one uncounted end left is the store refusing the write (N2).
   - The runner's refusal text (`:244`) goes only into the observation and the step's refusal text. No result, kind, subject or path is read from it. The port does not limit its size. New case `a_runner_refusal_text_is_data_and_names_no_blob_no_record_and_no_file` is green: a 64 KiB text shaped like JSON and like a path leaves no blob, no record and no file, and the next step is `RunsSpent`.
   - Nothing treats a null `blob_sha256` as a digest. Records cite the observation id, and nothing outside `program.rs` reads the observation.
   - If the observation cannot be written, the step returns an `EffectError` naming the governor, and the run is not counted (N2). Only a store write failure causes this. A program can cause it only where it can write the store (under `local`), and there it could edit the store anyway. The model cannot.
   - `arguments_from` (`:374`) leaves no observation, as the planner decided. While it keeps failing, every step starts it again without counting (N1). It always gets `{}` on stdin, so the model chooses nothing it reads. `RunsSpent` is checked before it (`:173`), and when it succeeds the bound program runs and counts. Only the loop's step budget limits the failures.
2. **F4 is closed.** My case passes unchanged.
   - The check compares the whole registration, `<name>@<major>`: the governor's value (`lib.rs:1118`) against the document's (`program.rs:162-163`). It runs before `RunsSpent` (`:173`), `arguments_from` (`:201`) and `run_child` (`:219`).
   - Two new cases are green. `a_case_of_the_same_protocol_name_and_another_major_is_refused_and_starts_nothing`: fixture@2 is refused. `a_case_the_governor_does_not_hold_starts_nothing_and_records_nothing`: the step fails with a governor error, nothing starts and nothing is kept.
   - `CaseOfAnotherProtocol` is declared in `ess/domains/bindings.yaml:320`. Its enum does not exist at c1a1f443, so this adds no variant to a base enum.
3. `a_blob_directory_that_is_a_link_is_never_written_through` still asserts that nothing is written through the link (`bound_program.rs:1034`), as well as the new observation check.
4. **My file:** the twelve cases have the assertions, inputs and expected values I wrote; the only addition is the `#[ignore]` at `:701`. A correction to my pass 1 report: at `:420` the input of `a_member_named_twice_through_an_escape…` is a plain duplicate, not an escaped one. My own write tool decoded `\u0061` when I created the file; the implementor did not change it. New case `a_member_named_twice_through_a_unicode_escape_is_refused_and_records_nothing` builds the escape at run time and is green.
5. **Prose:** guide `:227-236`, AGENTS.md `:1224-1243`, CHANGELOG `:19` (an unreleased bullet) and status.json `:23` are true of the code. One exception: "whatever the runner answers" is false when the observation write fails (N2).
6. The wave 6 path is untouched: `git diff 6b36644b..3a016448 --stat -- crates/loom-intake-slice crates/loom-governor` prints nothing.

**Suite:** `cargo test -p b10x-loom-bindings --locked --no-fail-fast`, exit 0: 159 passed, 0 failed, 1 ignored. That is the implementor's 155 plus my 4 new cases. My file passes `rustfmt --check`.

**Findings** (both notes):
- **N1:** CONFIRMED, introduced, `program.rs:374`. A failing `arguments_from` is started again on every step and never counted. This is the planner's decision; it is listed so the planner sees it.
- **N2:** INFEASIBLE, introduced, `program.rs:256`. A store write failure leaves a started run uncounted. Only a program that can write the store can cause it.

**Not established:** how large a refusal text a `linux` runner can return, and whether that text can carry the child's output.

**`git status --short`:** ` M crates/loom-bindings/tests/security_w20261121w7_u2.rs` (+160 lines, 4 new cases, for the implementor to commit).

```findings
- file: crates/loom-bindings/src/program.rs
  line: 374
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: "a failing arguments_from program is started again on every step with no observation, bounded only by the loop's step budget"
- file: crates/loom-bindings/src/program.rs
  line: 256
  category: integrity
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: "when the store refuses the run's observation the step errors and the started run is not counted; only a program that can write the store can cause it"
```
