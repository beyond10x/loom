---
format: aep.planning-md/3
id: story:gate-program-identity-bound-to-spawn
kind: story
status: proposed
title: Program admission is one unit, and a bound program runs the bytes it digested
summary: The admission unit admits (name or absolute path, search path, workspace root), launches the real file read-only under its name, and verifies it after the run; the program port, git and gate commands use all three
tags:
- sdc
relations:
- serves: vision:O1
- depends_on: story:engineering-protocols-dependency-removed
- decomposes: epic:observation-collection-as-executors
- depends_on: story:bound-commands-run-confined
- depends_on: story:bound-program-result-table
- depends_on: story:observe-executor-reads-workspace-files
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/admission.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/program_identity.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: website/docs/guides/bindings.md
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:54:58Z", actor: "agent:sdc", revision: 14, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

A bound program runs the bytes admission digested. Program admission is one unit,
`crates/loom-bindings/src/admission.rs`, and it gives three things that every caller uses
together:

- `admit`: input a program, either a bare name (no `/`) or an absolute path (a gates project's
  `executable:`), a search path (the ordered list of absolute directories the caller gives), and
  the workspace root (the host directory the runner mounts at `/workspace`); answer an
  `AdmittedProgram` (the name for `argv[0]`, the real path, the sha256) or a `ProgramRefusal`;
- `launch`: from an `AdmittedProgram` and the `ChildCommand` the caller built with the program's
  mount path (`AdmittedProgram::mount_path`, `/opt/loom/programs/<n>/<name>`) as its program, the
  command that runs it: the real file bound read-only there by `ChildCommand::with_read_only_file`
  (`crates/loom-intake-slice/src/confinement.rs:1157`), so `argv[0]` is that mount path, whose last
  component is the program name; a command whose program is not the mount path is refused
  `ProgramNameInvalid`. The caller builds the `ChildCommand` so that a git launch is visible to the
  hardening scan by its text (`story:observe-git-changed`);
- `verify`: after the run, the real file digested again; a digest that differs is
  `ProgramChanged`.

`admit` applies the same checks to every search path it is given, the host's or a gates
project's, against the workspace root it is given:

- a bare name is looked up on that search path only; never the `PATH` of Loom's own process and
  never a default of Loom's own beside the caller's;
- every entry must be absolute and outside the workspace root, whether or not it holds the program;
- the match is resolved to its real path once. That file is opened without following its last
  component and without waiting (`O_NOFOLLOW | O_NONBLOCK`, through `libc` flags on
  `std::fs::OpenOptions`), checked to be a regular file no larger than `MAX_PROGRAM_BYTES`
  (256 MiB) before one byte is digested, and its sha256 is the admitted digest; a real path inside
  the workspace root is refused;
- an absolute path is admitted as a match is, with no search.

Three callers, each with its own search path and its own place to record a refusal (the rule of the
epic, § Refusals):

- the program effect port, built per case, before any action of that case runs, for the bindings
  document and the case's workspace root: each `program` binding's `argv[0]`, and each
  `arguments_from` argv's `argv[0]` (that argv starts a process the same way,
  `story:bound-program-result-table`), on the host's search path. A refusal there is a
  `ProgramPortRefused` and no run exists. A relative name with a `/` is a workspace path; it is not
  passed to `admit`, is not mounted, and runs as written from `/workspace`;
- `story:observe-git-changed`, when the observe port is built for a case: `git` on the host's
  search path;
- `story:gate-run-collects-a-gates-document`, at the step, per project command: the project's
  `search_path` when it has entries, else the host's, or the command's absolute `executable:`.

Each launch goes through Substrate, which opens the mounted file once, refusing a link at any
component, and binds the descriptor it opened (`AGENTS.md` § Intake and the command line). rustup's
`cargo` link runs as `cargo`, and a `#!` script sees a path ending in its own name as `$0`; its
interpreter must be one the sandbox shows (`/usr`, `/bin`, `/lib` and the roots the host names,
`confinement.rs:1029-1031`). A launch carries at most Substrate's 16 read-only files
(`MAX_READ_ONLY_FILES`, `confinement.rs:1107-1109`): the program's mount plus those the action's
confinement names; one that would carry more is refused `ProgramMountsExceeded` when the port is
built, naming the action and the count. A git launch (`story:observe-git-changed`) and a gate
command's launch (`story:gate-run-collects-a-gates-document`) cannot pass the limit: each carries
exactly one read-only file, the admitted program, and nothing else names one (held there by
`git_runs_the_admitted_file` and `a_gate_command_gets_only_its_program`). Were one to carry 17, the
runner would refuse it `ScopeInvalid` before it starts (`confinement.rs:2386-2398`), which those
stories record as `LaunchRefused`.

