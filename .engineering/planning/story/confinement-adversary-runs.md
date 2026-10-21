---
format: aep.planning-md/3
id: story:confinement-adversary-runs
kind: story
status: implemented
title: The confinement escape tests run under a delegated scope instead of being ignored
relations:
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/security_w20261021w6_u2.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/support/mod.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T13:40:13Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-21T13:40:14Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-21T15:53:26Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

The confinement adversary tests run wherever `confined_child.rs` runs. Each starts its own test
binary under `systemd-run --user --scope -p Delegate=yes`, skips naming the missing prerequisite,
and is no longer `#[ignore]`d.

## Why

`crates/loom-intake-slice/tests/adversary_confinement.rs` holds the escape tests for the confined
test runner: target hard links into source, `.git` or external files, an inherited environment,
a toolchain override, and a socket. On `05699aae` four of them carry
`#[ignore = "requires a dedicated delegated systemd user scope"]` (lines 14, 62, 89, 133). No
task and no workflow passes `--ignored`: `Taskfile.yml:34` runs `cargo test --workspace --locked`.
AGENTS.md § Intake requires real delegated tests to run before confinement counts as qualified.
These tests run nowhere. Whether they pass today: I don't know.

## Design

- Move each ignored test to the pattern of `crates/loom-intake-slice/tests/confined_child.rs`:
  the parent test re-executes its own binary filtered to the child under a delegated user scope,
  and skips with a named prerequisite where bubblewrap, a user systemd manager or the driver is
  absent.
- The fixture child at line 124 (`fixture child invoked only inside Substrate`) stays ignored;
  only its parent starts it.

## ESS first

Exempt: no behaviour of Loom changes. The tests move from ignored to run.

## Acceptance

- `cargo test -p b10x-loom-intake-slice --locked --test adversary_confinement -- --list --ignored`
  lists only the fixture child.
- On a host with a user systemd manager and bubblewrap, `cargo test -p b10x-loom-intake-slice
  --locked --test adversary_confinement` runs the four escape tests and they pass. A failure
  found here is fixed in the runner within this story.

## Scope

- `crates/loom-intake-slice/tests/adversary_confinement.rs`
- `crates/loom-intake-slice/src/confinement.rs` (only if an escape test fails)

## Shipped

Merged into wave 2026-10-21-w6 as 2497f285, after one security review (review-result:sec-w20261021w6-u2-pass-1). Running the escape tests found that an inherited `RUSTUP_TOOLCHAIN` overrode the workspace toolchain file (fixed in 57debc06). The review found that the shared harness passed a case that never ran, or a refusal naming no missing prerequisite, as a skip (fixed in ae16fdb5).
