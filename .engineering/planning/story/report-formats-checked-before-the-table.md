---
format: aep.planning-md/3
id: story:report-formats-checked-before-the-table
kind: story
status: draft
title: A test, ESS conformance or Codegate report is checked and normalized before a result table reads it
summary: report_format on a program (tests.run) and tests.report, ess.report, codegate.dependencies on a file; an inconsistent report is refused on ProgramRun or ObserveRun
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-compares-generated-trees
- depends_on: story:bound-program-result-table
scope:
- confidence: inferred
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/reports.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/src/reports.rs
- confidence: cited
  path: crates/loom-bindings/tests/fixtures/
- confidence: cited
  path: crates/loom-bindings/tests/observe_files.rs
- confidence: cited
  path: crates/loom-bindings/tests/report_formats.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
revision: 8
---
## Outcome

Three report formats are checked and normalized before a result table reads them, wherever the
report comes from:

- `engineering-tests/1` → `{inventory, passed, failed, skipped, total, executed}`;
- `ess-conformance-report/2` → `{passed, failed, unsupported, errors, skipped, total, inventory,
  conformance_status, spec_digest, implementation}`;
- `codegate-dependency-report/0.1` → `{verdict, coverage, findings_count, fan_out_max, source_id,
  configuration_id}`.

From a program: a `program` executor may name `report_format`, and its standard-output report is
checked and replaced by the normalized object before the entries' pointers read it. This is how
`tests.run` is collected (with `engineering-tests/1`, as engineering-protocols reads it), and a
program printing either other format may name it too. For `engineering-tests/1` the exit status is
checked against the report: exit 0 with `failed > 0`, or a non-zero exit with `failed == 0`, is
refused. `ess.run` takes no report format: its value is the process value of
`story:gate-run-collects-a-gates-document`.

From a file: three `observe` operations, `tests.report`, `ess.report` and `codegate.dependencies`
(`path`), read the file through story 1's reader, with no process and under its 1 MiB per-file
bound, and return the normalized object as the report's `value`.

The checks run in upstream's order: the `format` string first, then for `engineering-tests/1`
from a program the exit status against the report, then the counts and the inventory
(`reports.rs:56-63` and on), so a report that both contradicts its exit and has a duplicate name is
`ExitContradictsReport`. A Codegate report spells its verdict on the wire as `Pass`, `Fail`,
`Incomplete`, `Unsupported` or `Error`, and the normalized object lowercases it (`reports.rs:97-107`,
`:155`): a table reads `/verdict` with the keys `pass` and `fail`.

A file that is not UTF-8 is story 1's `NotUtf8` before any check; a UTF-8 file that is not one JSON
object is `ReportInconsistent`; a program's standard output that is not one JSON object is the
table refusal `ReportUnreadable` of `story:bound-program-result-table`. The two paths differ on
purpose: the program path keeps the refusal the result table already gives every program report.

Where each outcome is recorded (the rule of the epic, § Refusals): from a file, in
`ObserveRun.outcome` and `ObserveRun.refusal`; from a program, in `ProgramRun.refusal` and the new
`ProgramRun.unavailable`, which this story adds to the record `story:bound-program-result-table`
declares and `story:gate-program-identity-bound-to-spawn` extends; `ReportUnreadable` and
`ResultNotInTable` stay that story's table refusals, the step's refusal with no `ProgramRun`
member.

## Why

engineering-protocols checks these formats before any value is used
(`crates/assertion-providers/src/reports.rs:29-160`, `:162-284`, tag `0.16.0`): an inventory with
duplicate or empty names, counts that do not add up, zero executed tests, a "passed" ESS report that
is not complete, or a Codegate pass with findings are errors, never a result; a Codegate verdict
other than `pass` or `fail` is unavailable (`:149-153`). Only `tests.run` reads a program's report
there (`reports.rs:34`, its format fixed to `engineering-tests/1` at `:51-57`); `ess.run` is plain
process collection (`src/lib.rs:182`). `story:bound-program-result-table` reads a JSON pointer into
a program's report with no such check, so `from: /failed` over a report that lists the same test
twice would record what the report claims. takt's `TestsRun` parses printed test output instead
(takt `25c62df`, `crates/takt/src/validator.rs:511-545`, read by a helper); this story gives a test
program a checked report to print.

## Waits on

- `story:observe-compares-generated-trees`: shared `ObserveOperation`, `observe.rs` dispatch,
  `ess/domains/observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md`.
