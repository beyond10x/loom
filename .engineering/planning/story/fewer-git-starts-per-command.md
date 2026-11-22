---
format: aep.planning-md/3
id: story:fewer-git-starts-per-command
kind: story
status: proposed
title: A Loom command starts git only when the workspace may have changed since its last hardening check
summary: 'a generation counter bumped by every workspace-writing effect lets host-git-hardening skip repeated rechecks: about 27 of 60 git starts; security code, adversary review'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/case.rs
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: cited
  path: crates/loom-intake-slice/src/git.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/host_git_hardening.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:01:36Z", actor: "agent:loom", revision: 5}
---
## Outcome

One `b10x-loom` command starts host git for a recheck only when something could have changed
since the last check of the same repository. In the traced scenario, `run` starts at most 23 git
processes in place of 40, and `resume` at most 12 in place of 20. No check is weakened.

**This changes security code**: the checks of `story:host-git-hardening`. The story is proposed
for the design authority's and Timo's review, and the unit gets an adversary review before it
merges. It is not on the Runbook path.

The form is a generation counter in `crates/loom-intake-slice/src/git.rs`:

- One process-wide counter. Every Loom effect that can write the workspace bumps it: the edit
  writer (`implementation.edit`, `tests.write`), the test runner (`tests.run`), the child runner,
  and the handoff's commit.
- Each `Recording` keeps the counter of its last full check. `recorded` (`git.rs:137-163`) runs
  the two checks that start a process, `git_dirs` (`:258`) and `refuse_program_keys` (`:232`),
  only when the counter moved since then.
- A second `record` (`:168`) of the same workspace, with no move, reuses the recording.

Checks that are never skipped:

- the file digest check of `recorded` (`:157-161`), which starts no process;
- every HEAD read: `case.rs:333` `show-toplevel`, `case.rs:349` and `executor.rs:1606`
  `verify HEAD`;
- the first check of each repository in a command;
- every check after an effect that can write the workspace;
- every check of a different repository path (the key is the canonical path, as `RECORDINGS` has
  it, `git.rs:121-128`);
- the git operation itself.

## Why

A strace of `resume_after_grant::resume_after_approve_merges_once` counted 60 git starts from
`b10x-loom`: 40 in `run` and 20 in `resume`.

- 27 of the 40 starts in `run` are rechecks: `record` twice, at 3 starts each, and `recorded` 9 to
  11 times, at 2 starts each (`git_dirs` at `git.rs:260` and `config --list` at `:234`).
- Each recheck repeats the same arguments in the same repository.
- `validate_workspace` (`case.rs:258`, called from `crates/loom-cli/src/main.rs:1208`) and then
  `open_declared` (`case.rs:236`) repeat `head` and `record` with no Loom effect between them.

Why a fresh check is needed after a write: an edit can rewrite an included config file to set
`core.pager`, `diff.textconv` or `filter.*.clean`. The next `git add` or `git diff` would then run
that program with the operator's rights.

Another process writing during the command is not closed by a recheck on every call either. The
gap between the check and git's own read is the same race. `story:host-git-hardening`'s threat is
the model and the test command.

About 27 of the 60 starts can go (inferred). That saves 0.1 to 0.2 s per test, 10 to 20 s of cli
test time, and 3 to 5 s of step wall time.

The target for `run` is 23, not 21, because every HEAD read stays and so `run` keeps the second
`head(Opening)` (2 starts). Alternative (b) reaches 21 by passing `validate_workspace`'s HEAD to
`open_declared`. That is for the design authority; the default (a) keeps the read.

## ESS first

Exempt: no noun or rule of the specification changes, only the number of processes behind the
same checks. The first commit adds the count test, red at 40 and 20.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call (the cli tests use the existing fake model). Each test is timed, and any test over 5 s is
named with its time.

- **The counted starts.** A counted fake `git` is put on `PATH` for the `b10x-loom` commands only.
  It is a POSIX sh script in the test's scratch directory that appends one line per start to a
  log, then `exec`s the real git found when the test starts. In the traced scenario, `run` starts
  at most 23 and `resume` at most 12. Test:
  `run_and_resume_start_at_most_23_and_12_host_git_processes`
  (`crates/loom-cli/tests/resume_after_grant.rs`).
- **A model edit.** The model's edit rewrites a work-tree file that `.git/config` includes so that
  it sets `core.pager`. The next checked call is refused `ProgramKey`, and the pager never runs.
  Planted defect: removing the edit writer's bump turns it red. Test:
  `an_edit_that_rewrites_an_included_config_is_refused_program_key`.
- **A test command.** `tests.run` replaces `.git` with a `.git` file that points elsewhere. The
  next check is refused `GitDirChanged`. Planted defect: the test runner's bump. Test:
  `a_test_command_that_moves_the_git_dir_is_refused_git_dir_changed`.
- **A child run.** The same through the child runner, with the child runner's bump as the planted
  defect. Test: `a_child_run_that_moves_the_git_dir_is_refused_git_dir_changed`.
- **A second record.** With no effect between, it starts no git. After an effect, it starts 3.
  Test: `a_second_record_reuses_the_recording_only_without_an_effect_between`.
- **A write between two checks.** Both checks start `git_dirs` and `config --list`, 4 starts in
  the counted log. Test: `a_second_check_after_a_workspace_write_still_starts_git`.
- **Another repository.** A check of another repository path is never skipped. Test:
  `a_check_of_another_repository_is_never_skipped`.
- **The digest check.** A changed `.git/config` with every bump planted out is still refused
  `ConfigChanged`. Test: `a_changed_config_digest_is_refused_without_a_bump`.
- **Existing tests.** The `host_git_hardening.rs` tests pass unchanged, among them
  `a_program_key_in_the_workspace_config_refuses_the_open` (`:313`) and
  `a_commondir_a_test_plants_is_refused` (`:346`).
- **Adversary review.** An adversary review of the unit is recorded before it merges. It names
  each bump site and checks that no other effect writes the workspace.

## Out of scope

- Another process writing the workspace during a command; the 13 starts of the test fixture;
  reusing a HEAD read, which is alternative (b).

## Scope

- `crates/loom-intake-slice/src/git.rs`, cited: `:121-128`, `:137-163`, `:168`, `:232`, `:258`,
  `:275`.
- `crates/loom-intake-slice/src/case.rs`, cited: `:225-251`, `:256-260`, `:333`, `:349`, `:405`.
- `crates/loom-intake-slice/src/executor.rs`: `:1126`, `:1654` and `:1606` are cited. The bump
  sites (edit writer, test runner, handoff commit) are inferred.
- `crates/loom-intake-slice/src/confinement.rs`: the child runner's bump site. Inferred.
- `crates/loom-intake-slice/tests/host_git_hardening.rs`, for the new tests. Cited.
- `crates/loom-cli/tests/resume_after_grant.rs:61`. Cited.

Edges:

- No depends_on. `story:tree-run-command` is not needed, and there is no edge to the Runbook
  stories.
- Collides with `story:drop-the-substrate-git-feature` only through `host_git_hardening.rs`,
  where that story changes the lock-file walk. Neither story touches the other's lines in
  `git.rs` or `case.rs`. Status: proposed, for the design authority and Timo's review.
