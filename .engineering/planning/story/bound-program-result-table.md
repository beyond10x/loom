---
format: aep.planning-md/3
id: story:bound-program-result-table
kind: story
status: proposed
title: A bound program's exit code or report field becomes evidence through the binding's result table
summary: The program effect port for any registered protocol; the program never names a result, revision, producer or instant
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bindings-document
- depends_on: story:bound-commands-run-confined
scope:
- confidence: cited
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/tests/bound_program.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
---
## Outcome

An action bound to a `program` executor in a `loom.bindings/1` document runs through an effect
port in `b10x-loom-bindings` (`crates/loom-bindings`) for any registered protocol, and its result
becomes evidence by the binding's table alone. The program runs through the injected runner with
its arguments as JSON on standard input and a cleared environment; its standard output and
standard error are kept as one content-addressed blob that the observation names. The arguments
are the request's, else the binding's literal `arguments`, else the object the `arguments_from`
argv prints, run the same way. For each `evidence` entry Loom reads the value `from` names (the
exit code, or a JSON pointer into the report on standard output), looks it up in `results`, and
submits one record of that kind about the entry's `subject`, with the binding's `producer`, the
observation's instant and the `upstream` artifacts' revisions. A value the table does not list
produces no record and the step is refused. A program never names a revision, a producer, an
instant or an evidence result.

The record's subject revision is the subject's revision in the case after the step's revision
moves. In this story nothing moves, so it is the revision the case held when the program started;
`story:bound-artifact-revisions` adds the moves and writes them and the step's evidence in one
store write.

## Why

Loom turns a bound command into evidence only from a report that itself lists `{kind, result}`
(`crates/loom-intake-slice/src/bindings.rs:443-489`), so the program decides its own evidence
result, and no production code implements `EvidenceAdapter`
(`crates/loom-commission/src/ports/evidence.rs:49-52`; the only `impl` is in a test). The SDC
driver has the table as data (`~/beyond10x/sdc/driver/src/main.rs:748-770`: exit code or `*` to
result, per kind for several kinds) and that is what lets a plain tool such as `ess specify
validate` produce `valid` or `invalid` without a wrapper. `~/beyond10x/sdc/docs/boundary.md` row
19: the mechanism is runtime, the tables are application data. The SDC's `stories.open` takes its
list of stories from an `args_from` program
(`~/beyond10x/sdc/driver/examples/sdc/sdc-epic.bindings.json:63-66`).

## ESS first

