---
format: aep.planning-md/3
id: story:child-case-own-workspace
kind: story
status: draft
title: An open executor can give each child case its own workspace directory, and no two open cases of a tree share one by accident
summary: ExecutorSpec.workspace, a JSON pointer to an absolute existing directory; refused on overlap; kept in CaseLink; Loom never creates or removes it
tags:
- sdc
relations:
- decomposes: epic:concurrent-and-standing-case-trees
- serves: vision:O1
- depends_on: story:child-case-runtime
- depends_on: story:tree-run-command
- depends_on: story:standing-tree-wakes-at-a-host-instant
revision: 2
---
## Outcome

An `open` executor in a `loom.bindings/1` document may name `workspace`: a JSON pointer into the
child's entry (with `each`) or into the arguments (with `name`), whose value is the child's
workspace directory, an absolute path. The child then runs in that directory: its placed artifacts
resolve under it, its programs and agent sessions get it as their workspace, and its own `open`
children inherit it unless they name one. A child opened without `workspace` shares its parent's,
as `story:child-case-runtime` says today.

The opening refuses, before any child is created and with the parent's case file byte-identical,
`WorkspaceInvalid` naming the child and the reason: the value is absent or not a string, not
absolute, not an existing directory, or, after symbolic links are resolved, equal to, inside or
containing the workspace of another case of the tree that is not terminated (the root's included;
a child that inherits its parent's workspace is the same case for this rule, not an overlap). The
directory is kept in the child's `CaseLink`, written in the same store write as the opening, so a
reopened store and a restarted daemon continue the case in it. `tree status` names each case's
workspace. A step of a case whose workspace no longer exists is refused `WorkspaceMissing` naming
the case and the path, and the case is stuck. Loom never creates, empties or removes the
directory: after the child terminates it stays as it was.

With its own workspace, a file the child writes moves no artifact of its parent; the parent learns
of the child through its `case_outcome` record only.

## Why

"each story in its own work tree" (`.engineering/drafts/e4-request.md`, item 2); "Each admitted
child gets its own managed worktree from `worktree create` and its own build directory ...,
replacing the shared `work/`" (`~/beyond10x/sdc/docs/orchestration.md` § 2.4). Today
`story:child-case-runtime` § Outcome says "The cases of one tree share the root's workspace", and
its § Out of scope lists "A workspace per child (`story:child-case-own-workspace`)";
`story:bound-artifact-revisions` § Out of scope lists "One workspace per child case". Two
concurrent writers in one workspace would race on the placed paths, so this story is what makes
`story:concurrent-cases-in-the-tree-daemon` useful. Loom does not make the work tree: in the tree
the arguments come from the binding (a literal or an `arguments_from` program), never from a model
(`story:tree-run-command`'s model-free executor), so the SDC's `arguments_from` program can run
`worktree create` and print each story's directory; confined, it needs the common `.git` writable,
which is `story:writable-host-root` (accepted by session `sdc` on 2026-11-21; no edge here). The
`open` executor today takes `name`, `each` and `bindings` (`loom.bindings.ExecutorSpec`,
`ess/domains/bindings.yaml:24-58`), and every path the loader checks is relative to the workspace
(`crates/loom-bindings/src/load.rs:286-292`).

## Waits on

- `story:child-case-runtime`: the `open` effect port in `crates/loom-bindings`, `CaseLink` in
  `ess/domains/governor.yaml` and the tree listing this story extends.
- `story:tree-run-command`: `crates/loom-tree`, `TreeStatus` in `ess/domains/tree.yaml`, and the
  root's workspace (its flag not read; the story names none).
- `story:standing-tree-wakes-at-a-host-instant`: the story before this one on
  `ess/domains/tree.yaml` (`TreeStatus`) and `crates/loom-tree`; through the chain also
  `story:run-caps-per-arrival`, the earlier story on `ess/domains/bindings.yaml` and
  `crates/loom-bindings/src/load.rs`.

Comes before, on shared files: `story:concurrent-cases-in-the-tree-daemon`
(`ess/domains/bindings.yaml` `WorkspaceInvalid`, `ess/domains/tree.yaml` `TreeStatus`,
`crates/loom-tree`), and through it `story:capacity-given-to-an-admission-program`
(`ess/domains/bindings.yaml`, `crates/loom-bindings/src/load.rs`).

## ESS first

`ess/domains/bindings.yaml`: `ExecutorSpec` gains `workspace` (optional JSON pointer, `Open`
only), the refusals `WorkspaceInvalid` (child, path, reason `Absent`, `NotAbsolute`,
`NotADirectory`, `Overlaps` with the other case) and `WorkspaceMissing` (case, path).
`ess/domains/governor.yaml`: `CaseLink` gains optional `workspace`. `ess/domains/tree.yaml`: the
per-case entry of `TreeStatus` gains `workspace`. On that commit `task drift` fails and
`crates/loom-cli/tests/tree_workspaces.rs` does not compile.

## Acceptance

With files added to `crates/loom-cli/tests/fixtures/area/`: `wave.protocol.yaml` (`wave/1`, one
`case_open` action opening `item/1` children from an `each` argument), `item.protocol.yaml`
(`item/1`: a writer of a placed `out` file and a `case_open` action opening one `step/1` child),
`step.protocol.yaml` (`step/1`: a writer of a placed `step-out` file), their bindings (the wave's
`open` with `workspace: /dir`, the item's `open` without one), an `arguments_from` stub printing
three entries each naming a directory under the test's temporary directory, and stub programs. No
model. `crates/loom-cli/tests/tree_workspaces.rs` drives `tree run`:

- Each item's writer creates `out` in that item's directory only; each item's artifact revision
  moves while the root's artifact at the same placed path does not.
- Each item's grandchild (`step/1`, opened without `workspace`) writes `step-out` into its item's
  directory, and `tree status` names the item's directory for the grandchild.
- An item opened without `workspace` (a second bindings file) writes into the root's workspace and
  moves the root's artifact at the next evaluation, as `story:child-case-runtime`'s acceptance has
  it.
- An entry naming a relative path, a missing directory, a file, the root's workspace, a directory
  inside another open item's, or a symbolic link resolving to another open item's directory is
  refused `WorkspaceInvalid` with its reason; no child case file is written and the root's case
  file is byte-identical.
- A directory of an item that has terminated may be named again by a new item.
- After the store is reopened, an item's next step runs in its directory; `tree status` names it.
- With an item's directory removed between two steps, the next step is refused `WorkspaceMissing`
  and `tree run` exits 4 naming the case.
- After an item terminates, its directory and every file in it are unchanged.

`crates/loom-tree/tests/workspace_agent.rs` drives the tree library with a stub agent adapter
registered by the test (no model) bound to the item's writer: the adapter is handed the item's
directory as its workspace, and its output lands there.

`crates/loom-bindings/tests/bound_open_workspace.rs`: a bindings file with `workspace` on a
`program` executor, or a value that is not a JSON pointer, is refused at load naming the action and
the member.

- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/governor.yaml`, `ess/domains/tree.yaml`, generated
`generated/rust/loom/src/{bindings,governor,tree}.rs`; `crates/loom-bindings/src/load.rs`,
`crates/loom-bindings/src/decode.rs` and the `open` port's module (named by
`story:child-case-runtime`, not read); `crates/loom-governor/src/file_store.rs` (the `CaseLink`
codec); `crates/loom-tree` (a case's workspace, `tree status`);
`crates/loom-cli/tests/tree_workspaces.rs`, `crates/loom-tree/tests/workspace_agent.rs`,
`crates/loom-bindings/tests/bound_open_workspace.rs` (new); `crates/loom-cli/tests/fixtures/area/`
(adds `wave.protocol.yaml`, `item.protocol.yaml`, `step.protocol.yaml`, their bindings and stubs);
`CHANGELOG.md`.

## Out of scope

Running cases at once (`story:concurrent-cases-in-the-tree-daemon`). Making, merging or removing a
Git work tree. A writable `.git` for a confined program (`story:writable-host-root`). The daemon's
rule that a workspace sits directly under its `workspace_root`
(`story:concurrent-cases-in-the-tree-daemon` checks it at the opening).
