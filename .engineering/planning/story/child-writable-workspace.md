---
format: aep.planning-md/3
id: story:child-writable-workspace
kind: story
status: implemented
title: A confined child writes the workspace paths the host names
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:sdk-confines-any-child
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:33:51Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T04:33:52Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T08:39:26Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A host that confines a child through `SubstrateRunner::run_child` names which paths of the
workspace the child may write, beyond the build output: a harness child editing its scratch copy,
`git commit` in it, or a forge command writing its state. Every other confinement guarantee holds,
and the applied confinement reports the writable paths. Today a confined child writes only under
`target`, the test runner's writable set (story:sdk-confines-any-child, wave 2026-10-20-w2, U4
report).

## Why

takt's confined-harness-child story runs a harness child, `git`, its forge command and `aep` under
a Substrate profile through Loom (Atlas ADR 0099 point 4). Each of them writes in the workspace; with
writes limited to `target`, none of them can run confined.

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs::a_child_writes_only_the_paths_the_host_named`
  (delegated, skipped on a named missing prerequisite): a child writes a file in a named path and
  fails to write one outside it, and the applied confinement lists the named path.
- `…::a_writable_path_outside_the_workspace_or_through_a_link_is_refused`: refused before launch.
- `TestRunner::run` keeps `target` as its only writable path.
- `task check` exits 0.

## ESS first

No noun the specification lacks if the writable paths reuse Substrate's `WorkspaceAccess`; the red
step is `a_child_writes_only_the_paths_the_host_named`.

## Depends on

story:sdk-confines-any-child.

## Scope

- `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/tests/confined_child.rs`
- `CHANGELOG.md`, `AGENTS.md`

## Correction at close (wave 2026-10-20-w2)

- What shipped: ChildCommand::with_writable names existing workspace directories (up to 64, Substrate's scoped access); a single file cannot be made writable; .git is read-only unless named; every refusal happens before the workspace changes (aff62f7).