`loom.bindings.ProgramRun` names the admitted program per action (`program`) and, for an action
with `arguments_from`, the admitted arguments program (`arguments_program`), so two actions whose
links were swapped name different programs. `verify` runs after each of the two launches;
`ProgramChanged` after either refuses the step.

## Why

Taken over from engineering-protocols on 2026-11-21 (`story:gate-program-identity-bound-to-spawn`
there, draft revision 2, frozen at tag `0.16.0`); session `sdc` decided Loom owns observation
collection (`.engineering/drafts/e7-request.md`). The engineering-protocols adversary pass found, at
`0.16.0`:

- `admit_command` sets `executable` to the search-path entry it matched, which may be a link
  (`crates/assertion-providers/src/process.rs:293-294`, `resolve_program` returning
  `path: candidate`, `:362-367`), and the spawn resolves it again (`Command::new(&binding.executable)`,
  `:171`);
- `executable_digest` reads its file to the end with no bound
  (`crates/assertion-providers/src/lib.rs:204-210`), so a path reaching `/dev/zero` or a FIFO never
  returns.

Upstream already admits one way for every search path: `admit_command` takes the project's
`search_path` when it has entries, else the runtime's default, and `resolve_program` holds both to
the same checks (`process.rs:266-296`; `resolve_program` at `:300`, its rule in the doc lines
`:298-299`). One unit here keeps that: the gate run's project
commands and git cannot get weaker checks than a bound program.

Loom's bound programs today start by `Command::new(&binding.argv[0])`
(`crates/loom-intake-slice/src/bindings.rs:415`) with only `PATH` inherited (`:54`), so the name
resolves on Loom's own `PATH` at each spawn and nothing is digested;
`story:bound-commands-run-confined` moves the spawn behind `run_child` but keeps the name.

The implemented gate stories of engineering-protocols each guarantee behaviour this story carries
as acceptance: `gate-commands-name-a-program-portably` (`process_program.rs`, `_adv1`, `_sec1`),
`gate-executable-link-keeps-its-name` (`executable_link.rs`), and
`gate-executable-identity-bound-to-spawn` (`executable_link_adv1.rs`, `_adv2.rs`), all under
`crates/assertion-providers/tests/` and `crates/canon-engineering-assertions/tests/` at `0.16.0`.

## Waits on

- `story:engineering-protocols-dependency-removed` (edge in the store).
- `story:bound-commands-run-confined`: the spawn goes through `run_child`.
- `story:bound-program-result-table`: the program effect port in `crates/loom-bindings` and
  `ProgramRun`, which this story extends.
- `story:observe-executor-reads-workspace-files`: both edit `ess/domains/bindings.yaml`,
  `generated/rust/loom/`, `crates/loom-bindings/src/lib.rs` and `CHANGELOG.md`.

Waiting on this one: `story:observe-reads-documents` (direct edge), and through it
`story:observe-compares-generated-trees`, `story:report-formats-checked-before-the-table` and
`story:gates-document-evaluated-in-the-governor`: each shares `ess/domains/bindings.yaml` (story 7:
`ess/system.yaml` instead), `generated/rust/loom/` and `CHANGELOG.md` with this story, and
`story:report-formats-checked-before-the-table` also `crates/loom-bindings/src/program.rs`.

## ESS first

`ess/domains/bindings.yaml` first, extending `story:bound-program-result-table`'s `ProgramRun`
(`action`, `exit`, blob digest, `timed_out`: `bound-program-result-table.md:63`):

- `AdmittedProgram` (`name`, `path`, `sha256`);
- `ProgramRun` gains `program` (`Optional<loom.bindings.AdmittedProgram>`, absent for a workspace
  path), `arguments_program` (`Optional<loom.bindings.AdmittedProgram>`, present when the action
  has an admitted `arguments_from`) and `refusal` (`Optional<loom.bindings.ProgramRunRefusal>`);
- enum `ProgramRunRefusal`, a step's refusal: `ProgramChanged`;
- enum `ProgramRefusal`, the admission unit's refusal: `ProgramNameInvalid`, `SearchPathInvalid`,
  `ProgramNotFound`, `ProgramInsideWorkspace`, `ProgramNotRegular`, `ProgramTooLarge`,
  `ProgramMountsExceeded`;
- `ProgramPortRefused` (`action`, `refusal`, `detail`), what the program port's build returns.

On that commit `task drift` fails and `crates/loom-bindings/tests/program_identity.rs` does not
compile.

## Evidence record

