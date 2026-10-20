---
format: aep.planning-md/3
id: story:python-test-profile
kind: story
status: draft
title: The confined test runner serves a toolchain beyond rustup Rust, Python first
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
  path: crates/loom-intake-slice/tests/python_profile.rs
revision: 5
---
## Outcome

The confined test runner serves a toolchain other than rustup's Rust, Python first: a test command
whose interpreter lives outside `RUSTUP_HOME` runs confined, with the interpreter and the source
read-only, no network, a cleared environment, and writes allowed only under `target` and a scratch
directory the host names. Bytecode is not written into the source tree. Today the compiler must
belong to an installed `RUSTUP_HOME/toolchains` directory
(`crates/loom-intake-slice/src/confinement.rs:555-565`) and only `target` is writable (`:26`).

## Why

takt delivers stories in repositories whose tests are Python. Until the profile serves them, takt
keeps its confinement off by default.

## Acceptance

- `crates/loom-intake-slice/tests/python_profile.rs::a_python_test_command_runs_confined`
  (delegated, skipped with a named missing prerequisite): a `python -m pytest` command over a fixture
  passes with no network and a cleared environment, and the observation names the applied
  confinement.
- `…::a_write_into_the_source_tree_fails`: a fixture test that writes a file next to its source
  fails with a read-only error, and the file does not exist after the run.
- `…::a_write_into_the_named_scratch_directory_succeeds`: the same write into the host's scratch
  directory succeeds.
- `…::no_bytecode_is_left_in_the_source_tree`: after the run, no `__pycache__` directory exists under
  the fixture's source.
- `…::an_interpreter_path_through_a_link_is_refused`: as the Rust toolchain check refuses links.
- The Rust profile's tests pass unchanged.
- `task check` exits 0.

## ESS first

If the toolchain kind becomes a declared input (for example in `intake.routing` or `loom.run`), it
is declared first; otherwise the red step is `a_python_test_command_runs_confined`.

## Depends on

`story:sdk-confines-any-child`, which changes `confinement.rs` first.

## Not in scope

- Toolchains other than Python.
- Installing interpreters: the operator provides them, as with Rust.

## Scope

Crates: `loom-intake-slice`.

- `crates/loom-intake-slice/src/confinement.rs`, `crates/loom-intake-slice/tests/python_profile.rs` (new)
- `CHANGELOG.md`, `AGENTS.md` (the confinement paragraph)

`confinement.rs` is shared with `story:sdk-confines-any-child` and `story:harness-child-egress-profile`.
