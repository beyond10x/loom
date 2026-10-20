---
format: aep.planning-md/3
id: story:sdk-confines-any-child
kind: story
status: active
title: 'loom-sdk confines a child process the host names: program, arguments, directory, environment, profile'
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-intake-slice/src/confinement.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/confined_child.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T04:14:24Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
---
## Outcome

loom-sdk confines a child process a host names (program, arguments, working directory,
environment, profile) through Substrate, with the same guarantees the test runner has: a typed
refusal when a guarantee is missing, never a silent fall-back to an unconfined run, and the applied
confinement reported with the result. Today the only confined launch is
`TestRunner::run(&TestCommand, root)` (`crates/loom-intake-slice/src/confinement.rs:33-36`),
exported as such (`crates/loom-sdk/src/lib.rs:77-78`).

## Why

takt runs a harness child, `git`, its forge command and `aep` beside its test command. ADR 0099
puts their confinement in Loom over Substrate; takt does not build its own.

## Acceptance

- `crates/loom-intake-slice/tests/confined_child.rs::a_named_child_runs_confined_and_reports_what_was_applied`
  (delegated, skipped with a named missing prerequisite where bubblewrap or the driver is absent):
  a program with arguments, a directory and two environment variables runs under the workspace
  profile, and the result carries the applied confinement.
- `…::a_missing_guarantee_is_a_typed_refusal`: with the backend absent, the launch is refused and
  nothing runs.
- `…::the_environment_holds_only_what_the_host_named`: no variable the host did not name reaches the
  child.
- `TestRunner` keeps its behaviour; the existing confinement tests pass unchanged.
- The seam is exported from `loom_sdk` next to `TestRunner` (`crates/loom-sdk/src/lib.rs:77-80`),
  and the stability page lists its type in "Types and functions an embedding calls";
  `crates/loom-sdk/tests/host_ports.rs::every_control_plane_path_on_the_page_resolves` passes.
- `task check` exits 0.

## ESS first

No noun the specification lacks: the child request is Substrate's own wire types
(`ConfinementRequest`, `ExecEnvironment`, `SandboxProfile`; imported at
`crates/loom-intake-slice/src/confinement.rs:20-23`), so `ess/` is untouched. The red step is
`a_named_child_runs_confined_and_reports_what_was_applied`.

## Not in scope

- Network egress (`story:harness-child-egress-profile`).
- Toolchains other than rustup's Rust (`story:python-test-profile`).

## Scope

Crates: `loom-intake-slice`, `loom-sdk`.

- `crates/loom-intake-slice/src/confinement.rs`
- `crates/loom-intake-slice/tests/confined_child.rs` (new)
- `crates/loom-sdk/src/lib.rs` (the confinement re-export, `:77-80`),
  `website/docs/reference/sdk-stability.md` (one row in the types table)
- `CHANGELOG.md`, `AGENTS.md` (§ Intake and the command line, the confinement paragraph)

`confinement.rs` is shared with `story:harness-child-egress-profile` and `story:python-test-profile`,
which both depend on this story. `crates/loom-sdk/src/lib.rs` is also edited by
`story:chooser-executor-for-plugin-turns` (another re-export), and the stability page by
`story:engineering-protocols-090` and `story:authority-sees-the-request` (other rows);
`story:agent-executor-host-port` and `story:delete-harness-port` land after this story.