A program's evidence record is unchanged from `story:bound-program-result-table`: this story adds
members to the observation (`ProgramRun.program`, `ProgramRun.refusal`), none to the record. Two
runs of an unchanged program write records equal but for `id` and `observed_at`, and observations
naming the same `AdmittedProgram`.

## Acceptance

Tests in `crates/loom-bindings/tests/program_identity.rs` (the delegated cases run under
`systemd-run --user --scope -p Delegate=yes` and skip naming the missing prerequisite, as
`crates/loom-intake-slice/tests/confined_child.rs` does), with programs compiled by the test where
`argv[0]` matters, since a shell script's `$0` names the file executed. "Port refusal" below means:
the program port's build returns `ProgramPortRefused` with the `refusal` named, no `ProgramRun`
exists, and no process starts.

The unit:

- `admit_answers_the_admitted_program`: `admission::admit` on a bare name, a search path holding it
  and a workspace root returns an `AdmittedProgram` whose `path` is the real path and whose
  `sha256` is the file's.
- `launch_mounts_the_real_file_at_its_name`: `admission::launch` of that program over a
  `ChildCommand` built with its mount path gives a command whose read-only files are exactly one,
  the real path at `/opt/loom/programs/<n>/<name>`, and whose program is that mount path.
- `launch_refuses_a_command_for_another_program`: over a `ChildCommand` built with any other
  program, `admission::launch` returns `ProgramNameInvalid`.
- `verify_reports_a_rewritten_file`: `admission::verify` after the real file is rewritten returns
  `ProgramChanged`; on an unchanged file it returns nothing.
- `admit_takes_an_absolute_path_without_searching`: given an absolute path and an empty search path,
  it returns that file's real path and sha256; given a relative path with a `/`, it returns
  `ProgramNameInvalid`.
- `the_admission_unit_refuses_a_workspace_entry_in_any_search_path`: a search path passed as a
  gates project's (entries from repository data) with an entry inside the given workspace root
  returns `SearchPathInvalid`, as the host's does below; the same entry with another workspace root
  is accepted.
- `a_workspace_path_program_runs_as_written`: a binding whose `argv[0]` is `tools/run` (a file the
  test writes in the workspace) runs that file from `/workspace`: its output is the file's, and
  `ProgramRun.program` is absent.
- `an_arguments_program_is_admitted`: an action with `arguments_from` whose `argv[0]` is a bare name
  on the search path runs the admitted file, and `ProgramRun.arguments_program` names its path and
  sha256.
- `an_arguments_program_not_on_the_search_path_is_refused`: with the name only on the test
  process's `PATH`, port refusal `ProgramNotFound`, naming the action.

Carried over from the engineering-protocols story, restated against the confined launch:

- `a_link_swapped_between_admission_and_spawn_runs_the_admitted_program`: a binding whose bare name
  matches a link on the search path, retargeted after the port was built and before the action
  runs, runs the file admission digested (its output is the admitted program's) or is refused with
  `ProgramRun.refusal` `ProgramChanged`; no record comes from the new target's output, and
  `ProgramRun.program` names the admitted path and sha256. A second case retargets it during the
  run and gets the same.
- `a_program_that_is_not_a_regular_file_is_refused_before_its_digest`: a search-path match that is
  `/dev/zero`, a FIFO (with no writer) or a directory is a port refusal `ProgramNotRegular`, within
  1 s, naming the path.
- `a_program_larger_than_the_bound_is_refused_before_its_digest`: a file larger than
  `MAX_PROGRAM_BYTES` is a port refusal `ProgramTooLarge`, within 1 s.

Carried from the implemented engineering-protocols stories:

- `an_admitted_link_runs_under_the_programs_name`: a program that prints its `argv[0]`, reached by
  a link named `tool`, prints a path whose last component is `tool`, not the target's name
  (`an_executable_link_runs_under_its_own_name`).
- `a_link_chain_runs_under_the_first_links_name`.
- `a_script_reached_through_a_link_sees_the_link_as_its_name`: a `#!` script prints a `$0` ending in
  the link's name.
- `a_dangling_link_or_a_link_to_a_directory_is_refused_at_admission`: port refusal
  `ProgramNotFound` naming the link.
- `the_first_search_path_entry_holding_an_executable_file_wins`: `ProgramRun.program.path` is the
  first entry's file.
- `a_file_without_an_execute_bit_is_no_match`: with only such a file on the search path, port
  refusal `ProgramNotFound`.
- `no_search_path_refuses_a_bare_name`: a port built with an empty search path gives port refusal
  `ProgramNotFound`, naming the program.
- `a_missing_program_is_named`: `ProgramPortRefused.detail` names the program and every entry.
- `a_search_path_entry_inside_the_workspace_is_refused`: an entry that is, lies in, or resolves into
  the workspace (through `..`, a link, or `/proc/self/cwd`) is port refusal `SearchPathInvalid`,
  naming it, even after a good entry.
- `a_relative_search_path_entry_is_refused`: port refusal `SearchPathInvalid`.
- `a_match_whose_real_path_is_in_the_workspace_is_refused`: port refusal `ProgramInsideWorkspace`.
- `names_that_are_not_bare_are_refused`: an empty name, `.`, `..`, a NUL, or more than 255 bytes is
  port refusal `ProgramNameInvalid`.
- `the_callers_path_is_never_used`: with the test process's `PATH` holding the program and the
  search path not, port refusal `ProgramNotFound`.
- `a_changed_program_cannot_reuse_the_admission`: the real file rewritten after admission gives
  `ProgramRun.refusal` `ProgramChanged` after the run, and no record.
- `two_actions_that_swap_their_links_name_different_programs`: the two `ProgramRun.program` values
  differ in `path`.
- `a_project_that_writes_argv0_is_refused`: a bindings document with an `argv0` member is refused
  at decode, `BindingsRefusalReason` `UndeclaredMember`. This is a guard, not a red-first test: it
  passes on today's tree (`crates/loom-bindings/src/decode.rs:27-40`) and keeps the member out.

Confinement and limits:

- `the_child_gets_only_the_admitted_program`: the launch's applied confinement (read from its
  `TestExecution`) lists the admitted program as the one read-only file beyond what the action's
  confinement names.
