---
format: aep.planning-md/3
id: story:suite-cycle-writes-a-workspace-file
kind: story
status: proposed
title: A suite cycle can write or replace a workspace file between two runs of one cycle
summary: New step kind write in loom.tree-suite/1 with from (a regular file of the suite, no link, at most 1 MiB) and to (a plain relative path in the cycle's workspace, no .., no link followed, written by create_new and rename); nothing in the run log or store; a failure is a difference of kind write; older Loom refuses StepKindUnknown; SUITE_AVAILABILITY entry
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-suite-format
- depends_on: story:cancel-a-running-bound-program
- depends_on: story:suite-runs-cycles-in-parallel
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree-suite
- confidence: cited
  path: crates/loom-cli/tests/tree_test/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/availability.rs
- confidence: cited
  path: crates/loom-tree/src/suite/decode.rs
- confidence: cited
  path: crates/loom-tree/src/suite/mod.rs
- confidence: cited
  path: crates/loom-tree/src/suite/path.rs
- confidence: cited
  path: crates/loom-tree/src/suite/run.rs
- confidence: cited
  path: crates/loom-tree/tests/suite_format/mod.rs
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/run-a-case-tree.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T12:27:09Z", actor: "agent:loom", revision: 8}
---
## Outcome

A cycle of a `loom.tree-suite/1` suite can write or replace one file of its own workspace between
two of its steps: the new step kind `write` copies a file of the suite's directory to a path in
the cycle's workspace. A person's decision written into `findings.yaml` between two runs is then
a suite step instead of a gap.

```yaml
steps:
  - {do: open}
  - {do: run, rounds: 1, exit: 3}
  - {do: write, name: owner-decides, from: seeds/findings-decided.yaml, to: findings.yaml}
  - {do: run}
```

Security-relevant: the test harness writes a file by a name the suite gives, into a directory the
programs of the cycle have written to before.

### Today

- No step changes the workspace. Seeds are copied only before the first step
  (`crates/loom-tree/src/suite/run.rs:887-926`). The twelve step kinds (`decode.rs:667-683`)
  each run a tree command, and "editing a store or a workspace between steps" is out of scope of
  `story:tree-suite-format`.
- Runbook (`docs/cycles-suite.md` at `impl/runbook-test-suite`) holds 8 entries pending for this
  gap: `finding-person`, `story-decision`, `story-late-person`, `trace-gap-drop` and
  `trace-constraint-decided` (lines 151-155), `cite-dark-spot` and `cite-fault-sent-back`
  (244-245), and `implement-percause` (276). Each needs "a person's decision written into
  `findings.yaml` between two runs".

### The step

- **Kind and members.** `do: write` takes `from`, `to`, `name` and `checks`. Both `from` and `to`
  are required; a missing one is `MemberMissing`. `at`, `exit`, `refusal` and every
  command member (`case`, `principal`, …) are `MemberUndeclared` on a `write` step, because it
  runs no tree command and has no exit.
- **`to`** is a path in the cycle's workspace (`<scratch>/<n>/work`). It is refused before any
  cycle with `PathOutsideCycle`, naming the member, when it:
  - is empty, absolute or holds a NUL or a backslash (the `stays_below` rules, `suite/path.rs:
    192-212`);
  - holds any component that is not a plain name: no `..` at all, not even one that stays below,
    and no `.`.

  `to` names a path in the workspace only, never the store.
- **`from`** is a path of the suite's directory, refused `PathOutsideSuite` by the rules of a
  seed's `from` (`decode.rs:325-336`, `:564-578`). Before any cycle, beside the seeds' check
  (`run.rs:465-478`), it must be a regular file:
  - not a link, and with no link at any component between the suite's directory and it;
  - at most `MAX_WRITE_BYTES`, 1 MiB.

  Otherwise it is `SeedUnusable`, naming `cycles[<i>].steps[<j>].from`. Exit 2, and nothing is
  written.
- **At the step:**
  1. The bytes of `from` are read again, at most 1 MiB. A `from` that is now over 1 MiB, or no
     longer a regular file, is the difference `StepFailed`.
  2. Each component of `to`'s parent is walked from the workspace:
     - an existing one must be a directory and not a link;
     - a missing one is made with `create_dir`, which fails when something appeared meanwhile.
  3. The last component must be absent, or a regular file that is not a link.
  4. The bytes go to a new file in that directory, created with `create_new` and mode `0644`. It
     is renamed over `to`. A rename replaces the directory entry and never follows a link, so the
     write never reaches a file outside the workspace.
- **The step's outcome.**
  - A link at any component of `to` is the difference `PathOutsideCycle`.
  - Any other entry at `to` (a directory, a fifo) is `StepFailed`, found as the entry's kind. So
    is an I/O error, found as its kind. Neither ever shows the content.
  - The difference names the step (its `name`, else `steps[<i>]`) with kind `write`, and it ends
    the cycle as any first difference does.
  - A `write` that succeeds gives no output to `{step: <name>}`. Its `checks` are evaluated right
    after it, and a `name` can be named by an `after`.
- **Order.** Steps run in list order (`run.rs:792-872`). A `write` runs after the step before it
  has returned. For a `run`, that means after `run_tree` returned and its launches ended. Under
  `local`, a background process a program left behind could still touch the workspace; `local`
  stays the declared-insecure level.
- **Cancel.** A `write` runs no tree, so it never gets a stop.
  - A `write` passes the check that `story:tree-suite-format` makes before every step
    (`run.rs:797-805`). This story does not change that check and adds no claim about it.
  - A `write` that has started finishes. Its bytes are bounded and the rename is one call, so a
    cancel never leaves a partial file at `to`.
  - A `run` before it that is cancelled ends `Cancelled` (`story:cancel-a-running-bound-program`,
    exit 5). `story:suite-runs-cycles-in-parallel` turns that into the difference `StepFailed`,
    found "the suite was cancelled". The cycle ends there, and the `write` after it never runs.
- **Time.** A `write` leaves the minute count of `at` unchanged (`run.rs:806`). Adding one never
  moves the instants of the steps after it.
- **What records it.** Neither the run log nor the store records the write: it is the test
  harness, not a tree action. The store is left byte for byte as it was. The report shows the
  step only through a difference. The cycle line and `CycleResult` are unchanged.
- **Availability.** `SUITE_AVAILABILITY` (`availability.rs:65`) gains `entry(Step, "write",
  Available)`, and `step_name` (`availability.rs:120`) gains its name.
- **Compatibility.** The format stays `loom.tree-suite/1`, and the change only adds to it. A suite
  without `write` reads and runs as before. A Loom without this story refuses `do: write` before
  any cycle with `StepKindUnknown` (`decode.rs:733-739`), exit 2, naming `cycles[<i>].steps[<j>].
  do`. It never runs such a suite with the step skipped.

## Decisions

- **D1, a step kind or a member.**
  - A (default): the step kind `write`, its own step in the list, ordered against the runs by
    its place.
  - B: a `write` member on any step, applied before the step runs. An older Loom then refuses it
    `MemberUndeclared`. The member hides the order inside another step and couples the write to
    that step's `exit` and `refusal`.
- **D2, what is written.**
  - A (default): a whole file, replaced by the bytes of a file of the suite. Runbook's stub
    programs write deterministic files, so the decided file is fixed and is kept beside the
    suite.
  - B: setting a value at a path inside a YAML file. That is an edit language, which
    `story:tree-suite-format` excluded along with query strings.
- **D3, missing directories.**
  - A (default): missing parent directories in the workspace are made, as a seed's are
    (`run.rs:918-921`).
  - B: a missing parent is `StepFailed`.

## ESS first

In `ess/domains/tree.yaml`:

- `loom.tree.StepKind` (`:499-502`) gains `write`, and its comment says "or one file of the
  suite written into the workspace".
- `loom.tree.CycleStep` (`:504-546`) gains `from: Optional<String>` and `to: Optional<String>`,
  and its comment names them as the source in the suite and the target in the workspace of a
  `write`.
- The comment of `loom.tree.SuiteRefusalReason` (`:1034-1067`) says that `SeedUnusable` also
  covers a `write` step's `from`: a link, absent, not a regular file, or over 1 MiB.
- The comment of `loom.tree.DifferenceReason` (`:933-954`) says that `PathOutsideCycle` covers a
  link at a `write` step's `to`.
- The comment of `loom.tree.CheckDifference` (`:956-974`) adds `write` to its kinds.

No new type, no new variant beyond `write`. The generated Rust and JSON Schema are regenerated.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step of the gate ends within 120 s.
- No test makes a network request or a model call.
- Bound programs and fixtures are POSIX sh, never Python.
- A build or test job in the slot pool ends within 10 minutes, or names its time with
  `slot --timeout` (at most 30 minutes).

Where the tests go:

- Tests that run cycles go in `crates/loom-cli/tests/tree_test/mod.rs`. They use its harness:
  `local_settings` (`:330-344`), the protocols fixture (`:30-33`) and `fresh_scratch`
  (`:346-353`).
- Their suites, seeds and POSIX sh stubs go below `crates/loom-cli/tests/fixtures/tree-suite/
  write/`. Each asserts its own time under the module's 5 s (`within_bound`, `:183-186`).
- Tests that only decode (tests 5 and 7) go in `crates/loom-tree/tests/suite_format/mod.rs`,
  with their YAML inline.
- Paths outside the suite that a stub needs (an outside directory, the suite's own `from` file)
  reach it through a seeded file the test writes.
- Every `from` holds the text `CANARY-w7rite`.

Tests, in `tree_test/mod.rs` unless marked:

1. `a_write_between_two_runs_is_read_by_the_second`.
   - The first `run` of a stub writes `findings.yaml` with an open finding as its last act, after
     a 0.5 s sleep, and stops at exit 3.
   - The `write` replaces the file from the suite.
   - The second `run` reads the decision and ends at exit 0.
   - A `doc` check on `work/findings.yaml` gives the decided value, and the cycle is as expected.

   This also shows that the `write` ran after the first `run` returned: a write during the run
   would be overwritten by the program's last act.
2. `a_write_makes_a_new_file_and_its_directories`:
   - `to: a/b/new.txt`, with no `a/`, gives that file with the bytes of `from` and mode `0644`.
   - A second `write` over an existing file whose mode is `0600` leaves mode `0644`.
3. `a_write_never_follows_a_link`. A first-run program makes `out -> <outside>/f` and `up -> ..`
   in the workspace. `<outside>` is a fresh test directory.
   - `write` to `out` ends `PathOutsideCycle`, and `<outside>/f` is unchanged.
   - `write` to `up/x` ends `PathOutsideCycle`, and nothing exists at the cycle directory's `x`.
4. `a_write_onto_what_is_no_file_fails_the_step`. In three cycles, a first-run program makes
   something at `to`, and the step ends `StepFailed` found as:
   - `directory`, for a directory at `to`;
   - `fifo`, for a `mkfifo` at `to`;
   - `permission denied`, for `to: ro/x` with `ro/` set to mode `0555`.
5. `write_paths_are_refused_before_any_cycle` (`suite_format/mod.rs`, through `decode_suite`).
   Each names the member:
   - `to` of `/etc/x`, `../x`, `a/../b`, `./a` and empty: `PathOutsideCycle`;
   - `from` of `../x`: `PathOutsideSuite`.
6. `a_write_source_must_be_a_small_regular_file`. Each of these is `SeedUnusable` naming
   `cycles[0].steps[1].from`, before any cycle, with `--scratch` left absent:
   - a `from` that is a link;
   - a `from` behind a linked directory;
   - a directory;
   - an absent file;
   - a file of 1 MiB + 1 byte.
7. `write_declares_only_its_members` (`suite_format/mod.rs`). `exit`, `refusal`, `at` and `case`
   on a `write` are `MemberUndeclared`. A missing `from` or `to` is `MemberMissing`.
8. `a_from_that_grew_or_turned_into_a_link_fails_the_step`.
   - The test copies the fixture suite directory into its own `fresh_scratch` directory and runs
     the copy, so the committed fixture is never changed.
   - The first-run program is given the path of the copy's `from`. It either appends past 1 MiB
     to it, or replaces it with a link.
   - Each ends the `write` with `StepFailed`, and nothing is written at `to`.
9. `a_write_leaves_the_store_and_the_log_alone`. The first `run` step is named `first`. On the
   `write` step stand two `compare` checks, each with `after: first` on the left and the same
   selection now on the right, `op: eq`:
   - `{files: {in: store, glob: "**"}}` with `take: count`;
   - `{doc: {in: store, path: <the root case's run log, log_file_name("root")>}, as: text}` with
     `take: digest`.

   The cycle is as expected, so both hold, and `tree verify` passes at its end.
10. `a_write_keeps_the_instants_of_later_steps`: the `at` of the second run's first log line is
    the same with and without the `write` step.
11. `a_failed_write_never_shows_the_content`. In every difference of tests 3, 4 and 8, the
    `CycleResult` JSON and the printed cycle line hold no `CANARY-w7rite`.
12. `a_cancelled_run_before_a_write_leaves_to_alone`. run_suite runs on a thread of the test.
    - The first run's program writes a started marker, then waits (bounded 5 s) for a release
      file the test never writes.
    - The test waits for the marker, then sets the flag while that `run` is in flight.
    - The program is killed, and the `run` ends `Cancelled`. The cycle ends `StepFailed` found
      "the suite was cancelled", naming the `run` step and not the `write`.
    - `to` does not hold the bytes of `from`.
13. `tree_test_runs_a_write_suite`: `b10x-loom tree test --confinement local` on test 1's suite
    prints `<name>: as expected` and exits 0.

Docs: the guide's step table (`website/docs/guides/run-a-case-tree.md:428-438`) gains the `write`
row and its path rules.

## Out of scope

- Editing the store between two runs. Runbook's `rework-pending-rejection`,
  `writer-not-called` and `principals` (lines 193, 195, 197) edit the store: a case cut, a
  program replaced, a decision written by hand. The store is Loom's record, and a suite does not
  write it.
- Removing a file, a link, or a mode other than `0644`.
- Writing outside the cycle's workspace, into another cycle's workspace or into the suite's
  directory.
- A value set inside a file (D2 B).

## Scope

Derived 2026-11-22 from the plan tree at `5bcc26d9` (`wave/2026-11-21-w7`). **Cited** means read
at the `file:line` named. **Inferred** means a reading that could be wrong.

- **ESS and generated:** `ess/domains/tree.yaml` (`:499-546`, `:933-974`, `:1034-1067`),
  `generated/rust/loom`, `generated/ess/loom` — cited.
- **Suite:**
  - `crates/loom-tree/src/suite/decode.rs`: `suite_path` and `cycle_path` (`:325-349`), `seed`
    (`:564-578`), `step_kind` (`:667-683`), `step_members` (`:685-707`), `step` (`:730-787`) —
    cited.
  - `crates/loom-tree/src/suite/path.rs`: `stays_below` (`:192-212`) is read, and a stricter
    plain-name check is added beside it — cited.
  - `crates/loom-tree/src/suite/run.rs`: `usable_seeds` (`:465-478`), `steps` (`:777-885`),
    `seed` (`:887-926`), `step` (`:940-1037`) — cited.
  - `crates/loom-tree/src/suite/availability.rs`: the table (`:65`) and `step_name` (`:120`) —
    cited.
  - `crates/loom-tree/src/suite/mod.rs`: the module doc and the export of `MAX_WRITE_BYTES` —
    cited.
- **Tests:**
  - `crates/loom-cli/tests/tree_test/mod.rs` (`:30-33`, `:183-186`, `:330-353`) and
    `crates/loom-tree/tests/suite_format/mod.rs` — cited.
  - `crates/loom-cli/tests/fixtures/tree-suite`, where the new `write/` lives — cited.
- **Docs:** `website/docs/guides/run-a-case-tree.md` (`:428-438`), `CHANGELOG.md`
  `[Unreleased]` — cited.

## Edges

- `depends_on story:tree-suite-format`, merged at `2e74232a`: the format, decoder and runner this
  story adds a step kind to.
- `depends_on story:cancel-a-running-bound-program` (active). Shared: `stop_exit`
  (`suite/run.rs:623-630`), which must map its new `TreeStop::Cancelled`, and
  `ess/domains/tree.yaml`. This story's cancel rule reads that stop.
- `depends_on story:suite-runs-cycles-in-parallel` (draft). Test 12 reads the mapping that story
  makes, from a `run` that ended `Cancelled` to the difference "the suite was cancelled".
- **Would collide with**, no edge:
  - `story:suite-runs-cycles-in-parallel` (draft). Shared: `suite/run.rs`, `suite/mod.rs`,
    `tree.yaml` (both edit `loom.tree.SuiteRefusalReason`: that story adds variants, this one
    changes the comment, a textual merge), both test modules, `crates/loom-cli/tests/fixtures/
    tree-suite` and the guide. That story merges first, and this story merges its head.
  - `story:cancel-a-running-bound-program` (active), beside its edge: `suite/run.rs`
    (`stop_exit`) and `tree.yaml` (`TreeStop`). It merges first.
  - `story:suite-runs-cycles-under-one-cgroup-root` (draft). Its layout moves the workspace to
    `<scratch>/work/<n>/`, which this story's tests read through `in: work` and the run harness.
    It merges after this story.
  - `story:person-action-runs-a-program` (active): `suite/run.rs` and `tree.yaml`. Whichever
    merges second merges the other's head.
  - `story:tree-test-command` (draft) and `story:tree-test-decide-and-requests` (draft). Shared:
    `suite/decode.rs`, `suite/availability.rs`, `suite/run.rs`, the `DifferenceReason` block of
    `ess/domains/tree.yaml`, both test modules and the guide. This story merges before
    `tree-test-command`, which merges its head. `tree-test-decide-and-requests` comes after
    that.
