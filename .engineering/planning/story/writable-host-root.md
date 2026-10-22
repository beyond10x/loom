---
format: aep.planning-md/3
id: story:writable-host-root
kind: story
status: draft
title: A confined child writes an operator-declared root outside the workspace
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:substrate-0-8-pin
revision: 1
---
## Outcome

`ChildCommand::with_writable_root(name)` mounts an operator-declared root writable for the confined
child; a linked worktree's common `.git` accepts a commit made by the child.

## Why

A child can write only under `/workspace` today (`with_writable`, `confinement.rs:1133`, takes
workspace paths). takt's git, forge and harness children need the common `.git` of a linked
worktree or a harness configuration home (design § 5 gap 4).

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs`
  (`a_child_commits_through_a_declared_writable_root`): a linked worktree's common `.git` declared
  by name; the child commits and the commit is in the primary repository.
- A name the operator did not declare is refused before launch, naming it.
- The applied confinement names each writable root; a record naming another set is refused.

## ESS first

`intake.confinement` declares writable roots in the applied confinement; `task intake-drift` is red
first.

## Not in scope

Choosing between Q2's options; that is `decision-blocker:sdk-writable-host-root`.

## Scope

Crates: `loom-intake-slice`, `ess/intake/`. `crates/loom-intake-slice/src/confinement.rs`,
`confinement/child.rs`, AGENTS.md, CHANGELOG.md.

## Ready

Blocked by `decision-blocker:sdk-writable-host-root` (Timo's Substrate ADR) and the Substrate
release that implements it.
