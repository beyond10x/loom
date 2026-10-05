---
format: aep.planning-md/3
id: story:confined-tests-run
kind: story
status: implemented
title: tests.run runs confined by Substrate, refused when confinement is unavailable
relations:
- decomposes: epic:effect-bindings
- informed_by: architecture-design:effect-isolation
- serves: vision:O1
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/loom-cli
- confidence: cited
  path: crates/loom-commission-conformance
- confidence: cited
  path: crates/loom-commission-testkit
- confidence: cited
  path: crates/loom-commission-xtask
- confidence: cited
  path: crates/loom-commission/tests
- confidence: cited
  path: crates/loom-executor/tests
- confidence: cited
  path: crates/loom-intake-slice
- confidence: cited
  path: crates/loom-sdk
- confidence: cited
  path: crates/loom-xtask
- confidence: cited
  path: docs/qualification
- confidence: cited
  path: ess
- confidence: cited
  path: ess/intake
- confidence: cited
  path: generated/rust
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: website
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T20:56:27Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T20:56:27Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T21:29:47Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":3,"review_outcome":1}}, executor: "agent:codex-confined-tests"}
---
## Outcome

`b10x-loom run` executes `tests.run` inside Substrate's host driver, embedded the way Harness embeds
it: no network, the toolchain and source read-only, writes only to `target/`, memory and pid
limits. When confinement is unavailable the step stops with a named `ConfinementRefusal` unless the
run passes `--confinement none`, which the run record names. With no delegated cgroup, `b10x-loom`
re-execs itself under `systemd-run --user -p Delegate=yes --scope`. Dependencies are fetched on the
host before the run and mounted read-only. Every `tests.run` observation carries the
`AppliedConfinement` record.

## Why

architecture-design:effect-isolation, decisions 1, 2, 4, 5, 6 and 7 (operator, 2026-10-05).

## ESS first

Specification-only commits `6c9f384` and `84fb170` define `intake.confinement`, add
`StopReason::ConfinementUnavailable`, and require ESS 0.53.0 across the three systems.
The intake specification validated before implementation. The first behavioural red was
`confinement_defaults_to_substrate_and_requires_explicit_opt_out`: 0 passed, 1 failed,
exit 101. The generated intake crate and `task intake-drift` now hold the typed contract.
The qualification record retains the red and the executed confinement checks:
`docs/qualification/2026-10-05-confined-tests.md`.

## Acceptance

- `real_confinement_rust_system_tools_and_cleanup` executes the real delegated driver:
  source, Git metadata, toolchain and external writes fail; `target/` writes succeed;
  network and synthetic credential access fail; resource bounds and timeout cleanup hold;
  cached registry and Git dependencies work offline; missing dependencies fail actionably.
- `confinement_defaults_to_substrate_and_requires_explicit_opt_out` holds the default and
  explicit opt-out flags. `confinement_refusal_stops_the_loop_without_test_evidence` holds
  the stop reason and absence of passing evidence. Executor checks hold actual applied
  confinement on observations and explicit `none` observations.
- The dedicated delegated lane executes without skips on a capable machine. The live
  Codex fixture exercises automatic `systemd-run --user -p Delegate=yes --scope` re-exec,
  preserves the original test, fixes the function, records passing evidence and stops at
  `ApprovalRequired (repository.merge)`. The CLI refusal test holds the re-exec loop guard
  and exit status 3. Unavailable prerequisites are refusals, never confinement qualification.
