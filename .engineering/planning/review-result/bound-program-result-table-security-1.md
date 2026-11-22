---
format: aep.planning-md/3
id: review-result:bound-program-result-table-security-1
kind: review-result
status: active
title: 'Security review, pass 1: bound-program-result-table (wave 2026-11-21-w7, U2, head 6b36644b)'
relations:
- reviews: story:bound-program-result-table
- reviews: story:bound-program-run-cap-holds-across-ports
revision: 1
---
unit: story:bound-program-result-table, worktree loom-w1121g-u2 at head 6b36644b, plus one new untracked test file
verdict: red
cases: executed 142→154, red 4
origin: introduced 4, pre-existing 0, undecided 0
wrote-outside-worktree: 6 paths (listed below)
needs-coordinator: yes (whether F1, the signal-killed run, blocks the merge)

1. `git --no-pager diff --stat` is empty: no tracked file changed. `git status --short` shows only `?? crates/loom-bindings/tests/security_w20261121w7_u2.rs`, a test file.
2. My cases are in that file, run alone before the suite (`sec1/case-run2.log`: "8 passed; 4 failed"). The 4 red ones:
   - `a_bound_program_that_ends_by_a_signal_counts_against_its_max_runs`: "started 3 times". Each step was "refused by its runner: CapabilityUnserved: … no exit status … signal Some(9)".
   - `a_bound_program_whose_blob_cannot_be_kept_still_counts_against_its_max_runs`: "started 3 times".
   - `two_ports_over_one_case_store_start_a_capped_action_at_most_max_runs_times`: "started 2 times through two ports over one case".
   - `a_case_of_another_protocol_records_no_kind_that_protocol_lacks`: "a case of other@1, which declares no gate kind, holds [EvidenceData { kind: "gate" …".
3. Suite: `CARGO_INCREMENTAL=0 cargo test -p b10x-loom-bindings --locked`, exit 101. lib 2, adversary 23, pass2 20, bindings_document 34, bound_program 63 all passed: 142, the implementor's count. My file: "8 passed; 4 failed". The five wave 6 intake files: exit 0, 4/3/10/4/4 passed, the same as `base-intake.log`.

Per invariant
1 holds. The only process starts are `run_child` at `program.rs:201` (the bound program) and `:339` (`arguments_from`), plus `bindings.rs:340` in the intake slice. There is no `Command`, fork or exec under `crates/loom-bindings/src`. The only `cfg(test)` is the unit test module at `program.rs:709`.
2 holds. Admission and preflight run at `program.rs:110-111`, before the port exists. The implementor's tests cover both refusals. My `the_public_admission_refuses_local_for_the_file_reason_and_near_misses_of_the_flags` is green.
3 holds. The child gets `PATH` alone, read from the host process (`bindings.rs:422-430`); a non-UTF-8 `PATH` is refused. `model_chosen_arguments_reach_standard_input_alone` is green: argv, environment and start directory are the binding's, and the arguments arrive only on stdin.
4 holds. `arguments_from_output_reaches_standard_input_alone_and_over_the_bound_starts_nothing` is green. Output over 1 MiB, or output the runner cut, is refused `ArgumentsUnreadable` and the bound program does not start.
5 holds. `report_members_named_like_every_record_field_change_nothing` is green. A report naming result, kind, subject, producer, revision, subject_revision, observed_at, upstream_revisions and evidence left the recorded facts unchanged.
6 holds. Green: `a_member_named_twice_through_an_escape…`, `a_pointer_with_escapes_and_an_index…`, `a_pointer_target_of_another_type…`. A report nested 80 deep is refused. The implementor's own test covers a missing pointer target.
7 holds. The blob path is built only from the store directory and the digest (`program.rs:520,542`). A `blobs` that is a link or a file is refused (`:521`). A link at the blob name is replaced by the rename, never written through (`:554`). The temporary file has a random name and is created exclusively. Only the host chooses the store directory, as a constructor argument; no document, request or report can. If that directory is itself a link, it is followed.
8 does not hold: see F1 to F3.
9 holds. `select_runner` keeps its order: flag `:350`, file `:354`, its own `NoRunner` `:364`, then the public admission. The admission's flag branch is never reached from `select_runner`. The mount condition (`:441`) is the same as at the base, and no intake test file was added or changed. As API: a caller that opens `local` with `UNCONFINED_FLAG_REASON` itself is admitted, but wave 6's host path already allowed that.
10 holds, by reading. 14e7b640 changes only `ess/domains/bindings.yaml`, 8cae18c5 only `generated/rust/loom`, and there is no `UNMAPPED:` marker. I did not re-run `task drift`. No enum that existed at the base gained a variant, so "No Breaking bullet" is right.

