---
format: aep.planning-md/3
id: story:gate-run-collects-a-gates-document
kind: story
status: draft
title: An engineering.gates action reads the gates at the base, collects every request through Loom's collectors and records the result through its table
summary: Base read, project limits, preflight, admission per command, the process value, 300 s budget, freshness, source unchanged, story context from the host; replaces takt's GateRun
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:gates-document-evaluated-in-the-governor
- depends_on: story:observe-git-changed
scope:
- confidence: inferred
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/gates.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/git.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/process.rs
- confidence: cited
  path: crates/loom-bindings/tests/fixtures/gates
- confidence: cited
  path: crates/loom-bindings/tests/gate_run.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_catalog.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 8
---
## Outcome

An action bound to `observe` with operation `engineering.gates` (`document`, optional `project`,
workspace paths) runs a repository's gates as takt's gate validator did, on Loom. It needs the
observe port's launcher: a port built with a reader and no launcher, for a document that binds
`engineering.gates`, returns `ObservePortRefused` with `refusal` `NoLauncher` at port build, as for
`git.changed` (`story:observe-git-changed`).

1. It resolves the change's base revision (`ObserveContext.change.base`) once, with `rev-parse
   --verify --quiet --end-of-options <base>^{commit}`, and reads the gates document and the
   `engineering-assertions-project/1` file at that commit with `git cat-file blob
   <commit>:<path>`, all through the one launch function of `crates/loom-bindings/src/observe/git.rs`.
   Each input is at most 1 MiB, the launch's output bound (Substrate's output limit at `0.21.0`,
   `crates/substrate-host/src/lib.rs:528`), so the change under test cannot weaken its own gates.
   No base in the context makes the run `Unavailable`.
2. It checks the project: `timeout_seconds` 1 to 300, `max_output_bytes` 1 to 16 MiB,
   `freshness_seconds` 1 to 86,400, at most 1,024 commands; `catalogs`, `providers` and `networks`
   are not served. A command is named `command:<name>` in the bundle, as upstream names it.
3. The governor plans the document: `loom_governor::gates::plan(document, variables, identity)`
   (`story:gates-document-evaluated-in-the-governor`), where `identity.source_identity` is the
   sha256 of the base commit's id and the two blobs' ids, and `identity.context_identity` the
   sha256 of the canonical JSON of `ObserveContext.change`. A document `plan` refuses (another
   format, a gate without assertions, an expression that does not compile, an operation the catalog
   does not hold) refuses the run `GatesRefused`, with the `GatesRefusal` in
   `ObserveRun.gates_refusal`. Before anything is collected, every request is checked: its operation
   is one this epic collects (`net.*` is not, until stories 9 and 10), every `binding` names a
   command the project declares, every `glob` is valid, and every project command is admitted. A
   request that fails refuses the run naming it, and nothing runs.
4. Each request is collected by the collector of the same name: an `ObserveOperation` (stories 1, 3
   to 6), or, for `process.run`, `process.succeeded`, `ess.run` and `tests.run`, the project's
   command, launched through the observe port's `ObserveLauncher`
   (`crates/loom-bindings/src/observe/launch.rs`) with exactly the environment the command
   declares, no network, the workspace writable only under `target/`, and for `tests.run` the
   `engineering-tests/1` checks of `story:report-formats-checked-before-the-table`. `story.id` and
   `story.has_tag` are answered from the host's `ObserveContext.change` (`story_id`, `story_tags`),
   never collected; absent, they are `Unavailable`. Each request's timeout is the project's
   `timeout_seconds` cut to what is left of the total budget, 300 s by default; the port's builder
   takes the budget (`collection_budget`) and the clock it is measured on (`clock`), so a test sets
   3 s and a fake clock its fake launcher advances. Each observation carries `observed_at` (when its
   collection ended) and `valid_until` (`observed_at` plus the project's `freshness_seconds`).
5. Every file whose sha256 the collection read, and every relative `executable:`, is read again
   after the last request; one that changed refuses the run (`SourceChanged`). This is narrower
   than upstream, which digests the whole workspace (`source_identity_with_inputs`,
   `crates/canon-engineering-assertions/src/lib.rs:679-686`): a command that rewrites a file no
   request read is not seen (§ Open of the ledger).
6. The governor evaluates the plan with `gates::evaluate(plan, observations, context)`, `context`
   the trusted time and the same two identities; observations are built as
   `loom_governor::gates::Observation`, so this crate never names `canon_expr`. The report is
   `{"operation": "engineering.gates", "outcome": "known", "value": {"result": <aggregate>,
   "gates": [{"id", "result", "reasons"}]}}`, and the entries read `from: /value/result` (or
   `/value/gates/<n>/result`) through the table. A `GatesRefusal` `EvaluationFailed` refuses the
   run `GatesRefused`.

The gates evaluation runs in `crates/loom-governor`, but this action calls it from the acting path:
`loom_governor::gates::plan` and `evaluate` are plain functions the observe port calls, and their
result reaches the governor's protocol evaluation only as the evidence records the binding's table
writes (§ Open 7 of the ledger, for session `canon`).

Each project command goes through the admission unit of `story:gate-program-identity-bound-to-spawn`
(`crates/loom-bindings/src/admission.rs`), all three parts:

- `admit`, at preflight, with the case's workspace root: a command naming a `program` on the
  project's `search_path` when it has entries, else on the host's search path, as upstream
  (`crates/assertion-providers/src/process.rs:266-296`); a command naming an absolute
  `executable:` as that path. The unit applies its search-path checks to the project's
  `search_path` as to the host's.
- `launch`, for each request: the admitted real file mounted read-only at its mount path, ending in
  the program's name, and run as `argv[0]`; so a link retargeted after preflight cannot change what
  runs, and a link keeps its name.
- `verify`, after each launch: a rewritten program refuses the run `ProgramChanged`.

A command naming a relative `executable:` is a workspace path: Loom reads its sha256 through the
`WorkspaceReader`, runs it as written from `/workspace`, and re-reads it for `SourceChanged`.

The process value, which this story owns for `process.run`, `process.succeeded` and `ess.run`, is
engineering-protocols' (`crates/assertion-providers/src/process.rs:22-39`, `src/lib.rs:182-191`, tag
`0.16.0`), mapped from the launcher's `TestExecution` (`crates/loom-intake-slice/src/confinement.rs:78-90`):

- exited with code `n`, output whole and UTF-8: `process.run` and `ess.run` are `{"exit_code": n,
  "stdout": <text>, "stderr": <text>}`; `process.succeeded` is `n == 0`;
- timed out, or standard output or error cut at the lesser of the project's `max_output_bytes`
  and Substrate's output limit (1 MiB at `0.21.0`): all four are `Unavailable`, `process timed out
  or exceeded output limit`;
- standard output or error that is not UTF-8: the run is refused `NotUtf8`, naming the request, as
  upstream errors (`process.rs:36`);
- ended by a signal: upstream gives `"exit_code": null` and `process.succeeded` `Unavailable`, but
  Loom's runner refuses a child that ends with no exit status (`confinement.rs:947-952`), so the run
  is refused `LaunchRefused`. Not carried until the runner reports the signal (§ Open of the
  ledger).

The plan, the observations and the admitted programs are kept as one `GateBundle` blob that
`ObserveRun.bundle_sha256` names: the plan itself, the two input blobs' sha256, the identity, each
observation, and each program, so the run can be evaluated again without collecting.

## Why

takt's `GateRun` (takt `25c62df`, `crates/takt/src/validator.rs:2482`, read by a helper and
spot-checked) reads the document and project from the pinned base (`:5185-5262`), calls
engineering-protocols' `prepare_with`, `collect` and `evaluate_gates`
(`crates/canon-engineering-assertions/src/lib.rs:348-580`, `:623-697`, `:780-846`, tag `0.16.0`),
and writes one `gate_result` record, `pass` or `fail`, none when unknown (`validator.rs:5585-5626`).
The library starts the gate's commands itself, outside takt's confinement (helper:
`validator.rs:5444-5465`). This story keeps what the library guarantees and moves every process
into Substrate:

- preflight before any collection (`lib.rs:523-566`;
  `all_command_operations_are_preflighted_before_collection` in
  `crates/canon-engineering-assertions/tests/adversary.rs:73`; the TCP host part,
  `tcp_host_admission_is_preflighted_before_any_collection`, `adversary.rs:130`, is
  `story:tcp-reachability-through-an-aperture`'s);
- a total collection budget of 300 s and each request's timeout cut to what is left
  (`lib.rs:625-645`);
- admitted programs checked around every request (`lib.rs:631`, `:647`;
  `changed_admitted_command_executable_cannot_reuse_prepared_identity`, `adversary.rs:94`);
- `valid_until` from `freshness_seconds` and the completion time (`lib.rs:648`;
  `live_run_uses_completion_time_for_freshness`, `collection_clock.rs:3`);
- the source unchanged during collection (`lib.rs:679-686`);
- project limits (`lib.rs:371-388`);
- the gates and project files are never retained evidence paths (`lib.rs:356-360`; here: they are
  read from git, never from the workspace).

The crate direction holds. `story:bound-program-result-table` already adds `b10x-loom-bindings` →
`b10x-loom-governor` (its scope's `Files:` line for the manifest, `bound-program-result-table.md:115`,
marked inferred there); this story adds no manifest line. `AGENTS.md` § Governor says the governor depends on Commission and on the executor, never the
reverse (`:438-442`, `:458-459`), which forbids nothing depending on the governor;
`b10x-loom-intake-slice` already does (`crates/loom-intake-slice/Cargo.toml:19`); the governor's
dependencies are `b10x-loom-protocols`, `b10x-canon`, `b10x-loom-commission`, `b10x-loom-executor`,
`loom`, `b10x-canon-engineering`, `serde_yaml_ng` and `sha2` (`crates/loom-governor/Cargo.toml:14-21`),
none of which depends on `b10x-loom-bindings`, so no cycle; and `AGENTS.md` § Never restricts only
`b10x-loom-commission`. So no host-wired port is needed.

## Waits on

- `story:gates-document-evaluated-in-the-governor`: plan and evaluation; both edit
  `generated/rust/loom/` and `CHANGELOG.md`.
- `story:observe-git-changed`: the launcher, the git helper function, `ess/domains/bindings.yaml`,
  `ess/domains/observation.yaml`, `observe.rs`, and through it every earlier observe operation and
  `story:gate-program-identity-bound-to-spawn`'s admission unit.

Waiting on this one: `story:tcp-reachability-through-an-aperture` (`observe/gates.rs`,
`observe_catalog.rs`, the ESS files), and through it `story:dns-resolution-through-a-resolver-aperture`.

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `EngineeringGates` (spelled
`engineering.gates`; `decode.rs` maps the spelling, `load.rs` checks the arguments `document` and
optional `project` as workspace paths). `ess/domains/observation.yaml`:

- `ChangeContext` gains optional `story_id` and `story_tags`;
- `ObserveRefusal` gains `GatesInputUnreadable`, `GatesRefused`, `RequestNotCollectable`,
  `CommandNotDeclared`, `ProjectInvalid`, `ExtensionNotServed`, `SourceChanged`,
  `ProgramNotAdmitted` and `ExitContradictsReport` (a gate command's `tests.run`; story 5 gives the
  others of its checks to `ObserveRefusal` already);
- `ObserveRun` gains `program_refusal` (`Optional<loom.bindings.ProgramRefusal>`), `gates_refusal`
  (`Optional<loom.gates.GatesRefusal>`) and `bundle_sha256` (optional);
- `ProcessValue` (`exit_code` optional integer, `stdout`, `stderr`);
- `GateObservation` (`request_id`, `operation`, `outcome`, `value` optional `Json`, `reason`
  optional, `observed_at`, `valid_until`), `GateProgram` (`binding`, `program`
  `loom.bindings.AdmittedProgram`, or `path` and `sha256` for a workspace path), and `GateBundle`
  (`plan` `Json`, `document_sha256`, `project_sha256` optional, `identity`
  `loom.gates.GatesIdentity`, `observations`, `programs`).

No `BindingsRefusalReason` is added. On that commit `task drift` fails and
`crates/loom-bindings/tests/gate_run.rs` does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record, the table reading the gates
report. Two runs over the same base, workspace and programs plan the same requests and collect the
same values, but each observation's `observed_at` and `valid_until` are that run's, so the kept
bundle differs; the records are equal but for `id` and `observed_at`.

## Acceptance

Tests in `crates/loom-bindings/tests/gate_run.rs`, over repositories the test builds with a gates
document committed at the base; the launches are delegated cases as in story 6, and the cases named
"fake" run on a recording fake `ObserveLauncher`, a fake reader and a fake clock. "Bundle" is the
`GateBundle` that `ObserveRun.bundle_sha256` names.

"Refused X before commands": the gate run's `ObserveRun.outcome` `Refused`, `ObserveRun.refusal` X,
the step refused naming it, no evidence record, and a marker file the first command would write
does not exist. "Refused X after commands": the same without the marker condition (the refusal
comes after a command ran). A table refusal (`ResultNotInTable`) is the step's refusal of
`story:bound-program-result-table`, with `ObserveRun.outcome` unchanged.

Results:

- `the_release_notes_gate_reaches_its_table_row`: the release-notes fixture of engineering-protocols'
  epic `gates-and-delivery-for-any-repository` (a gate with `applies_when:
  'story.has_tag("consumer-visible")'` and assertion `git.changed("CHANGELOG.md")`) gives the
  record's `result` for `not_applicable` with a story without the tag, for `fail` with a tagged
  story and no changelog change, and for `pass` with one.
- `every_upstream_gates_fixture_keeps_its_result`: `fixtures/assertions/gates.yaml`,
  `git-changed.fixture.yaml`, `applies-when.fixture.yaml` and `program-by-name.fixture.yaml` at
  `0.16.0`, copied to `crates/loom-bindings/tests/fixtures/gates/` with their sha256 named in the
  test, each case run as a gate run: the report's `/value/result` and per-gate results equal the
  case's `expect` (for `gates.yaml`, `pass` over a workspace with `Cargo.toml` and a README holding
  `Apache-2.0`, as engineering-protocols' `assertion_replay.rs:17-22` writes it), and for
  `program-by-name` the bundle's `programs` entry has the case's `binding` (`command:tool`),
  `program` and `path`.
- `a_gate_edited_in_the_subject_runs_as_the_base_has_it`: the report's gate result is the base
  version's.
- `the_gate_result_reaches_the_protocol_only_as_evidence`: over a governed case, the governor's
  observations after the run (`CanonGovernor::try_observations`) differ from those before by
  exactly the records the table wrote, and no `GatesReport` or bundle is among them.
- A gate `unknown` maps through the table: the run's `ObserveRun.outcome` is `Known` and the record
  is the table's row for `unknown`.
- `the_bundle_replays`: `gates::evaluate` over the bundle's `plan` and `observations`, with an
  `EvalContext` of the same `now` and the bundle's `identity`, gives the same report bytes.

Port build and inputs:

- `a_reader_only_port_refuses_a_gate_run`: a port built with no launcher for a document binding
  `engineering.gates` returns `ObservePortRefused.refusal` `NoLauncher`, and no `ObserveRun` exists.
- No `base` in `ObserveContext.change`: `ObserveRun.outcome` `Unavailable`, `reason` `no base
  revision`, and the table's `unavailable` row.
- A `base` git cannot resolve: `Unavailable`, `reason` naming `rev-parse` and its exit code (story
  6's rule).
- A gates document larger than 1 MiB at the base: refused `GatesInputUnreadable` before commands,
  `reason` naming the bound.
- A gates path absent at the base, or bytes that are not YAML: refused `GatesInputUnreadable` before
  commands, naming the path.
- A gates document present in the workspace but not at the base: refused `GatesInputUnreadable`
  before commands.
- A gates document with `format: engineering-gates/2`: refused `GatesRefused` before commands,
  `ObserveRun.gates_refusal` `UnsupportedFormat`.
- A gates document whose assertion names `nosuch.op("x")`: refused `GatesRefused` before commands,
  `gates_refusal` `CompileFailed`.

The project:

- `project_limits_are_checked` (one case per bound): `timeout_seconds` 0 or 301,
  `max_output_bytes` 0 or 16 MiB + 1, `freshness_seconds` 0 or 86,401, and 1,025 commands are each
  refused `ProjectInvalid` before commands, `reason` naming the member.
- A project with `catalogs`, `providers` or `networks`: refused `ExtensionNotServed` before
  commands, naming the member.

Preflight:

- A plan with a `net.tcp.reachable` or `net.host.resolves` request: refused `RequestNotCollectable`
  before commands, naming the request.
- A `binding` the project does not declare: refused `CommandNotDeclared` before commands.
- A `git.changed` request whose glob is invalid (an empty segment): refused `GlobInvalid` before
  commands.
- A command whose program is not on the search path: refused `ProgramNotAdmitted` before commands,
  `ObserveRun.program_refusal` `ProgramNotFound`.
- A project `search_path` entry inside the workspace: refused `ProgramNotAdmitted` before commands,
  `program_refusal` `SearchPathInvalid`.

Admission, launch and verify for a gate command:

- `the_projects_search_path_replaces_the_hosts`: with `tool` on both, the bundle's `programs` entry
  has the project entry's path, and the output is that file's.
- `a_runtime_default_search_path_is_used` (engineering-protocols' `process_program.rs` case): with
  no project `search_path`, the entry has the host search path's file.
- `an_executable_path_is_admitted_and_recorded`: a command with an absolute `executable:` runs that
  file, and the bundle's entry names its real path and sha256.
- `a_gate_command_link_swapped_after_preflight_runs_the_admitted_program`: a command whose program
  is a link on the project's `search_path`, retargeted after preflight and before its request, runs
  the file admitted at preflight (its output is that file's), and the bundle names that path and
  sha256.
- `a_gate_command_link_runs_under_its_name`: a program that prints its `argv[0]`, reached by a link
  named `tool`, prints a path whose last component is `tool`.
- `a_gate_command_gets_only_its_program`: the delegated case's applied confinement lists the
  admitted program as the child's one read-only file, no network, and no writable path but
  `target/`.
- `a_gate_command_has_only_its_declared_environment`: the `ChildCommand` the recording launcher
  received has exactly the command's declared environment as its `environment()`.
- A relative `executable:`: the bundle's entry names the workspace path and its sha256.
- A relative `executable:` rewritten by an earlier request: refused `SourceChanged` after commands.
- A program rewritten after admission, between two requests: refused `ProgramChanged` after
  commands.

Process values (fake launcher, each read from the bundle's observation for the request):

- Exit 0 with output `ok`: `process.run`'s `value` is `{"exit_code": 0, "stdout": "ok", "stderr":
  ""}`, and `process.succeeded`'s `value` is `true`.
- Exit 3: `process.succeeded`'s `value` is `false`, and `process.run`'s `value.exit_code` is `3`.
- A launch the runner refuses for ending with no exit status (the signal case): refused
  `LaunchRefused` after commands.
- Timed out: `outcome` `Unavailable`, `reason` `process timed out or exceeded output limit`, for
  `process.run`, `process.succeeded` and `ess.run`.
- Standard output cut at the bound (project `max_output_bytes` 16 bytes, 17 printed): `outcome`
  `Unavailable`, the same reason.
- Standard error that is not UTF-8: refused `NotUtf8` after commands, naming the request.
- `ess.run` exiting 0 is the same object as `process.run`; no report format is applied to it.
- `tests.run` whose command exits 0 with a report holding `failed: 1`: refused
  `ExitContradictsReport` after commands.
- `tests.run` whose report lists one test twice: refused `ReportInconsistent` after commands.
- A delegated case: a command printing `ok` and exiting 0 gives the same `process.run` value as the
  fake.

Clock, budget and freshness (fake):

- `each_requests_timeout_is_cut_to_what_is_left`: with `collection_budget` 3 s and a first command
  that advances the fake clock by 2 s, the launcher records a second launch whose timeout is at
  most 1 s.
- A command still running when the budget is spent: `ObserveRun.outcome` `Unavailable`, `reason`
  `collection budget`.
- `live_run_uses_completion_time_for_freshness`: with `freshness_seconds` 60, each bundle
  observation's `valid_until` is its `observed_at` plus 60 s, and `observed_at` is the fake clock's
  time when its launch ended.
- `story.id()` with `story_id` `story-a` in the context: the bundle observation's `value` is
  `"story-a"`.
- `story.id()` and `story.has_tag(...)` with no story in the context: each bundle observation's
  `outcome` is `Unavailable`, and the gate that uses it is `unknown` in the report.

Source:

- A workspace file read by `text.contains` and rewritten by a later `process.run`: refused
  `SourceChanged` after commands, naming the path.
- A workspace file no request read, rewritten by a `process.run`: the run's `ObserveRun.outcome` is
  `Known` (the narrowing above).

The catalog:

- `observe_operations_cover_the_catalog` (`crates/loom-bindings/tests/observe_catalog.rs`) holds
  two lists. The catalog's: each of the 20 observation functions of `catalogs/engineering.yaml`
  (18 operations, `document.get` behind `doc.string`, `doc.integer` and `doc.boolean`) resolves to
  an `ObserveOperation`, a project command (`process.run`, `process.succeeded`, `ess.run`,
  `tests.run`), host context (`story.id`, `story.has_tag`) or an operation refused by name
  (`net.host.resolves`, `net.tcp.reachable`, until stories 9 and 10), and each of the 6 recipes
  plans through `gates::plan` with no operation of its own; a function added to the catalog fails
  it. The upstream list's: each of the 20 names of `assertion_providers::OPERATIONS` at `0.16.0`
  (`src/lib.rs:120-141`, written into the test with the tag) resolves the same way, `json.read` and
  `yaml.read` to `ObserveOperation` `JsonRead` and `YamlRead`.

The rest:

- `crates/loom-intake-slice/tests/host_git_hardening.rs::every_host_git_command_goes_through_a_hardened_helper`
  passes, run and not changed: `rev-parse` and `cat-file` add no git start to `observe/git.rs`.
- `canon_expr_is_the_governors_alone` (story 7) passes: no file under `crates/loom-bindings`
  names `canon_expr`.
- `CHANGELOG.md` `### Added`: `engineering.gates`; `### Changed`: `**Breaking:**` for the new enum
  variants.
