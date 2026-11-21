---
format: aep.planning-md/3
id: story:observe-git-in-a-linked-worktree
kind: story
status: proposed
title: git.changed answers in a linked git worktree, its common git directory mounted read-only
summary: The host reads the worktree's .git file and commondir, mounts the common git directory read-only at /repository for each git launch; what the mount exposes (all refs and objects, config with remote URLs and credential helpers) is a security point the operator settles
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-git-changed
- depends_on: story:gate-run-collects-a-gates-document
scope:
- confidence: cited
  path: crates/loom-bindings/src/admission.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/git.rs
- confidence: cited
  path: crates/loom-bindings/tests/gate_run.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_git.rs
- confidence: cited
  path: ess/domains/observation.yaml
- confidence: cited
  path: generated/rust/loom
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T04:08:01Z", actor: "agent:sdc", revision: 8, decided_on: {"recorded":{"review_outcome":4}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`git.changed` (`story:observe-git-changed`) and the gate run's git launches
(`story:gate-run-collects-a-gates-document`) work in a workspace that is a linked git worktree,
as they do in an ordinary clone, instead of answering `Unavailable`. Every SDC story runs in a
managed linked worktree (session `sdc`, `.engineering/drafts/e7-sdc-answers-2.md`), so without this
story every story check that reads `git.changed` or runs a gate is blind.

By git's layout (`gitrepository-layout(5)`: `commondir`, `worktrees/<id>/gitdir`), a linked
worktree's `.git` is a file naming the worktree's own git directory. That directory's `commondir`
names the repository's common git directory, which holds the objects, the refs and `config`, and
its `gitdir` holds the path back to the `.git` file that points to it, which git writes when it
makes the worktree. All of these lie outside the workspace, so the sandbox does not show them. A
child sees only the host paths the host names (`crates/loom-intake-slice/src/confinement.rs:1029-1036`,
which also says such a directory can be given read-only only).

How the host finds and checks the common directory, before the run's first git launch:
`admission::linked_worktree(workspace_root)` (`crates/loom-bindings/src/admission.rs`), called from
the launch function of `crates/loom-bindings/src/observe/git.rs` once per run. The
`WorkspaceReader` cannot read these files, because a path with a `.git` component is
`resource.not-found` to Substrate (`crates/substrate-guarded/src/fs.rs:955-958`, Substrate
`0.21.0`). Every file is opened with `O_NOFOLLOW | O_NONBLOCK`, as `admission::admit` opens a program
(`story:gate-program-identity-bound-to-spawn`), and at most 4 KiB of each is read. In order:

1. `lstat` of `<root>/.git`, before any open:
   - nothing there: no git directory at all, so story 6's path runs and git answers `Unavailable`;
   - a directory: an ordinary clone, so story 6's path runs unchanged;
   - a regular file: go on;
   - anything else (a link, a FIFO, a socket, a device): refused.
2. Read `<root>/.git`: one line `gitdir: <path>`, a relative path taken against the root. Its
   canonical path is the worktree's git directory `W`. Canonicalizing follows links, so a link in
   the workspace that leads outside is followed here, and step 5 decides.
3. Read `W/commondir`: one line, relative to `W` or absolute. Its canonical path is the common
   directory `C`.
4. `W` must be `C/worktrees/<id>`, with `<id>` one component.
5. Read `W/gitdir`, the back-pointer, and compare it with `<root>/.git` lexically, following no
   link:
   - `<root>` is the canonical workspace root the host already holds;
   - an absolute back-pointer is taken as written; a relative one, which git writes with
     `worktree.useRelativePaths` or `--relative-paths` (`git-config(1)`, `git-worktree(1)`;
     `gitrepository-layout(5)` documents the absolute form), is relative to the directory holding
     the file, `W`, as git's `get_linked_worktree()` reads it (mirrored by `gix-discover` 0.55.0,
     `src/path.rs:50`, `:114`);
   - the absolute text is normalised lexically: `.` and empty components dropped, each `..` removing
     the component before it, no link resolved;
   - the result must equal `<root>/.git` byte for byte.

   Lexical equality is enough because of step 1: the host opened `<root>/.git` itself with no
   link followed and found a regular file. `<root>` is canonical, so the one path that passes names
   that file and holds no link anywhere. Git writes the back-pointer in the repository that owns the
   worktree, so a `.git` file a run wrote cannot claim another repository's entry. A link the run
   puts in the workspace cannot make another repository's back-pointer pass: that would need
   following links, and following links is exactly what that attack needs. The limit: a
   back-pointer written through a non-canonical but equivalent absolute path (git run under a
   linked home directory, for example) is refused `LinkedWorktreeRefused`.