`ess/domains/bindings.yaml` first: `ProgramRun` (action, exit, blob digest, timed out),
`RecordedEvidence` and the refusals `ResultNotInTable`, `ReportUnreadable`, `ArgumentsUnreadable`
and `RunsSpent`. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_program.rs` does not compile.

## Acceptance

Each with a fixture protocol of two artifacts and stub programs, no model:

- Exit 0 with `results: {"0": valid, "*": invalid}` records `valid`; exit 3 records `invalid`; a
  timeout looks up the key `timeout`, falls to `*`, and with neither is refused
  `ResultNotInTable`.
- `from: /status` reads the member `status` of the JSON report; a report that is not one JSON
  object, is larger than 1 MiB, or lacks the pointer is refused `ReportUnreadable` and records
  nothing.
- An entry list with two kinds records one record per kind from one run, each from its own table.
- A program that writes its standard input and environment into its report received the
  arguments as one JSON object and an environment whose only variable is `PATH`.
- With `arguments_from`, the program receives the object the argv printed; an `arguments_from`
  program that exits non-zero or prints anything but one JSON object is refused
  `ArgumentsUnreadable` and the bound program does not start.
- The record's subject revision is the revision the case holds for the subject, its instant is
  the observation's, its producer is the binding's; a `producer`, `observed_at` or
  `subject_revision` member in the program's report changes nothing.
- The record cites the observation, and the observation names the blob's SHA-256; the blob is
  the file `blobs/<sha256>` in the store directory, written by `crates/loom-bindings` through a
  temporary file and a rename, and holds standard output followed by standard error. Two runs
  with the same output leave one file. `crates/loom-governor` is not changed by this story.
- An action that has already run its `budget.max_runs` times in this case is not performed and is
  reported `RunsSpent`. The count is the number of observations the case's store holds for that
  action from this effect port; it survives a reopen of the store. The action this check drives in
  `crates/loom-bindings/tests/bound_program.rs` is a writer (its effect class is `write`), so the
  check stays green when `story:run-caps-per-arrival` exempts observations.
- The effect port reports `performs` only for actions bound to `program`.
- `task check` exits 0.

## Out of scope

Moving artifact revisions after the program ran (`story:bound-artifact-revisions`).
Counting runs per arrival, and the rule that an observation (an action whose effect class is not
`write`) is never refused `RunsSpent` and may state no `max_runs`: both land in
`story:run-caps-per-arrival`, which depends on this story. Agent, person, open and copy executors.
Report checks that name a tool (a spec digest, a clean tree).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (new crate, created by `story:bindings-document`): the `program` effect port, the result-table lookup, the blob and the run count. Cited: the story's Outcome names the crate.
- **Files:** `crates/loom-bindings/tests/bound_program.rs`. Cited: ESS first.
- **Files:** `ess/domains/bindings.yaml`, which gains `ProgramRun`, `RecordedEvidence`, `ResultNotInTable`, `ReportUnreadable`, `ArgumentsUnreadable` and `RunsSpent`. Cited: ESS first.
- **Files:** `crates/loom-bindings/Cargo.toml` gains path dependencies on `b10x-loom-intake-slice` (runner), `b10x-loom-commission` (`EffectPort`, `submit_evidence`) and `b10x-loom-governor` (`CanonGovernor::revisions`, `try_observations`), plus `sha2`. Inferred: the story names no manifest.
- **Symbols:** `EffectPort` (`crates/loom-commission/src/ports/effect.rs:39`), `submit_evidence` (`crates/loom-commission/src/ports/evidence.rs:125`), `SubstrateRunner::run_child` (`crates/loom-intake-slice/src/confinement.rs:475`), `ChildCommand` (`confinement.rs:1037`). Cited: all four are used and none is changed.
- **Also likely:** none. `crates/loom-intake-slice`, `crates/loom-commission` and `crates/loom-governor` are used, not changed. Inferred: every item needed is `pub` today (`loom-intake-slice/src/lib.rs:32`, `confinement.rs:77,126,475,1037`; `clock.rs:64,75`; `loom-governor/src/lib.rs:1094,1128,1414,1436`). Only `fresh_uuid` is `pub(crate)` (`executor.rs:1645`), and the crate can write its own.
- **Documents:** none. The acceptance requires no guide page, and the bindings document format gains no members in this story. Inferred.
- **Confidence:** high. The story names the crate, the test file and the ESS file. The one open question is where the blob is kept, and it does not change the primary surface.
- **Would collide with:** any unit changing `crates/loom-bindings` or `ess/domains/bindings.yaml`, and any unit changing the public runner API in `crates/loom-intake-slice/src/confinement.rs` (`ChildCommand`, `run_child`, `TestExecution`). Inferred for the runner API. If the blob or the run count is put into the governor's store, it would also collide with units changing `crates/loom-governor/src/lib.rs` or `file_store.rs`. Inferred.
- **Safety fact:** the story needs no change to `crates/loom-governor`. The run count can be read from `CanonGovernor::try_observations` (`crates/loom-governor/src/lib.rs:1128`), and `FileCaseStore` keeps observations on disk (`file_store.rs:296-305`), so the count survives a reopen. Observations are kept for the whole store, not per case (`ObservationData` has no case id: `generated/rust/commission/src/responsibility.rs:2290-2301`), so the port must put the case and action in the payload and filter on its own `source`. Proven to step 2, unproven.