- `website/docs/guides/bindings.md` shows a gate-run binding in prose, outside the page's
  `format: loom.bindings/1` file, so the guide test's counts are unchanged.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs` (the dispatch, the port builder's `collection_budget` and
`clock`, `NoLauncher` for `engineering.gates`), `crates/loom-bindings/src/observe/gates.rs` (new),
`crates/loom-bindings/src/observe/process.rs` (new, the process value),
`crates/loom-bindings/src/observe/git.rs` (`rev-parse` of the base and `cat-file` through the
existing launch function), `crates/loom-bindings/src/decode.rs` (the spelling),
`crates/loom-bindings/src/load.rs` (the arguments), `crates/loom-bindings/src/observe/launch.rs`
(called, not changed), `crates/loom-bindings/src/admission.rs` (called, not changed: `admit`,
`launch`, `verify`), `crates/loom-bindings/tests/gate_run.rs` (new),
`crates/loom-bindings/tests/observe_catalog.rs` (new), `crates/loom-bindings/tests/fixtures/gates/`
(four copied fixtures), `website/docs/guides/bindings.md`, `CHANGELOG.md`. No manifest line: the
governor dependency is `story:bound-program-result-table`'s, and `serde_json`, `sha2` and
`tempfile` are stories 1 and 6's.