- `story:bound-program-result-table`: the program report and its pointer lookup, which this story
  puts the check in front of, and `ProgramRun`.
- `story:gate-program-identity-bound-to-spawn`, through `story:observe-reads-documents` and
  `story:observe-compares-generated-trees` (which do not edit `program.rs` themselves; the edge
  path orders the two): this story and story 2 both edit `crates/loom-bindings/src/program.rs`,
  `ess/domains/bindings.yaml` (`ProgramRun`), `generated/rust/loom/` and `CHANGELOG.md`.

Waiting on this one: `story:observe-git-changed` (direct), and, through it,
`story:gates-document-evaluated-in-the-governor`, `story:gate-run-collects-a-gates-document`,
`story:tcp-reachability-through-an-aperture` and `story:dns-resolution-through-a-resolver-aperture`
(`generated/rust/loom/`, `CHANGELOG.md`, and for the last three the ESS files and `observe.rs`).

## ESS first

`ess/domains/bindings.yaml`:

- enum `ReportFormat` (`EngineeringTests1`, `EssConformanceReport2`, `CodegateDependencyReport01`,
  spelled as their `format` strings);
- `ExecutorSpec` gains `report_format` (`Optional<loom.bindings.ReportFormat>`, a `Program`
  member), and the `ExecutorSpec` comment says so, as story 1's says `operation` belongs to
  `Observe`;
- `ObserveOperation` gains `TestsReport`, `EssReport`, `CodegateDependencies`;
- `ProgramRunRefusal` gains `ReportFormatUnsupported`, `ReportInconsistent`,
  `ExitContradictsReport`;
- `ProgramRun` gains `unavailable` (`Optional<String>`): the reason when the normalized report says
  the value is unavailable; such a run looks up the table's `unavailable` row, then `*`.

`ess/domains/observation.yaml`: `ObserveRefusal` gains `ReportFormatUnsupported` and
`ReportInconsistent`.

On that commit `task drift` fails and `crates/loom-bindings/tests/report_formats.rs` does not
compile. No `BindingsRefusalReason` is added, so `crates/loom-bindings/tests/bindings_document.rs`
still compiles.

## Evidence record

From a file: as `story:observe-executor-reads-workspace-files` § Evidence record. From a program:
as `story:bound-program-result-table`'s record, the pointer reading the normalized object; the raw
standard output stays in the program's blob. Normalization is a function of the report's bytes and,
for tests, the exit status, so two runs with the same output write equal records but for `id` and
`observed_at`.

## Acceptance

Tests in `crates/loom-bindings/tests/report_formats.rs`, over the two fixtures engineering-protocols
ships (`crates/assertion-providers/tests/fixtures/ess-report.json`, `codegate-report.json`, copied
with their sha256 named in the test) and written `engineering-tests/1` reports; program cases use
stub programs, as `story:bound-program-result-table`'s do. "File refused X": `ObserveRun.outcome`
`Refused`, `ObserveRun.refusal` X. "Program refused X": `ProgramRun.refusal` X. Either way the step
is refused naming X and no evidence record is written. "Table refusal X": the step's refusal X of
`story:bound-program-result-table`, with no `ProgramRun.refusal`.

- `engineering-tests/1` with three passed and one failed: the record's `result` is the table's row
  for `/failed` `1`, from a file and from a program.
- A duplicate inventory name, an empty one, counts whose sum is not the inventory's length, and zero
  executed: each file refused `ReportInconsistent` (cases of `conformance.rs`
  `empty_test_run_never_certifies…` and `test_inventory_counts_and_duplicate_names_are_checked`).
- The same four reports printed by a program with `report_format: engineering-tests/1`: each
  program refused `ReportInconsistent`.
- A program with `report_format: engineering-tests/1` that exits 0 with `failed: 1`: program refused
  `ExitContradictsReport` (`tests_run_reports_conflicting_process_status_as_error`).
- The same with exit 1 and `failed: 0`: program refused `ExitContradictsReport`.
- A program that exits 0 with `failed: 1` and a duplicate inventory name: program refused
  `ExitContradictsReport` (the exit check runs first).
- `ess-conformance-report/2`: the fixture's normalized object has the ten members (the test reads
  `/conformance_status` and `/spec_digest` through the table).
