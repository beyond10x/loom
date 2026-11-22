---
format: aep.planning-md/3
id: story:per-run-program-path
kind: story
status: proposed
title: A run's settings carry the PATH its bound programs start with, refused before any process when unusable and recorded at the start of the run
summary: program_path member of the run settings and --program-path; absent means the process PATH; refusals ProgramPathUnusable; a program path line in the run log with source, count and digest
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:tree-run-command
- depends_on: story:bound-program-contract
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/child.rs
- confidence: cited
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-bindings/tests/bound_case_id.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: inferred
  path: crates/loom-cli/tests/tree_run.rs
- confidence: inferred
  path: crates/loom-tree
- confidence: inferred
  path: docs/contracts/run-log-v1.json
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: inferred
  path: ess/domains/tree.yaml
- confidence: inferred
  path: generated/rust/loom
- confidence: cited
  path: website/docs/guides/bindings.md
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
- confidence: cited
  path: website/docs/reference/cli.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T02:06:20Z", actor: "agent:loom", revision: 6}
---
## Outcome

A run's settings can name the `PATH` of the bound programs of that run. When they do, every bound
program of the run and every `arguments_from` program starts with that `PATH`. When they do not,
the programs start with the process's `PATH`, as today.

**The setting.** `run_tree` takes a settings value (`story:tree-run-command`, which names no
type for it yet). That value gains two members:

- `program_path: Option<String>`: a list of directories separated by `:`;
- `program_path_source: ProgramPathSource`, `library` by default.

`b10x-loom tree run` gains `--program-path <dirs>`. The CLI sets both members, the source to
`command line`, the spelling of the source label that `story:engine-agents-section` uses for a
flag.

**Where the value goes.** `ProgramEffects::with_program_path(self, path) -> Self` stores the
checked value on the port. `ProgramEffects::new` (`crates/loom-bindings/src/program.rs:132-150`)
keeps its six parameters. The `OpenEffects` port of `story:open-children-from-arguments-from`
takes the value the same way for its `arguments_from` program. `run_tree` gives the value to
every port it builds.

`bound_case_child` (`crates/loom-bindings/src/child.rs:33-53`) builds every such child. It already
rebuilds the child's whole environment from `ProgramEnvironment` (`:47-52`, `:57-70`). With a
program path, `ProgramEnvironment.path` is that value in place of the host's. The variables
`story:bound-program-contract` and `story:bound-program-case-id` add (`LOOM_CASE`,
`LOOM_CASE_NAME`, `LOOM_PARENT_CASE_NAME`, `LOOM_TIMEOUT_SECONDS`, `LOOM_RETRY_FEEDBACK`) do not
change.

`bound_program_child` (`crates/loom-intake-slice/src/bindings.rs:414-450`) is not changed. It
still reads the process's `PATH` (`:75`, `:421-430`). The intake slice's `run --action-bindings`
path keeps it.

**Under a runner that is not `local`.** A bare program name resolves inside the sandbox, where
only the system view (`/usr`, `/bin`, `/lib`, `/lib64`, `bindings.rs:86`) and the mounted paths
are visible. So each entry outside the system view is mounted read-only at its own path
(`ChildCommand::at_host_path`, `crates/loom-intake-slice/src/confinement.rs:1297-1306`), as an
absolute program is today (`bindings.rs:440-446`). Substrate mounts at most
`ChildCommand::MAX_READ_ONLY_ROOTS` (4) directories (`confinement.rs:1233`, `:1255`).

**Refused at the start.** `run_tree` checks the value before anything else. A refused value
starts no process and writes no line. The refusal is `ProgramPathUnusable`, a new reason of
`loom.tree.TreeRefusalReason`, naming the entry (escaped) and the rule it breaks:

- the value is longer than 4096 bytes (`MAX_PROGRAM_PATH_BYTES`);
- an entry is empty (a leading, trailing or doubled `:`);
- an entry is not absolute;
- an entry has a `..` component;
- an entry is not an existing directory.

Under a runner that is not `local`, three more cases are refused, each of which the mount would
refuse later at launch with `ScopeInvalid` (`confinement.rs:1242-1259`):

- an entry is reached through a symbolic link;
- an entry is, holds or lies inside the run's workspace;
- more than 4 entries lie outside the system view.

The check is one public function of `b10x-loom-bindings`,
`program_path(value, workspace, local) -> Result<ProgramPath, ProgramPathRefusal>`. Both ports
take its result, so neither ever holds an unchecked value. When the setting is absent the
process's `PATH` is used unchecked, as today.

**Recorded.** At the start of each `tree run`, before its first step, the root case's run log
gets one line `program path` (`loom.runlog.ProgramPath`). It has three members:

- `source`: `command line`, `library` or `process`;
- `entries`: the number of directories;
- `sha256`: the SHA-256 of the value as given, or of the process's `PATH` when the source is
  `process`.

The line holds no directory and no value. `story:engine-agents-section` states that its
`run settings` line holds no absolute path and no environment variable's value, and this line
keeps the same rule so that N9 can take it over. A caller checks the value by hashing the string
it gave.