## Out of scope

Skipping the requests of a `not_applicable` gate (collected anyway, as upstream: its story's
"Open"). Network requests and a project's `networks` (stories 9, 10). Extension catalogs and
external providers. A replay command. Gate inputs over 1 MiB: refused until the driver's and the
output limit are raised (§ Open of the ledger). A whole-workspace source digest (step 5). The
signal case of the process value (§ Open of the ledger).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings/src/observe/` (the `engineering.gates` collector) — cited, story § Files
- **Files:** `crates/loom-bindings/src/observe.rs`, `observe/gates.rs` (new), `observe/process.rs` (new), `observe/git.rs` (`cat-file` through its one launch function) — cited, story § Files
- **Files (ESS):** `ess/domains/bindings.yaml` (`ObserveOperation::EngineeringGates`), `ess/domains/observation.yaml` (new types and refusals) — cited, story § ESS first
- **Files (tests):** `crates/loom-bindings/tests/gate_run.rs` (new), `tests/observe_catalog.rs` (new), `tests/fixtures/gates/` (four copied fixtures) — cited, story § Files
- **Files (docs):** `website/docs/guides/bindings.md` — cited, story § Files and Acceptance
- **Called, not changed:** `observe/launch.rs`, `crates/loom-bindings/src/admission.rs`, `crates/loom-governor` `gates::{plan, evaluate}` — cited, story § Files / Outcome
- **Symbols:** `ObserveOperation::EngineeringGates`, `ChangeContext.{story_id, story_tags}`, `ObserveRefusal::{GatesInputUnreadable, RequestNotCollectable, CommandNotDeclared, ProjectInvalid, ExtensionNotServed, SourceChanged, ProgramNotAdmitted, NoExitCode}`, `ObserveRun.{program_refusal, bundle_sha256}`, `ProcessValue`, `GateObservation`, `GateProgram`, `GateBundle` — cited, story § ESS first
- **Symbols (existing):** `SubstrateRunner::run_child` `crates/loom-intake-slice/src/confinement.rs:475`; `ChildExit` and `ChildOutput` re-exported at `confinement.rs:19-22`, built at `:887-894`; `CanonGovernor::try_observations` `crates/loom-governor/src/lib.rs:1131` — cited
- **Also likely:** `crates/loom-bindings/Cargo.toml` — inferred, only if `serde_json` or similar is needed for `ProcessValue`; the story says it adds no manifest line
- **Documents:** `website/docs/guides/bindings.md`, `CHANGELOG.md`; no `workflows/`, `protocols/` or `principles/` file — cited
- **Confidence:** high for the file set (the story lists every file and the observe chain it waits on names the same ones). Low for any symbol check against code: `observe.rs`, `ObserveLauncher`, `admission::admit`, `gates::plan` and `evaluate` exist in no tree yet.
- **Would collide with (observe surface):** the stories below land on `observe.rs`, `observe/git.rs`, `ess/domains/observation.yaml` or `ess/domains/bindings.yaml`. All are ancestors in `depends_on`, so already ordered, and none carries a scope entry yet.
  - `story:observe-executor-reads-workspace-files`, `story:gate-program-identity-bound-to-spawn`, `story:observe-reads-documents`, `story:observe-compares-generated-trees`, `story:report-formats-checked-before-the-table`, `story:observe-git-changed` — all ancestors via `observe-git-changed`.
  - `story:bound-program-result-table` (scope has `ess/domains/bindings.yaml`) — ancestor, ordered.
- **Would collide with (observe descendants):** `story:tcp-reachability-through-an-aperture` (direct `depends_on` on this story) and `story:dns-resolution-through-a-resolver-aperture` (via tcp). Both name `observe/gates.rs`, `tests/gate_run.rs`, `tests/observe_catalog.rs` and the ESS files; ordered.
- **Would collide with (bindings ESS and guide, NO `depends_on` path either way):** same file and generated crate `generated/rust/loom/src/bindings.rs`, so a merge conflict if they run in one wave.
  - Scope entries on `ess/domains/bindings.yaml`: `story:bound-artifact-revisions`, `story:bound-agent-node`, `story:agent-node-run-records`, `story:record-copy-between-cases`, `story:run-caps-per-arrival`, `story:tree-usage-and-model-budget`, `story:concurrent-cases-in-the-tree-daemon`, `story:child-case-own-workspace`, `story:capacity-given-to-an-admission-program`.
  - Bodies only, no scope entry yet: `story:autonomy-grant-table`, `story:action-limits-enforced`, `story:limit-hits-in-run-log`, `story:named-egress-per-action`, `story:confinement-level-required`, `story:engine-configuration-document`, `story:agent-session-through-substrate`.
  - On `website/docs/guides/bindings.md`: `story:capacity-given-to-an-admission-program`, `story:run-caps-per-arrival` (scope entries), both unordered. `story:bindings-document` is an ancestor.
  - On `crates/loom-bindings/src/lib.rs`: `story:run-caps-per-arrival`, `story:record-copy-between-cases`, unordered. This story's own scope does not list `lib.rs`, so this is a risk only if `observe.rs` needs a module line there.
- **Safety fact:** `git cat-file` adds no git start site: `every_host_git_command_goes_through_a_hardened_helper` (`crates/loom-intake-slice/tests/host_git_hardening.rs:444`) counts `Command::new(` / `ChildCommand::new(` with a git argument per helper file, and `observe-git-changed` already enrols `observe/git.rs` as one helper that starts git in one place — step 2, unproven
- **Safety fact (crate direction):** no dependency cycle. `loom-bindings` depends on no other Loom crate today (`crates/loom-bindings/Cargo.toml`), and no `crates/*/Cargo.toml` except its own names `b10x-loom-bindings`. The governor depends on commission and executor, intake-slice already depends on the governor (`crates/loom-intake-slice/Cargo.toml:19`) — step 2, unproven