- `programs_beyond_the_read_only_file_limit_are_refused`: an action whose confinement, as the host
  gives it, already names 16 read-only files is port refusal `ProgramMountsExceeded`, naming the
  action and the count 17; with 15 it is accepted.
- The admission digest reads at most `MAX_PROGRAM_BYTES` (the `ProgramTooLarge` case above); wall
  time, output and process count are the program executor's (`story:bound-program-result-table`,
  `story:action-limits-enforced`).

Records and the rest:

- `the_record_is_unchanged`: a program's evidence record has exactly the members of
  `story:bound-program-result-table`'s, and two runs of an unchanged program have equal
  `ProgramRun.program`.
- `CHANGELOG.md` `### Changed`: `**Breaking:** a bound program named by a bare name resolves on the
  host's search path, never Loom's PATH; one not found there is refused before it starts`.
- `website/docs/guides/bindings.md` says how a program name resolves.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `generated/rust/loom/` (regenerated),
`crates/loom-bindings/src/program.rs` (the program effect port; `story:bound-program-result-table`
names only the crate, so the file name is this set's, and if that story puts the port in another
file, this story and `story:report-formats-checked-before-the-table` edit that one),
`crates/loom-bindings/src/admission.rs` (new, the unit: `admit`, `launch`, `verify`),
`crates/loom-bindings/src/lib.rs`, `crates/loom-bindings/Cargo.toml` (`sha2 = "0.10"` and
`libc = "0.2"`, both already in `Cargo.lock` through `b10x-loom-intake-slice`; the lock changes
only in `b10x-loom-bindings`' dependency list), `Cargo.lock`,
`crates/loom-bindings/tests/program_identity.rs` (new), `website/docs/guides/bindings.md`,
`CHANGELOG.md`.

## Out of scope

- A hostile process of the collector's own user that rewrites the resolved file itself between
  Substrate's open and the exec (the digest after the run reports it; it cannot prevent it).
- The engine's executable allow-list (`story:executable-allow-list`, another epic): it judges the
  admitted path this unit produces.
- Where the host's search path is configured: the host passes it to the port; `loom.engine/1`
  (`story:engine-configuration-document`) may carry it later.
- Which search path a gates project command uses, and recording its refusal: the caller's
  (`story:gate-run-collects-a-gates-document`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (`src/admission.rs`, new, plus the program port in `src/program.rs`) — cited, story `## Outcome` and `## Files`
- **Files:** `ess/domains/bindings.yaml` — cited, `## ESS first`; first commit, `task drift` fails on it
- **Files:** `crates/loom-bindings/src/admission.rs` (new) — cited, `## Outcome`
- **Files:** `crates/loom-bindings/src/program.rs` — cited, `## Files`; the file does not exist yet, and `story:bound-program-result-table` names only the crate, so the name is this set's
- **Files:** `crates/loom-bindings/src/lib.rs` — cited, `## Files`; only `mod`/`pub use` lines today (`lib.rs:15-20`)
- **Files:** `crates/loom-bindings/tests/program_identity.rs` (new) — cited, `## Acceptance`
- **Files:** `website/docs/guides/bindings.md` — cited, acceptance bullet; the file exists
- **Files:** `crates/loom-bindings/Cargo.toml` — inferred, needs `sha2` (not in the crate today) and a way to open with `O_NOFOLLOW | O_NONBLOCK`; the story's `## Files` omits it
- **Symbols (new):** `admission::admit`, `AdmittedProgram`, `ProgramRefusal`, `ProgramRunRefusal`, `ProgramPortRefused`, `MAX_PROGRAM_BYTES` — cited; `git grep` finds none of them in `crates` or `generated`
- **Symbols (read, not changed):** `ChildCommand::with_read_only_file` (`crates/loom-intake-slice/src/confinement.rs:1157`), `TestExecution` (`confinement.rs:79`), `ChildCommand::MAX_READ_ONLY_FILES` (`confinement.rs:1109`, = 16) — cited
- **Symbols (extended from another story):** `ProgramRun` — cited; it does not exist yet, `story:bound-program-result-table` creates it (`.engineering/planning/story/bound-program-result-table.md:63`)
- **Also likely:** none. `crates/loom-intake-slice/src/bindings.rs:415` (`Command::new(&binding.argv[0])`) is replaced by `story:bound-commands-run-confined`, not here — inferred
- **Already holds:** `executor.argv0` is refused as `UndeclaredMember` today, because `EXECUTOR` has no `argv0` (`crates/loom-bindings/src/decode.rs:28-40`, `known` at `:767-777`). `a_project_that_writes_argv0_is_refused` needs no source change — cited
- **Documents:** `website/docs/guides/bindings.md` only; every other acceptance line is code or ESS — cited
- **Confidence:** high for the file set, since the story lists it and I checked each path against the tree. Medium for the shape of `program.rs`, which depends on a file another story creates.
- **Would collide with** (order read from `aep plan artifact graph --format json`):
  - `ess/domains/bindings.yaml`:
    - ordered by `depends_on`, before this story: `bound-program-result-table`, `observe-executor-reads-workspace-files`, `bindings-document` (implemented).
    - ordered by `depends_on`, after this story: `observe-git-changed`, `observe-reads-documents`, `report-formats-checked-before-the-table`, `observe-compares-generated-trees`, `gate-run-collects-a-gates-document`, and `tcp-reachability-through-an-aperture` and `dns-resolution-through-a-resolver-aperture` through the chain.
    - **no `depends_on` path either way:** `run-caps-per-arrival`, `record-copy-between-cases`, `bound-artifact-revisions`, `agent-node-run-records`, `bound-agent-node`, `child-case-own-workspace`, `confinement-level-required`, `named-egress-per-action`, `action-limits-enforced`, `capacity-given-to-an-admission-program`, `limit-hits-in-run-log`, `autonomy-grant-table`, `engine-configuration-document`, `agent-session-through-substrate`, `bindings-child-path-refusal-names-the-bindings-directory`. Each names the file in its body; I did not check that each edits it, so inferred.
  - `crates/loom-bindings/src/lib.rs`: before this story, `bound-program-result-table`, `observe-executor-reads-workspace-files`, `bindings-document`. Unordered, `run-caps-per-arrival`, `record-copy-between-cases` (its scope names `lib.rs`), `child-case-runtime` (cites `lib.rs:79-99`).
  - `website/docs/guides/bindings.md`: before this story, `observe-executor-reads-workspace-files`. After it, `gate-run-collects-a-gates-document`. Unordered, `capacity-given-to-an-admission-program`, `run-caps-per-arrival`.
  - `crates/loom-bindings/src/program.rs`: created by `bound-program-result-table` or whichever story puts the port there (before this story); after it, `observe-reads-documents`, `report-formats-checked-before-the-table`.
  - `crates/loom-bindings/src/admission.rs`: after this story, `observe-git-changed`, `gate-run-collects-a-gates-document`. No unordered story.
  - `crates/loom-bindings/Cargo.toml`: before this story, `bound-program-result-table` (adds path dependencies and `sha2`).
  - Not a collision: `story:bound-commands-run-confined` owns `crates/loom-intake-slice/src/{bindings,confinement}.rs` and is ordered before this story.
- **Safety fact:** the port can mount the admitted real file, because `with_read_only_file` takes a host path and a mount path (`confinement.rs:1157`), and Substrate opens it once with no link at any component (`AGENTS.md` § Intake and the command line, `:2131-2136`). `AppliedConfinement` carries `read_only_files` (`confinement.rs:862`, `:1383`), so the confinement test can read it back. Pointed at `file:line`, proof level 2, unproven.