6. `C` must not be inside the workspace and must not hold it.

Every case where `.git` or a file it leads to is present and not acceptable refuses the step with
`ObserveRefusal` `LinkedWorktreeRefused`, `reason` naming the step and the path, and no launch:

- a link or a special file at `.git`;
- a `.git` file that is unreadable, over 4 KiB, or has no `gitdir:` line;
- a `gitdir:` naming a missing path, or a directory with no `commondir` (`/`, a directory that is
  no git directory, a submodule's git directory);
- a `commondir` that is missing, unreadable or over 4 KiB;
- a layout other than step 4;
- a back-pointer that is missing or names another `.git`;
- a `C` inside the workspace.

`Unavailable` stays only for "no git directory at all" (step 1, nothing there), as story 6 has it.

What remains possible after the check: a run that can write `<root>/.git` can still make Loom mount
the common directory of any repository holding a worktree entry whose back-pointer is exactly
`<root>/.git`. That is the repository this worktree belongs to, or another repository whose worktree
once lived at this same path and whose stale entry was never pruned. A link in the workspace
reaches no other (§ Exposure).

Each git launch of the run then gets `C` as a read-only root at `/repository`
(`ChildCommand::with_read_only_root`, `confinement.rs:1111-1136`), plus three environment variables:
`GIT_DIR=/repository/worktrees/<id>`, `GIT_COMMON_DIR=/repository` and `GIT_WORK_TREE=/workspace`.
The mount is fixed so that host paths stay out of the child's view, and so that it is never on a
path Substrate owns, such as `/tmp` (`confinement.rs:1121-1124`). The runner still checks the root
before anything starts (`confinement.rs:2253-2274`). With step 6 done and `C` canonical, its only
remaining case is a `C` that holds the driver's state directory; it is recorded `LaunchRefused`, as
story 6 records any runner refusal. Substrate opens the root once and binds the descriptor it opened
(`--ro-bind-fd`, `confinement.rs:2270-2274`), so a directory put in its place afterwards is not
bound. A directory swapped between Loom's check and Substrate's open is bound instead, and the run's
`ObserveRun.revisions` name the object ids it resolved there.

Writes: the commands used are read-only. Story 6's `rev-parse`, `merge-base` and `diff-tree`
compare objects and trees and never read or refresh an index, and story 8's `cat-file` prints a
blob. None takes `index.lock` or writes the worktree's entry; story 6's `GIT_OPTIONAL_LOCKS=0`
already turns off the optional locks. Any write under `/repository` fails, because the mount is
read-only.

Ownership: git accepts `/repository` without a `safe.directory` refusal when `C` is owned by the user
Loom runs as. Substrate unshares the user namespace (`--unshare-user`,
`crates/substrate-host/src/process.rs:74`) and names no other uid, so the child keeps the caller's
uid, as for story 6's `/workspace`. This story adds no `safe.directory` setting. A `C` owned by
another user is refused by git's own ownership check, which story 6 records as `Unavailable` with
git's reason, because loosening that check is not this story's to do.

A `config` read from the mount runs nothing, by the parts of story 6's hardening that do not depend
on where the file lies:

- the `-c` overrides on every launch (`core.fsmonitor=false`, `core.hooksPath=/dev/null`,
  `diff.external=`, `core.sshCommand=false`, `protocol.allow=never`), which as command scope take
  precedence over the repository's `C/config` and the worktree's `W/config.worktree`;
- `GIT_NO_LAZY_FETCH=1` against a promisor remote;
- no network.

The hardening scan requires the helper to set the first two (`host_git_hardening.rs:497`). The
environment is story 6's seven variables, plus the three above in a linked worktree only. Every
launch still goes through the one launch function, which builds `ChildCommand::new(git.mount_path(), …)`
and hands it to `admission::launch`, so `every_host_git_command_goes_through_a_hardened_helper`
counts one git start. The root and the three variables are added to that command, never by a second
start.

## Exposure: a security point for the operator

Not settled here. Recorded as § Open 1 of the e7r5 ledger (`.engineering/drafts/e7r5-changes.md`),
carried from § Open 21 of `e7r3-changes.md`. Settles: the operator.

The mount shows the whole common directory of this worktree's repository, read-only, to the git
child:

- objects, and every ref: branches and tags (`refs/`, `packed-refs`), the stash, `logs/` (reflogs)
  and `FETCH_HEAD`;
- `config`, with any remote URL (which can carry a token) and any `credential.helper` line;
- `hooks/`: the hook scripts can be read, though none runs;
- `info/`: `exclude`, `attributes`, and any `grafts` or sparse-checkout file;
- `modules/`: each submodule's whole git directory, with its own `config`;
- `worktrees/`: every other linked worktree's entry (its `HEAD`, index, `gitdir` back-pointer naming
  that worktree's host path, and `config.worktree`).

`config` is not filtered; this story states the limit instead. Git reads `$GIT_COMMON_DIR/config`
itself, and no variable skips the repository's file: `GIT_CONFIG_NOSYSTEM` and `GIT_CONFIG_GLOBAL`
cover only the system and global ones. A filtered copy cannot be mounted over it, because Loom
refuses a mount under another root's mount (`confinement.rs:1125`). Copying the common directory per
run would copy every object. An ordinary clone already shows its child the same `.git` inside the
workspace.

What a run that can write `<root>/.git` reaches: the common directory of a repository whose
worktree entry names exactly `<root>/.git`, meaning this worktree's repository or one with a stale
entry for a worktree once at this path. Step 5 refuses every other, a back-pointer reached through
a workspace link included.

What leaves the child:

- the object ids and path names of story 6's launches;
- for the gate run, the content of the gates document and the project file at the base, as
  `cat-file` prints them (kept in the gate run's plan and bundle);
- at most 200 characters of git's first stderr line in `ObserveRun.reason`.

The child has no network (story 6), so a credential it can read has no way out but those.

## Why

`story:observe-git-changed` answers `Unavailable` for a linked worktree and points here. Session
`sdc` asked for this story under its standing rule: after the epic's first stories, and not
blocking the epic's move to proposed. Upstream runs git as a host process
(`crates/assertion-providers/src/git.rs:216-255`, tag `0.16.0`), which sees the common directory
without asking; Loom's git runs in Substrate, so the directory must be named.

## Waits on

- `story:observe-git-changed`: the launch function, the environment and `observe_git.rs`.
- `story:gate-run-collects-a-gates-document`: it adds `git cat-file` to the same launch function and
  writes `gate_run.rs`, which this story's gate-run bullet extends.
- `story:gate-program-identity-bound-to-spawn`, through `story:observe-git-changed` and not
  directly: it creates `admission.rs`, which this story extends. If that path is ever dropped, this
  story needs its own edge.

`story:tcp-reachability-through-an-aperture` waits on this story (an edge in the store), and
`story:dns-resolution-through-a-resolver-aperture` through it: the three share `CHANGELOG.md`,
`ess/domains/observation.yaml` and `generated/rust/loom/`.

## ESS first

`ess/domains/observation.yaml`: `ObserveRefusal` gains `LinkedWorktreeRefused`. On that commit
`task drift` fails and `crates/loom-bindings/tests/observe_git.rs` does not compile. The mounted
root needs no declaration: it is in the applied confinement the driver reports.

## Evidence record

As `story:observe-git-changed` § Evidence record: in a linked worktree the value depends on the
resolved object ids and the glob, as in a clone.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_git.rs`, unless named otherwise. They are delegated
cases with story 6's prerequisites, over a repository and a worktree the test makes with
`git worktree add` under the crate's target directory (not under `/tmp`, which Substrate owns).
"Fake" cases run on story 6's recording launcher. "Refused" means `ObserveRun.outcome` `Refused`,
`ObserveRun.refusal` `LinkedWorktreeRefused`, the step refused naming it, no evidence record, and
the recording launcher counts no launch.

Reads:

- `a_linked_worktree_answers_git_changed`: with `CHANGELOG.md` changed between `base` and
  `subject`, the record's row is `true` and `ObserveRun.outcome` is `Known`. It replaces story 6's
  linked-worktree `Unavailable` case.
- `a_gate_run_in_a_linked_worktree_reads_the_base` (`crates/loom-bindings/tests/gate_run.rs`): a
  gates document committed at the base and edited in the worktree gives the base version's gate
  result. The `rev-parse` and `cat-file` launches each have `C` at `/repository` in their applied
  confinement.
- `the_common_directory_is_mounted_read_only`: each of story 6's launches has `C` at `/repository`
  in its applied confinement (`TestExecution.applied.read_only_roots`, `confinement.rs:83`,
  `:861`), and no writable path but `target/`.
- `a_write_through_the_mount_fails`: a delegated case runs `/usr/bin/touch
  /repository/loom-write-probe` with the same root; it exits non-zero, and no file
  `loom-write-probe` exists in `C` afterwards.
- `a_linked_worktree_launch_has_ten_variables` (fake): each `ChildCommand` the recording launcher
  received has, as its `environment()` (`confinement.rs:1343`), story 6's seven variables and the
  three above, with `GIT_DIR` naming the worktree's `<id>`.
- `an_ordinary_clone_gets_no_mount` (fake): in a clone, each command has no read-only root and
  exactly story 6's seven variables.
- `no_git_directory_is_unavailable`: a workspace with no `.git` gives `ObserveRun.outcome`
  `Unavailable`, as story 6 has it.

The workspace tie (each refused):

- `another_repositorys_worktree_is_refused`: a second fixture repository with its own linked
  worktree. The workspace's `.git` is rewritten to `gitdir: <second>/.git/worktrees/<its id>`;
  steps 2 to 4 pass, and the back-pointer names the second worktree's `.git`.
- `a_gitdir_naming_the_root_directory_is_refused`: `gitdir: /`.
- `a_gitdir_naming_no_git_directory_is_refused`: `gitdir:` naming an empty directory outside the
  workspace.
- `a_workspace_link_to_an_outside_git_directory_is_refused`: `gitdir: wt-link`, where `wt-link` is a
  link in the workspace to the second repository's `worktrees/<its id>`.
- `a_foreign_back_pointer_through_a_workspace_link_is_refused`: a second fixture repository whose
  worktree entry's `gitdir` names `<root>/nested/.git`, a path inside this workspace. The run puts
  a link at `<root>/nested/.git` to `<root>/.git`, and `<root>/.git` holds
  `gitdir: <second>/.git/worktrees/<its id>`. Steps 2 to 4 pass; the canonical back-pointer would
  be `<root>/.git`, the lexical one is not. Refused, and the recording launcher receives no
  command, so nothing is mounted.

The layout (each refused):

- `a_link_at_dot_git_is_refused`: `.git` is a link to a valid `.git` file of another worktree.
- `an_oversized_git_file_is_refused`: a `.git` file of 4 KiB + 1 byte.
- `an_unreadable_git_file_is_refused`: a `.git` file with mode `000`.
- `a_git_file_without_gitdir_is_refused`: a `.git` file holding `hello`.
- `a_gitdir_naming_a_missing_path_is_refused`.
- `a_missing_commondir_is_refused`: the worktree entry's `commondir` deleted.
- `a_layout_outside_worktrees_is_refused`: the worktree entry moved to `C/elsewhere/<id>`, with
  `.git` and `commondir` adjusted.
- `a_common_directory_inside_the_workspace_is_refused`: a repository made inside the workspace at
  `<root>/inner`, with a worktree entry `inner/.git/worktrees/w`. Its `commondir` is `../..`, its
  `gitdir` names `<root>/.git`, and `<root>/.git` holds `gitdir: inner/.git/worktrees/w`, so steps
  1 to 5 pass and step 6 refuses.

Configuration from the mount (each: no marker file, and the loopback listener accepts no
connection where one is named):

- `a_common_config_fsmonitor_runs_nothing`: `C/config` sets `core.fsmonitor` to a marker-writing
  script.
- `a_common_config_hook_runs_nothing`: `C/config` sets `core.hooksPath` to a directory with a
  marker-writing hook for every hook name.
- `a_common_config_promisor_remote_connects_nowhere`: `C/config` names a promisor remote at a
  loopback listener, in a partial clone missing an object the diff needs.
- `a_worktree_config_runs_nothing`: `W/config.worktree` (with `extensions.worktreeConfig`) sets
  `core.fsmonitor` and `core.hooksPath` to marker-writing programs.

The rest:

- `every_host_git_command_goes_through_a_hardened_helper` passes, not changed: still one git start
  in `observe/git.rs`.
- `CHANGELOG.md` `### Added`: `git.changed` and the gate run in a linked worktree, with the common
  git directory mounted read-only. `### Changed`: `**Breaking:**` for the new `ObserveRefusal`
  variant.
- `task check` exits 0.

## Files

`ess/domains/observation.yaml`, `generated/rust/loom/` (regenerated),
`crates/loom-bindings/src/admission.rs` (`linked_worktree`), `crates/loom-bindings/src/observe/git.rs`
(the call, the root and the three variables on the launch), `crates/loom-bindings/tests/observe_git.rs`,
`crates/loom-bindings/tests/gate_run.rs` (the gate-run bullet), `CHANGELOG.md`.

## Out of scope

Filtering `config` or the refs (§ Exposure, the operator's). A writable git directory. A worktree
whose git directory is not under `<common>/worktrees/`, and a submodule checkout's `.git` file
(its git directory has no `commondir`): both refused `LinkedWorktreeRefused`. A `safe.directory`
exception for a repository owned by another user.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings`, in `src/admission.rs` (`linked_worktree`) and `src/observe/git.rs` (the root and three variables on the launch) — cited (§ Files)
- **Files:** `crates/loom-bindings/src/admission.rs` — cited; not at HEAD, created by `story:gate-program-identity-bound-to-spawn`, which this story extends
- **Files:** `crates/loom-bindings/src/observe/git.rs` — cited; not at HEAD, created by `story:observe-git-changed`, then extended by `story:gate-run-collects-a-gates-document` (`cat-file`)
- **Files:** `crates/loom-bindings/tests/observe_git.rs` — cited; not at HEAD, created by `story:observe-git-changed`
- **Files:** `CHANGELOG.md` (exists at HEAD), `ess/domains/observation.yaml`, `generated/rust/loom/` (regenerated), `crates/loom-bindings/tests/gate_run.rs` (created by `story:gate-run-collects-a-gates-document`) — cited, § Files after revision e7r5; `AGENTS.md` left § Files in that revision
- **Read, not changed:** `crates/loom-intake-slice/src/confinement.rs` (`with_read_only_root` at `:1136`, exists at HEAD) — cited
- **Symbols:** `linked_worktree` (new), `ChildCommand::with_read_only_root`, `TestExecution.applied.read_only_roots`, `ChildCommand::environment`, `ObserveRun.outcome`, `ObserveRun.revisions` (planned by story 6), `every_host_git_command_goes_through_a_hardened_helper` (`host_git_hardening.rs:467`) — cited
- **Also likely:** none. Where `linked_worktree` is called (port build or per run) is not stated; if it is `observe.rs`, that file joins — inferred
- **Documents:** none beyond `CHANGELOG.md`. The ESS change is one `ObserveRefusal` member, `LinkedWorktreeRefused`, in `ess/domains/observation.yaml` (§ ESS first) — cited
- **Confidence:** high for the file set, since the story lists four code files and the two documents. Medium for the code shape, since every code file is created by a story that is not implemented.
- **Would collide with:**
  - `observe/git.rs`: `story:observe-git-changed` (direct `depends_on`) and `story:gate-run-collects-a-gates-document` (direct), both ordered before this story. No other story names the file.
  - `tests/observe_git.rs`: `story:observe-git-changed` only, ordered before.
  - `admission.rs`: `story:gate-program-identity-bound-to-spawn`, ordered before through `observe-git-changed`. `observe-git-changed` and `gate-run` only call it. No unordered story names it.
  - `AGENTS.md` § Intake: `story:observe-git-changed` edits it, ordered before. `tcp-reachability-through-an-aperture` and `dns-resolution-through-a-resolver-aperture` cite the section without a Files entry, and `reference-enrichment` (draft) cites it only.
  - `CHANGELOG.md`: about 250 story bodies name it. `story:tcp-reachability-through-an-aperture` and `story:dns-resolution-through-a-resolver-aperture` are ordered after this story, tcp by a direct `depends_on` edge and dns through tcp; the same edge orders the three on `ess/domains/observation.yaml` and `generated/rust/loom/`.
  - `story:tcp-reachability-through-an-aperture` depends on this story: one inbound edge.
- **Safety fact:** `observe/git.rs` stays the only git start, because the hardening scan fails any other `Command::new(` naming git (`crates/loom-intake-slice/tests/host_git_hardening.rs:464-500`) — step 2, unproven