- A `conformance_status` `passed` with one `failed`, unsorted or repeated scenario ids, a category
  count unequal to its list, or `policy` other than `complete-selection/1`: each file refused
  `ReportInconsistent`.
- `codegate-dependency-report/0.1`: the fixture's wire verdict `Pass` normalizes to `/verdict`
  `pass`, and the table's row `pass` is recorded.
- A wire `Pass` with a finding, a wire `Fail` without one, or a policy naming another source: each
  file refused `ReportInconsistent`.
- A wire verdict outside the five spellings (`pass` in lower case): file refused
  `ReportInconsistent`.
- A wire `Incomplete`, `Unsupported` or `Error` from a file: `ObserveRun.outcome` `Unavailable`,
  `ObserveRun.reason` naming the verdict, and the table's `unavailable` row.
- A wire `Incomplete` printed by a program with `report_format: codegate-dependency-report/0.1`:
  `ProgramRun.unavailable` naming the verdict, and the table's `unavailable` row.
- A report whose `format` is another string: file refused `ReportFormatUnsupported` naming it, and
  the same printed by a program: program refused `ReportFormatUnsupported`.
- A program's standard output that is not one JSON object under `report_format`: table refusal
  `ReportUnreadable`.
- `report_format` on an `agent`, `person`, `open` or `copy` executor: refused at load,
  `BindingsRefusalReason` `MemberNotForExecutor`.
- An absent report file: `ObserveRun.outcome` `Unavailable`, `ObserveRun.code`
  `resource.not-found` (engineering-protocols' `conformance.rs`
  `missing_report_is_unavailable_but_malformed_report_is_a_hard_error`).
- A UTF-8 report file that is not one JSON object: file refused `ReportInconsistent`; no record with
  a `false` row.
