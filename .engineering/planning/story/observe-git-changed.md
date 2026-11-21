---
format: aep.planning-md/3
id: story:observe-git-changed
kind: story
status: proposed
title: The observe port gains a launcher, and git.changed runs git through it with a fixed hardened argv and the host's base and subject
summary: Merge-base diff of two host-given revisions, glob matched by Loom; missing revision unavailable, never false; repository configuration runs nothing
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:report-formats-checked-before-the-table
- depends_on: story:gate-program-identity-bound-to-spawn
- depends_on: story:bound-commands-run-confined
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/git.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/launch.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_files.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_git.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:54:59Z", actor: "agent:sdc", revision: 10, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

The `observe` executor answers `git.changed` (`glob`): `true` when a path matching the glob differs
between the `base` and `subject` revisions of the change, measured from where the subject forked
from the base. `base` and `subject` are trusted host context: the host gives them to the observe
port as an argument of each run (`ObserveContext.change`), never the model and never the bindings
document.

This story adds a launch port to the observe port: `ObserveLauncher`
(`crates/loom-bindings/src/observe/launch.rs`), one method that runs a `ChildCommand` through
`SubstrateRunner::run_child` (`crates/loom-intake-slice/src/confinement.rs:475`) and returns what it
returns: a `TestExecution` (exit code, timed out, output, the applied confinement, the driver's
record, and `child_output`, standard output and standard error apart, each flagged when cut;
`confinement.rs:78-90`), or its `ConfinementError`. `TestExecution` carries no signal: a child that
ends with no exit status, no timeout and no declared byte ceiling is refused by the runner
(`CapabilityUnserved`, `confinement.rs:947-952`), and the launcher passes that refusal on. The trait and its
implementation over the runner are both in `b10x-loom-bindings`, which depends on the slice
(`story:bound-program-result-table`). Two operations may use it: `git.changed` (this story) and
`engineering.gates` (`story:gate-run-collects-a-gates-document`: `git cat-file` and the project's
commands). No file or document operation may: story 1's rule stands for them.

What this does to story 1:

- The observe port is built with a reader and, optionally, a launcher. A port built with a reader
  and no launcher (the reader-only build) still serves every file and document operation; for a
  document that binds `git.changed` it returns `ObservePortRefused` with `refusal` `NoLauncher`, and
  no run exists. A port with no reader is still `NoReader`, launcher or not.
- `no_file_or_document_operation_starts_a_process` is kept: it builds the port with a counting
  launcher as well as the counting runner and asserts zero launches for every file and document
  operation, so a file operation that started using the launcher would fail it.

When the observe port is built with a launcher for a document that binds `git.changed` or
`engineering.gates`, it admits `git` once through `admission::admit`
(`crates/loom-bindings/src/admission.rs`, `story:gate-program-identity-bound-to-spawn`) on the
host's search path and the case's workspace root; a refusal there is `ObservePortRefused` with
`refusal` `GitNotAdmitted` and the unit's `ProgramRefusal` in `program_refusal`, and no run exists.
For a document that binds neither, git is not admitted, so a host without git still serves the file
and document operations.
Every git launch is built by the unit's `launch` (the real file mounted read-only, `argv[0]` its
mount path) and followed by its `verify` (`ProgramChanged`), as for any bound program.

