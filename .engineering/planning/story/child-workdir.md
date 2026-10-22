---
format: aep.planning-md/3
id: story:child-workdir
kind: story
status: draft
title: A confined child starts in the working directory the host names
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:substrate-0-8-pin
revision: 1
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
- A workdir that is not a mount the child has (workspace, read-only root, writable root) is refused
  `ScopeInvalid` before launch, naming it.
- The applied confinement names the workdir; a record naming another is refused.

## ESS first

`intake.confinement` declares the workdir in the applied confinement; `task intake-drift` is red
first.

## Not in scope

A writable mount outside `/workspace` (`story:writable-host-root`).

## Scope

Crates: `loom-intake-slice`, `ess/intake/`. `crates/loom-intake-slice/src/confinement.rs`,
`confinement/child.rs`, AGENTS.md, CHANGELOG.md. Same files as `story:several-egress-apertures`:
not in one wave with it.

## Ready

Waits on `story:substrate-0-8-pin` and a Substrate release with a working directory outside
`/workspace`.