**How N9 takes it over.** This story adds a small line of its own and not the `RunSettings` line:
`RunSettings` does not exist on the tree yet, and N9 (`story:engine-agents-section`) is drafted
and not in the store. When N9 lands:

- `RunSettings` gains `program_path` of the same type, `loom.runlog.ProgramPath`;
- N9's unit stops writing the separate line, in the same commit;
- the `ProgramPath` member of `loom.runlog.RunLogEvent` stays declared, so a log written
  before N9 still decodes and verifies.

The type and the three labels stay unchanged, so neither unit edits what the other declared.
N9's mapping reads this member as a typed member.

## Why

Runbook's daemon runs several runbooks in one process, and each runbook brings its own directory
of tools. Today a bound program gets the process's one `PATH` (`bindings.rs:75`, `:421-430`;
`website/docs/guides/bindings.md:216-220`). A daemon would have to put every runbook's
directory on it, and then a run of one runbook can start another runbook's tool under the same
name. Timo: runs "in an isolated, secure way". A per-run `PATH`, checked and recorded, lets each
run find only its own tools and lets the run log say which list it used.

## ESS first

`ess/` changes:

- `ess/domains/runlog.yaml`: `loom.runlog.ProgramPath` (`source`, `entries`, `sha256`),
  `loom.runlog.ProgramPathSource` (`CommandLine`, `Library`, `Process`), and the member
  `ProgramPath` of `loom.runlog.RunLogEvent` (`:36-50`);
- `ess/domains/tree.yaml` (added by `story:tree-run-command`, `:260-284` on
  `impl/tree-run-command`): `ProgramPathUnusable` in `loom.tree.TreeRefusalReason`.

On that commit `slot task drift` fails, `docs/contracts/run-log-v1.json` is out of date, and the
new tests in `crates/loom-bindings/tests/bound_case_id.rs` and `crates/loom-tree` do not
compile.

## Acceptance

The fixtures are a protocol with a `program` action and an `arguments_from`, bound to `tool` by
its bare name. There are two tool directories: in `a/bin`, `tool` prints `a` and `only-a` exits
0; in `b/bin`, `tool` prints `b` and `only-b` exits 0. The tests use no model and no network. The
`local` tests use an `EngineRunner` on `local` with `UNCONFINED_FLAG_REASON`. The `linux` test
uses the delegated harness of `crates/loom-intake-slice/tests/support/mod.rs:37-58`, skips
naming the missing prerequisite where there is no delegated cgroup v2, and is `#[ignore]` off
Linux. Each test ends within 60 s, and the test step stays within 120 s. The new tests go into
existing files, which become modules of `tests/main.rs` under `story:one-test-binary-per-crate`.

- Two runs in one process from two threads: one with `program_path` `<a>/bin:/usr/bin:/bin`,
  one with `<b>/bin:/usr/bin:/bin`. Each run's `tool` prints its own letter. A program of the
  first run that calls `only-b` exits 127, and so does one of the second run that calls
  `only-a`. Test: `two_runs_with_their_own_program_path_find_only_their_own_tools`.
- The same holds under `linux`, with each `bin` directory mounted read-only at its own path.
  Test: `a_confined_program_finds_its_tool_on_the_runs_program_path`.
- The `arguments_from` program of a run gets the same `PATH` as its bound program. Test:
  `an_arguments_from_program_gets_the_runs_program_path`.
- With no `program_path`, a program's `PATH` is the process's `PATH`. The run's `program path`
  line has `source` `process` and the SHA-256 of that value. Test:
  `without_a_program_path_a_program_gets_the_process_path`.
- With `program_path` given through the library, the line has `source` `library`, `entries` 3
  and the SHA-256 of the string given. It is the first line the run appends, and no line holds a
  directory of the value. Test: `the_program_path_is_recorded_with_its_source_and_digest`.
- `tree run --program-path <a>/bin` writes the line with `source` `command line`. Test (in
  `crates/loom-cli`): `the_program_path_flag_is_recorded_as_command_line`.
- Each of these values is refused `ProgramPathUnusable`, naming the entry and the rule: 4097
  bytes, `:/usr/bin`, `/usr/bin:`, `/usr/bin::/bin`, `bin`, `/opt/../usr/bin`, a missing
  directory and a regular file. No process starts (a recording runner receives no child), and the
  store holds no new run-log line. Test: `an_unusable_program_path_is_refused_before_any_process`.
- Under a runner that is not `local`, three more values are refused `ProgramPathUnusable` before
  any process: an entry reached through a symbolic link, an entry inside the workspace, and five
  entries outside the system view. The same values are admitted under `local`. Test:
  `a_program_path_the_sandbox_cannot_mount_is_refused_before_any_process`.
- A request argument, an `arguments_from` output, a report and a bindings document that each
  name `PATH` change nothing: the program's `PATH` is the run's value. Test:
  `nothing_but_the_runs_settings_sets_the_program_path`.
- The intake slice's command path keeps the process's `PATH`. Command:
  `git diff --stat $(git merge-base main HEAD)..HEAD -- crates/loom-intake-slice/src/bindings.rs`
  prints nothing.