Findings (all against 6b36644b)
F1, blocker, NEEDS-CHANGE, introduced. `program.rs:201-208`: if the runner returns an error after the child started and ended, no observation is kept, so that run never counts. Under `local`, a child killed by a signal ends this way (`engine.rs:566`). What reaches it: any host of the public `ProgramEffects`, though no host in the tree uses it yet. A program that crashes on input the model chooses can then be retried without limit, and the guide's "at most `budget.max_runs` times" is false. Suggested fix: keep the observation before `run_child`, or after every `run_child` result.
F2, warning, NEEDS-CHANGE, introduced. `program.rs:213-214`: if the blob cannot be written after the run, the step returns an `EffectError` and keeps no observation. What reaches it: a program that can write the store directory (any program under `local`), or a disk or permission failure. Under `linux` with the store outside the workspace, only the second.
F3, warning, CONFIRMED, introduced. `program.rs:158-201`: reading the count and starting the program are not one step, so two ports over one `FileCaseStore` can both start at max_runs-1. What reaches it: two hosts or two threads on one case; `LockedDir` documents two processes over one store as supported.
F4, warning, CONFIRMED, introduced. `program.rs:102-120`: the port never checks `document.protocol` against the case's protocol, and the governor accepts evidence of a kind the case's protocol does not declare. What reaches it: a host that pairs a document with a case of another protocol. That answers the brief's question: such a document records its own kinds into that case.

Checked and found nothing wrong:
- Duplicate-member detection gives the same answer in both JSON readers: escaped names are decoded, and the depth limit of 64 is below serde's.
- A timed-out run reads `timeout` and records no exit code (`engine.rs:614`).
- The prose in `AGENTS.md`, the guide and `lib.rs` matches the code, except the budget sentence (F1).
- The boundaries, the manifest lines, the lock block and the absence of any `/home/` literal all match the brief.

Not established: how the `linux` adapter reports a signal-killed child (I had no delegated cgroup scope), and `task drift` (not re-run).

Written outside the worktree, under `~/.cache/scratch/94200cb9-e807-4544-b8fe-80c8ce4c8ba2/wave-2026-11-21-w7/u2/`: `sec1/story.md`, `sec1/case-run1.log`, `sec1/case-run2.log`, `sec1/suite.log`, `sec1/intake5.log`, `sec1-report.md`.

```findings
- file: crates/loom-bindings/src/program.rs
  line: 201
  category: integrity
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "a run that started and ended by a signal comes back as a runner refusal, keeps no observation, and never counts against budget.max_runs"
- file: crates/loom-bindings/src/program.rs
  line: 213
  category: integrity
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: "a blob write failure after the program ran keeps no observation, so a program that can write the store runs past max_runs"
- file: crates/loom-bindings/src/program.rs
  line: 158
  category: concurrency
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the run count is read and the program started without holding the store, so two ports over one case both start at max_runs-1"
- file: crates/loom-bindings/src/program.rs
  line: 110
  category: judgement
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: "the port never checks the document's protocol against the case's, and records kinds the case's protocol does not declare"
```