- A report file that is not UTF-8: file refused `NotUtf8`.
- A report file of 1 MiB + 1 byte: file refused `FileOverBound` (story 1's).
- A program report over the program executor's 1 MiB bound: table refusal `ReportUnreadable`.
- `no_file_or_document_operation_starts_a_process` (story 1) covers the three file operations.
- `CHANGELOG.md` `### Added`: `report_format` and the three operations; `### Changed`:
  `**Breaking:**` for the new enum variants, the `ExecutorSpec` member and the `ProgramRun` member.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/reports.rs` (new, the
three file operations), `crates/loom-bindings/src/reports.rs` (new, the three checks, shared by both
paths), `crates/loom-bindings/src/program.rs`, `crates/loom-bindings/src/decode.rs`
(`report_format` in the closed `EXECUTOR` list at `:27-40` and the `ExecutorSpec` literal at
`:353`; the three operation spellings), `crates/loom-bindings/src/load.rs` (a twelfth row of the
`fit` table at `:230-245`, `report_format` for `Program`; the three operations' `path` argument),
`crates/loom-bindings/tests/report_formats.rs` (new), `crates/loom-bindings/tests/fixtures/` (two
copied reports), `crates/loom-bindings/tests/observe_files.rs` (the no-process test's operation
list), `CHANGELOG.md`. No manifest line: `serde_json` is story 1's.

## Out of scope

Parsing printed libtest, pytest or unittest output (takt's). Binding the expected spec digest or
implementation identity: the table reads `/spec_digest` and `/implementation`, and a protocol
decides what they must be. `ess.run`'s value (`story:gate-run-collects-a-gates-document`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (checks in `src/reports.rs`, file operations in `src/observe/reports.rs`, the program path in `src/program.rs`) — cited, story § Files
- **Files:** `ess/domains/bindings.yaml` (`ReportFormat`, `ExecutorSpec.report_format`, `ObserveOperation`, `ProgramRunRefusal`, `ProgramRun.unavailable`) — cited, story § ESS first; the file and `ExecutorSpec` exist (`ess/domains/bindings.yaml:37-63`)
- **Files:** `ess/domains/observation.yaml` (`ObserveRefusal` gains two) — cited, story § ESS first; the file does not exist at `61fd500b`, `story:observe-executor-reads-workspace-files` creates it
- **Files:** `crates/loom-bindings/src/observe.rs`, `src/observe/reports.rs` (new), `src/reports.rs` (new), `src/program.rs` — cited, story § Files; `observe.rs` and `program.rs` are not in the tree yet (the upstream stories create them)
- **Files:** `crates/loom-bindings/src/load.rs` — cited, story § Files; the member-fit table is `load.rs:230-245` (`members: [(&str, bool, Executor); 11]`, `MemberNotForExecutor` at `:248`)
- **Files:** `crates/loom-bindings/tests/report_formats.rs` (new), `tests/fixtures/` (two copied reports; the directory does not exist), `tests/observe_files.rs` (the no-process test's operation list; created by `story:observe-executor-reads-workspace-files`) — cited, story § Files
- **Files:** `crates/loom-bindings/src/decode.rs` — inferred, see Story defects 1: `EXECUTOR` (`decode.rs:27-40`, a closed list of 12 member names) and the `ExecutorSpec { .. }` literal (`decode.rs:353`) must take `report_format`
- **Files:** `crates/loom-bindings/src/lib.rs` — inferred, `mod reports;` beside `mod decode; mod load; mod tree;` (`lib.rs:15-17`)
- **Files:** `crates/loom-bindings/Cargo.toml` — inferred, it holds `serde` and `serde_yaml_ng` but no `serde_json` (and no `sha2` for the fixture digest); the checks parse JSON
- **Symbols:** `ExecutorSpec`, `Executor::Program`, `ObserveOperation`, `ObserveRun`, `ObserveRefusal`, `ProgramRun`, `ProgramRunRefusal`, `BindingsRefusalReason::MemberNotForExecutor` — cited; the first and last exist now (`decode.rs:297-353`, `load.rs:248`), the rest come from upstream stories
- **Also likely:** none beyond the inferred files above
- **Documents:** none required by the acceptance; `generated/rust/loom/` and `CHANGELOG.md` are the house files — cited
- **Confidence:** high for the surface and the story's file list; medium for the exact shape of `program.rs` and `observe.rs`, which the tree does not hold yet — the story names the files and its upstream stories name the same ones
- **Would collide with:**
  - Ordered by a `depends_on` path (ancestors of this story): `story:bound-program-result-table` (creates the program port, `ProgramRun`), `story:observe-executor-reads-workspace-files` (`observe.rs`, `observation.yaml`, `observe_files.rs`), `story:observe-reads-documents`, `story:observe-compares-generated-trees` (direct edge), `story:gate-program-identity-bound-to-spawn` (`program.rs`, `ProgramRun`) — cited, graph
  - Ordered after this story: `story:observe-git-changed` (direct; `observe.rs`, `observe_files.rs`, `bindings.yaml`), `story:gate-run-collects-a-gates-document` (through git-changed; `observe.rs`, `ObserveOperation`), `story:dns-resolution-through-a-resolver-aperture` and `story:tcp-reachability-through-an-aperture` (`observe.rs`, `observe/network.rs`) — cited, graph
  - NOT ordered with it by any `depends_on` path, all on `ess/domains/bindings.yaml`, `generated/rust/loom/` and `CHANGELOG.md`: `story:bound-artifact-revisions` (draft chain to `bound-program-result-table`), `story:bound-agent-node`, `story:record-copy-between-cases`, `story:agent-node-run-records`, `story:tree-usage-and-model-budget`, `story:confinement-level-required`, `story:action-limits-enforced`, `story:named-egress-per-action`, `story:limit-hits-in-run-log`, `story:engine-configuration-document`, `story:concurrent-cases-in-the-tree-daemon`, `story:agent-session-through-substrate` — cited from their bodies
  - NOT ordered, on `ExecutorSpec` with `decode.rs` (`EXECUTOR`, the literal at `:353`) and `load.rs` (`fit`'s table): `story:child-case-own-workspace` (adds `ExecutorSpec.workspace`), `story:capacity-given-to-an-admission-program`, `story:run-caps-per-arrival` — cited from their scope lists; each edits the same array and literal, so they conflict textually
  - NOT ordered, on `load.rs`: `story:bindings-child-path-refusal-names-the-bindings-directory` (`load.rs:203`, `:286-294`; its only edge is `story:bindings-document`) — cited
  - NOT ordered, on `ProgramRun` and the program port (`program.rs`): `story:action-limits-enforced`, `story:limit-hits-in-run-log`, `story:engine-configuration-document` (each names `ProgramRun` members) — cited
  - `Cargo.toml`: `story:canon-0-9-0-pin` (active, wave 2026-11-21-w2) edits the canon pin; no `depends_on` path with this story — inferred
- **Contradicted by the code:** nothing in the story contradicts today's tree; see Story defects for the gaps