- The guide states the setting, the flag and the line. Command: `git grep -c program_path --
  website/docs/guides/bindings.md website/docs/guides/run-a-case-tree.md AGENTS.md` prints a count
  of at least 1 for each file.
- The generated code and the run-log contract match the specification. Command: `slot task drift`
  exits 0 after `task generate`. On the ESS first commit it fails.
- `slot task check` exits 0.

## Out of scope

Changes to Substrate, to Canon, and to the CLI's default: without `--program-path` the CLI still
gives the process's `PATH`. A `PATH` per binding or per case. Other variables per run. Resolving a
bare name in Loom before the start: the program still resolves it. The `RunSettings` line, which
is N9's (`story:engine-agents-section`). The intake slice's `run --action-bindings` path. The
fixed text `UNCONFINED_FLAG_REASON` stays; Runbook decided no story for it.

Depends on `story:tree-run-command` (the settings value of `run_tree`, `tree.yaml`, the
`tree run` command) and `story:bound-program-contract` (`bound_case_child` as it builds the
whole environment). It comes directly after `story:tree-run-command`. It shares no source file
with `story:concurrent-confined-runs`, so it can be built beside that story or before it. Their
only common files are `AGENTS.md` and `CHANGELOG.md`, in other sections. It shares
`crates/loom-bindings/src/program.rs` with `story:cancel-a-running-bound-program`.

## Scope

Derived 2026-11-22 from the plan tree at `1d6757d1` (`wave/2026-11-21-w7`) and
`impl/tree-run-command` at `57728f01`. **Cited** means read at the `file:line` named.
**Inferred** means a reading that could be wrong.

- **Primary surface:** `crates/loom-bindings/src/child.rs`: `bound_case_child` (`:33-53`) takes
  the run's path for `ProgramEnvironment.path` (`:47-52`) and mounts its entries under a runner
  that is not `local`; `environment` (`:57-70`) is unchanged — cited
- **Files (source):** `crates/loom-bindings/src/program.rs`: the port's fields (`:102-109`),
  `with_program_path` beside `new` (`:132-150`), the two `bound_case_child` calls
  (`:243-253`, `:495-505`) — cited. A new module for `program_path`, `ProgramPath`,
  `ProgramPathRefusal` and `MAX_PROGRAM_PATH_BYTES`, exported from `src/lib.rs` (`:22-37`) — cited
  for `lib.rs`, inferred for the module's name
- **Files (source):** the `OpenEffects` port (`crates/loom-bindings/src/arguments.rs`, from
  `story:open-children-from-arguments-from`) — inferred; not on the tree yet
- **Files (source):** `crates/loom-tree`: the settings value of `run_tree`, the check at the
  start, the `program path` line through the tree's log writer (`Logs`, `pub(crate)` in
  `crates/loom-tree/src/log.rs` on `impl/tree-run-command`) — inferred; `run_tree` is not on that
  branch yet
- **Files (source):** `crates/loom-cli/src/tree.rs`: `--program-path` on `tree run` — inferred
  (`tree run` is not on the branch yet)
- **Files (ESS source):** `ess/domains/runlog.yaml` (`RunLogEvent` `:36-50`) — cited;
  `ess/domains/tree.yaml` (`TreeRefusalReason` `:260-284` on `impl/tree-run-command`) — cited
- **Files (generated):** `generated/rust/loom/**`, `docs/contracts/run-log-v1.json` (generated
  from `loom.runlog`, `AGENTS.md` § Run log), `website/docs/reference/ess/loom-runlog.md` — cited
  for the contract and the page, inferred for the Rust paths
- **Not changed:** `crates/loom-intake-slice/src/bindings.rs` (`INHERITED` `:75`,
  `bound_program_child` `:414-450`), `crates/loom-intake-slice/src/confinement.rs` (`at_host_path`
  `:1297-1306` is used as it is), `ess/domains/bindings.yaml` (`ProgramEnvironment.path` already
  exists) — cited
- **Files (tests):** `crates/loom-bindings/tests/bound_case_id.rs` (the environment tests of
  `story:bound-program-case-id`) — cited; a test file of `crates/loom-tree` and
  `crates/loom-cli/tests/tree_run.rs` (named by `story:tree-run-command`'s Scope) — inferred
- **Files (docs):** `website/docs/guides/bindings.md:216-220` ("`PATH` and `LOOM_CASE` alone in
  its environment"), `website/docs/guides/run-a-case-tree.md` (from `story:tree-run-command`),
  `website/docs/reference/cli.md` (the flag), `AGENTS.md` § Bindings document (`:1283`) and § Run
  log (`:1392`), `CHANGELOG.md` one `Added` bullet under `[Unreleased]` (`:13-15`) — cited for the
  existing files, inferred for the tree guide
- **Would collide with:** `story:cancel-a-running-bound-program` on `program.rs` (two builders
  side by side); `story:bound-program-contract` on `child.rs` (it lands first);
  `story:engine-agents-section` on `ess/domains/runlog.yaml` (it takes the type over, as
  § Outcome states); `story:tree-run-command` on `crates/loom-tree` and `tree.yaml` (it lands
  first) — inferred for the order
