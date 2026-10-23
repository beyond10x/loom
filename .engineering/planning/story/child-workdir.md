---
format: aep.planning-md/3
id: story:child-workdir
kind: story
status: implemented
title: A confined child starts in the working directory the host names
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:substrate-0-8-pin
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T03:54:53Z", actor: "human:timo", revision: 5, correlation: "wave-2026-10-23-w2"}
- {from: "proposed", to: "active", at: "2026-10-23T03:54:53Z", actor: "human:timo", revision: 6, correlation: "wave-2026-10-23-w2"}
- {from: "active", to: "implemented", at: "2026-10-23T05:38:13Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}, correlation: "wave-2026-10-23-w2"}
---
## Outcome

`ChildCommand::with_workdir(path)` starts the confined child in that directory; a pin that cannot
honour it refuses the command before launch, naming the path.

## Why

Substrate 0.7.18 gives no working directory outside `/workspace` (AGENTS.md § Intake and the
command line). takt runs a harness session in a linked worktree and cannot confine it (design § 5
gap 3).

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs`
  (`a_child_starts_in_its_named_workdir`): the child prints its working directory and it is the
  named one.
- A workdir that is not at or beneath a mount the child has (the workspace or a read-only root) is
  refused `ScopeInvalid` before launch, naming it. A workdir under a writable root waits on
  `story:writable-host-root` (Timo's Q2) and is not accepted here.
- The applied confinement names the workdir; a record naming another is refused.

## ESS first

`intake.confinement` declares the workdir in the applied confinement; `task intake-drift` is red
first.

## Not in scope

A writable mount outside `/workspace` (`story:writable-host-root`).

## Scope

Crates: `loom-intake-slice`, `ess/intake/`. `crates/loom-intake-slice/src/confinement.rs`,
`confinement/child.rs`, AGENTS.md, CHANGELOG.md. Same files as `story:several-egress-apertures`:
not in parallel with it; in wave 2026-10-23-w2 it starts from the wave branch after that story
merged.

## Ready

Ready since Substrate 0.10.0 (`105cf718`, linked by Loom 0.36.0): an exec or session start may
name `workdir` at or beneath `/workspace` or a declared read-only root, served under the
`exec.workdir` fact (Substrate CHANGELOG 0.9.0 § Added). Accepted in wave 2026-10-23-w2.