Every launch goes through one function of `crates/loom-bindings/src/observe/git.rs`, with no
network, the workspace bound as it is for any child (writable only under `target/`; `.git` not
named writable), and a fixed argv and environment Loom owns. Four launches at most: `rev-parse
--verify --quiet --end-of-options <rev>^{commit}` for each revision, `merge-base <base> <subject>`,
and `diff-tree -r --name-only --no-renames --relative -z <left> <subject> --`. `fork` is the merge base of `base` and `subject`; `<left>` is `fork`, or `base` when
`fork` equals `subject` (the subject is already inside the base), as upstream
(`crates/assertion-providers/src/git.rs:100`, tag `0.16.0`). A launch that exits non-zero gives the
reason `git <subcommand> failed with exit <n>: <first stderr line>`, at most 200 characters of
stderr, as upstream builds it (`git.rs:247`). Every launch carries `-c
core.fsmonitor=false -c core.hooksPath=/dev/null -c core.pager=cat -c diff.external= -c
core.sshCommand=false -c protocol.allow=never -c protocol.ext.allow=never` and exactly the
environment `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_NO_LAZY_FETCH=1`,
`GIT_PROTOCOL_FROM_USER=0`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`. Loom
matches the glob over the NUL-separated paths itself.

`crates/loom-intake-slice/tests/host_git_hardening.rs::every_host_git_command_goes_through_a_hardened_helper`
finds every `Command::new(` in `crates/*/src` (the match also finds `ChildCommand::new(`) whose
argument text, up to the first `)`, contains `git` (`:464-475`), and refuses one outside its named
helpers, each of which must start git in exactly one place and set `core.hooksPath=` and
`core.fsmonitor=false` (`:479-500`). This story adds `crates/loom-bindings/src/observe/git.rs` to
that list as a fourth helper, the confined one. Its one launch function builds
`ChildCommand::new(git.mount_path(), …)` (the admitted git held in a variable named `git`, so the
argument text names git and the scan counts exactly one start) and hands it to the unit's `launch`;
it sets both flags. `AGENTS.md` § Intake and the command line names it beside the slice's and the
pinned-source helper.

A workspace that is a linked worktree has a `.git` file pointing at a git directory outside the
workspace, which the sandbox does not show (a child sees only the host paths the host names,
`confinement.rs:1029-1036`); git then cannot find the repository and the operation is
`Unavailable` with git's reason. `story:observe-git-in-a-linked-worktree` mounts the common git
directory read-only and serves this case; until it lands, this answer stands.

## Why

`git.changed` is engineering-protocols' only change-aware operation, added so a gate can require
that a consumer-visible change touches the changelog (`story:a-gate-sees-the-change-and-may-not-apply`,
implemented there; `crates/assertion-providers/src/git.rs:62-125`, tag `0.16.0`). It runs git from
fixed host locations as a host process (`git.rs:11-16`, `:216-255`). Substrate `0.21.0`'s `git`
feature compares a working tree with the commit it materialized from a configured source
(`read_workspace_git_changes_v2`, `crates/substrate-host/src/lib.rs:720`; `GitChangesQuery`,
wire `lib.rs:1448`), not two revisions of an adopted workspace, so it cannot answer this. A
`program` binding cannot either: its argv is binding data, while the revisions are host context and
the hardening must not be the document's to drop. Loom's host-side git helper
(`crates/loom-intake-slice/src/git.rs`, `AGENTS.md` § Intake and the command line)
runs outside Substrate.

## Waits on

- `story:report-formats-checked-before-the-table`: shared `ObserveOperation`, `observe.rs`
  dispatch, `ess/domains/observation.yaml`, `generated/rust/loom/` and `CHANGELOG.md`.
- `story:gate-program-identity-bound-to-spawn`: the admission unit git goes through.
- `story:bound-commands-run-confined`: `run_child` as the one launch path for a bound process.

Waiting on this one: `story:gates-document-evaluated-in-the-governor` (edge added in this revision:
both edit `generated/rust/loom/` and `CHANGELOG.md`) and `story:gate-run-collects-a-gates-document`
(the launcher, `observe/git.rs`, `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`), and
`story:observe-git-in-a-linked-worktree` (`observe/git.rs`, `observe_git.rs`; it holds no other
story).

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `GitChanged` (spelled `git.changed`).
`ess/domains/observation.yaml`:

- `ObserveContext` (`change`: `ChangeContext` with optional `base` and `subject`);
- `ObserveRefusal` gains `RevisionInvalid`, `GlobInvalid`, `ProgramChanged`, `LaunchRefused`
  (`BindingsRefusalReason` is not changed: a literal glob that cannot match is the load-time
  `InvalidObserveArguments`);
- `ObserveRun` gains `revisions` (optional `ResolvedChange`: `base`, `fork`, `subject` object ids);
- `ObservePortRefusal` gains `NoLauncher` and `GitNotAdmitted`; `ObservePortRefused` gains
  `program_refusal` (`Optional<loom.bindings.ProgramRefusal>`).

On that commit `task drift` fails and `crates/loom-bindings/tests/observe_git.rs` does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record. The value depends only on the
resolved object ids and the glob; the observation names the ids, so two runs over the same
revisions write equal records but for `id` and `observed_at`. A branch name that moved between two
runs resolves to other ids, which the observations show.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_git.rs`, over repositories the test builds in a
temporary directory with the host's git; the launches are delegated cases under
`systemd-run --user --scope -p Delegate=yes`, skipping naming the missing prerequisite (bubblewrap,
a user systemd manager, the driver, git), as `crates/loom-intake-slice/tests/confined_child.rs`
does. Each case named after one in `crates/assertion-providers/tests/git_changed*.rs` at `0.16.0`
is carried from it. "Refused X": `ObserveRun.outcome` `Refused`, `ObserveRun.refusal` X, the step
refused naming it, no evidence record.

- `paths_between_base_and_subject_are_matched_by_glob`: `CHANGELOG.md` changed gives the record's
  row for `true`, not changed the row for `false`; `docs/*.md` stays in one segment, `docs/**` and
  `**/*.rs` cross segments.
- `a_base_that_is_not_an_ancestor_does_not_credit_the_change_with_the_bases_edits`:
  `ObserveRun.revisions.fork` is the merge base, and the row is `false` for a path only the base
  changed.
- `a_subject_inside_the_base_compares_the_two_trees`: with `subject` an ancestor of `base`,
  `ObserveRun.revisions.fork` equals `subject`, and the row is `true` for a path the two trees hold
  differently (the diff runs from `base`).
- `a_missing_revision_or_repository_is_unavailable_not_false`: no `base` or `subject` in the
  context gives `ObserveRun.outcome` `Unavailable`, `reason` `no base revision` or `no subject
  revision`, and the table's `unavailable` row.
- A revision git cannot resolve: `Unavailable`, `reason` `git rev-parse failed with exit 1: ` (the
  launch name and exit code, with git's empty stderr).
- A workspace with no repository, and a shallow clone missing the base: each `Unavailable`, `reason`
  naming the failed subcommand and its exit code.
- Histories with no common ancestor: `Unavailable`, `reason` `git merge-base failed with exit 1: `.
- A linked-worktree workspace: `Unavailable`, `reason` naming `rev-parse` and its exit code
  (`story:observe-git-in-a-linked-worktree` replaces this case).
- `a_revision_that_looks_like_an_option_is_refused_before_git_runs`: an empty revision, one over 256
  bytes, starting with `-`, holding `..`, or a byte outside `[A-Za-z0-9._/~^@-]`: refused
  `RevisionInvalid`, and the counting launcher counts no launch.
- `a_glob_that_can_never_match_a_git_path_is_refused`: empty, over 1,024 bytes, a NUL, or an empty,
  `.` or `..` segment: refused at load, `BindingsRefusalReason` `InvalidObserveArguments` naming
  `glob`, when literal; refused `GlobInvalid` with no launch when it reaches the step otherwise.
- `a_non_utf8_path_in_the_change_does_not_abort_collection`: `ObserveRun.outcome` `Known`.
- `renames_deletions_odd_names_and_an_empty_range`: the row for each case as upstream's test gives.
- `a_root_in_a_subdirectory_matches_paths_relative_to_that_root`.
- `the_repository_configuration_cannot_run_a_program`: a workspace configuration naming an
  fsmonitor command or a hook path leaves no marker file.
- `a_promisor_remote_in_the_repository_configuration_cannot_run_a_program`: a promisor remote in
  the configuration leaves no marker file and the loopback listener it names accepts no connection.

The launch port and the port build:

- `a_reader_only_port_refuses_a_document_binding_git_changed`: `ObservePortRefused.refusal`
  `NoLauncher`, and no `ObserveRun` exists.
- `git_not_on_the_search_path_refuses_the_port`: `ObservePortRefused.refusal` `GitNotAdmitted`,
  `program_refusal` `ProgramNotFound`.
- `a_document_without_git_operations_needs_no_git`: a port built with a launcher and a host search
  path holding no git, for a document binding only `text.contains`, is built, and the action's
  `ObserveRun.outcome` is `Known`.
- `a_git_rewritten_after_admission_refuses_the_step`: refused `ProgramChanged`.
- `git_runs_the_admitted_file`: each `ChildCommand` the recording launcher received has the
  admitted git's mount path as its program and that file as its one read-only file.
- `no_file_or_document_operation_starts_a_process` (story 1) passes with the counting launcher
  added.
- `every_host_git_command_goes_through_a_hardened_helper` passes with
  `crates/loom-bindings/src/observe/git.rs` in its helper list: the file exists, starts git in
  exactly one place and sets both flags (`host_git_hardening.rs:490-500`), so the list's new entry
  fails the test until this story's helper is written.

Confinement and limits:

- Each launch's applied confinement (read from its `TestExecution`) has no network.
- Each launch's applied confinement has no writable path but `target/`.
- Each `ChildCommand` the recording launcher received has, as its `environment()`
  (`confinement.rs:1343`), exactly the seven variables above (the applied confinement carries no
  environment to read back).
- A launch the runner refuses (driver unserved, scope invalid, or a child ending with no exit
  status): refused `LaunchRefused`, `ObserveRun.reason` the runner's refusal; no record with a
  `false` row.
- The four launches share one deadline, the action's `timeout_seconds` (default 10 s, at most 300 s,
  story 1); past it (a test with a fake launcher that blocks and a 1 s timeout): `ObserveRun.outcome`
  `Unavailable`, `reason` `deadline`.
- Output cut at Substrate's output limit (1 MiB at `0.21.0`, `crates/substrate-host/src/lib.rs:528`;
  a fake launcher returns a cut `ChildOutput`): `ObserveRun.outcome` `Unavailable`, `reason`
  naming the limit; never a partial list read as `false`.
- The whole process tree ends with the launch (Substrate's `whole_tree_kill` fact, `AGENTS.md`
  § Intake and the command line): a case whose git leaves a background child finds no process of the scope after
  the launch.
- A git exit other than 0 with stderr longer than 200 characters: `ObserveRun.outcome`
  `Unavailable`, `reason` the subcommand, the exit code and the first 200 characters of git's first
  stderr line.
- `observe_reads_only_through_the_reader` (story 1) still finds no `std::process::Command` under
  `crates/loom-bindings/src/observe*`.
- `CHANGELOG.md` `### Added`: `git.changed`, `ObserveContext` and the observe port's launcher;
  `### Changed`: `**Breaking:**` for the new enum variants.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs` (the port's launcher member, the per-run context and the
dispatch), `crates/loom-bindings/src/observe/launch.rs` (new, `ObserveLauncher` and its
implementation over the runner), `crates/loom-bindings/src/observe/git.rs` (new),
`crates/loom-bindings/src/decode.rs` (the `git.changed` spelling), `crates/loom-bindings/src/load.rs`
(the `glob` argument and its literal check), `crates/loom-bindings/src/admission.rs` (called, not
changed: `admit`, `launch`, `verify`), `crates/loom-bindings/Cargo.toml` (`[dev-dependencies]
tempfile = "3"`, already in `Cargo.lock`; the lock changes only in `b10x-loom-bindings`' list),
`Cargo.lock`, `crates/loom-bindings/tests/observe_git.rs` (new),
`crates/loom-bindings/tests/observe_files.rs` (the counting launcher),
`crates/loom-intake-slice/tests/host_git_hardening.rs` (the helper list), `AGENTS.md` § Intake and
the command line, `CHANGELOG.md`.

## Out of scope

`story.id` and `story.has_tag`: trusted context, not collected; `story:gate-run-collects-a-gates-document`
hands them to Canon. Where a host takes `base` and `subject` from (§ Open 2 of the ledger).
Reading a file at a revision (`story:gate-run-collects-a-gates-document` adds `git cat-file` through
the same helper function).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (a new `observe/` launch port and the git operation), with ESS in `ess/domains/{bindings,observation}.yaml` first — cited (Files section)
- **Files:** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/launch.rs` (new), `crates/loom-bindings/src/observe/git.rs` (new), `crates/loom-bindings/tests/observe_git.rs` (new), `crates/loom-bindings/tests/observe_files.rs`, `crates/loom-intake-slice/tests/host_git_hardening.rs` (helper list, `:479-481`) — cited
- **Files read, not changed:** `crates/loom-bindings/src/admission.rs`, `crates/loom-intake-slice/src/confinement.rs:475` (`SubstrateRunner::run_child`) — cited
- **Symbols:** `ObserveLauncher`, `ObserveOperation::GitChanged`, `ObserveContext`, `ChangeContext`, `ResolvedChange`, `ObserveRefusal::{RevisionInvalid,GlobInvalid,ProgramChanged,LaunchRefused}`, `ObservePortRefusal::{NoLauncher,GitNotAdmitted}`, `ObservePortRefused.program_refusal`, `ObserveRun.revisions`, `admission::admit`, `ChildCommand`, `every_host_git_command_goes_through_a_hardened_helper` — cited
- **Also likely:** `crates/loom-bindings/src/decode.rs` (spells `git.changed`) and `crates/loom-bindings/src/load.rs` (load-time refusal of a literal bad `glob`) — inferred. The story's Files omit them. Story 1 lists both for its own operations, and the "refused at load ... naming `glob`" bullet needs them.
- **Also likely:** the glob matcher and validators live in `observe/git.rs` — inferred. Upstream's are `pub(crate)` at `git.rs:~290` (tag `0.16.0`), and Loom no longer depends on engineering-protocols, so Loom carries its own copy.
- **Documents:** none beyond the house-rule files (`AGENTS.md` § Intake and the command line, `CHANGELOG.md`) — cited
- **Confidence:** high for the file set, since the story names every file and the code it cites checks out except the points under Story defects. Medium for `decode.rs` and `load.rs`.
- **State of the tree:** `observe.rs`, `observe/`, `admission.rs` and `ess/domains/observation.yaml` do not exist at `61fd500b`. The predecessor stories create them, so every line of this story is written against files that do not exist yet — cited (`ls crates/loom-bindings/src` shows `decode.rs lib.rs load.rs tree.rs`)
- **Would collide with, ordered by a `depends_on` path (this story waits for them):** `story:observe-executor-reads-workspace-files` (observe.rs, observation.yaml, bindings.yaml, observe_files.rs, decode.rs, load.rs), `story:report-formats-checked-before-the-table`, `story:observe-reads-documents`, `story:observe-compares-generated-trees` (same four), `story:gate-program-identity-bound-to-spawn` (bindings.yaml, admission.rs), `story:bound-program-result-table` (bindings.yaml, `crates/loom-bindings` as a directory), `story:engineering-protocols-vendor-command` (host_git_hardening.rs) — cited, graph
- **Would collide with, ordered the other way (they wait for this story):** `story:gate-run-collects-a-gates-document` (direct edge; `observe/launch.rs`, `observe/git.rs`, both yamls, `observe.rs`), `story:gates-document-evaluated-in-the-governor` (direct edge; generated tree), `story:tcp-reachability-through-an-aperture` and `story:dns-resolution-through-a-resolver-aperture` (observe.rs, observation.yaml, bindings.yaml; ordered through gate-run) — cited, graph
- **Would collide with, no `depends_on` path either way:**
  - `crates/loom-intake-slice/tests/host_git_hardening.rs`: `story:confinement-level-on-oci-and-pod`, `story:executable-descendants-held`, `story:ingress-routes-per-action` and `story:substrate-daemon-on-another-host` all edit the Substrate pin assertion (`:549-552`). This story edits the helper list (`:479-481`). Same file, different hunks — cited
  - `ess/domains/bindings.yaml`, scope entries recorded: `story:capacity-given-to-an-admission-program`, `story:bound-artifact-revisions`, `story:agent-node-run-records`, `story:tree-usage-and-model-budget`, `story:bound-agent-node`, `story:record-copy-between-cases`, `story:run-caps-per-arrival`, `story:child-case-own-workspace`, `story:concurrent-cases-in-the-tree-daemon` — cited. This story adds one enum variant, so the conflict is textual only.
  - `ess/domains/bindings.yaml`, body only: `story:action-limits-enforced`, `story:agent-session-through-substrate`, `story:autonomy-grant-table`, `story:confinement-level-required`, `story:engine-configuration-document`, `story:limit-hits-in-run-log`, `story:named-egress-per-action` — cited
  - `crates/loom-bindings/src` as a directory: `story:child-case-runtime` — cited
  - `load.rs` and `decode.rs`, if the inferred edits hold: `story:capacity-given-to-an-admission-program`, `story:child-case-own-workspace`, `story:run-caps-per-arrival` — cited (their scope entries)
- **Contradicted by the code:** see Story defects 1-3
- **Safety fact:** `observe/git.rs` is the only file that starts git, because the hardening scan fails any other `Command::new(` naming git (`host_git_hardening.rs:444-500`). The scan already names the three existing helpers (`:479-481`). Level 2, unproven.
