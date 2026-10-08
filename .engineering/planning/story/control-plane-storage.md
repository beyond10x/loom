---
format: aep.planning-md/3
id: story:control-plane-storage
kind: story
status: implemented
title: Fallible storage and distinct execution identities for durable hosts
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-commission-conformance/src/lib.rs
- confidence: cited
  path: crates/loom-commission-conformance/src/store.rs
- confidence: inferred
  path: crates/loom-commission-testkit/tests
- confidence: cited
  path: crates/loom-commission-testkit/tests/adversary2_run_conformance.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/adversary_run_outcomes.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/durable_run_storage.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/run_outcomes.rs
- confidence: cited
  path: crates/loom-commission-xtask/tests/checks.rs
- confidence: inferred
  path: crates/loom-commission/src/outcome.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: generated/rust/commission
- confidence: inferred
  path: website/docs/reference/commission
revision: 37
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T18:56:45Z", actor: "human:timo", revision: 33}
- {from: "proposed", to: "active", at: "2026-10-08T18:56:45Z", actor: "human:timo", revision: 34}
- {from: "active", to: "implemented", at: "2026-10-08T19:28:03Z", actor: "human:timo", revision: 37, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

A durable host can drive `run_until_blocked` over run start and suspend whose storage failures are
reported as storage failures. Today `run_until_blocked` requires the generated
`StartRunBehavior + SuspendRunBehavior` (`crates/loom-commission/src/runtime.rs:327`). Their only error
is `UnmetObligation` (`generated/rust/commission/src/responsibility.rs:2277`), and the generated
`RunStorage` cannot fail (`generated/rust/commission/src/behaviour.rs:26-38`), so a durable adapter must
report a failed write as `LoopFailure::Obligation` (`runtime.rs:203`, `:227`).

This story declares a `storage-failed` outcome on `StartRun` and `SuspendRun` in the Commission
specification and adds a `LoopFailure` variant for it. A failed `start_run` ends the loop before
any executor step or effect call, with `run_id: None`. A failed `suspend_run` is reported in
`LoopError::suspension`, never as an obligation.

Already on main and not reclaimed: case-store failure reporting and trusted evaluation time
(`story:hosted-governor`; `crates/loom-governor/src/lib.rs:152`, `tests/hosted_protocol.rs:163`), and
per-Loom selection and argument-request ids (`crates/loom-executor/src/lib.rs:140`, `:325`;
`tests/adversary_run_identity_across_looms.rs:77`; `tests/interruption_recovery.rs:380`).

## ESS first

Spec first. Decided 2026-10-08: the run storage failure is declared in
`ess/commission/domains/responsibility.yaml` as an `external:` outcome on `StartRun` and
`SuspendRun` (see ESS probe), regenerated with the newest `ess`; no hand-written fallible trait.

## Acceptance

Rewritten 2026-10-08 to the declared outcome: the run storage failure is declared in the
Commission specification, not added as a hand-written fallible trait.

- `ess/commission/domains/responsibility.yaml` declares the error
  `commission.responsibility.RunStorageFailed { reason: String }` and an `external:` outcome
  `storage-failed` with that error on `StartRun` and on `SuspendRun`; `task commission:spec`,
  `commission:ess-gate`, `commission:drift`, `commission:docs-drift` and `commission:conform` pass
  after `task commission:generate`, with 0 refusals.
- The generated `StartRunOutcome` and `SuspendRunOutcome` carry `StorageFailed`; both commands are
  host obligations, and `RunStore` (`crates/loom-commission/src/outcome.rs`) implements them, so
  every existing `Generated<RunStore>` caller compiles unchanged
  (`crates/loom-intake-slice/src/run.rs:471-484`, `crates/loom-sdk/examples/software_change.rs:122`).
- `failed_durable_run_start_invokes_no_effect` (loom-commission-testkit): a run interface whose
  `start_run` returns `StorageFailed` makes `run_until_blocked` return `LoopError { run_id: None }`
  with a new `LoopFailure` storage-failure variant; executor, authority and effect ports are called
  zero times; no `UnmetObligation` appears.
- `durable_run_suspend_failure_is_reported_not_obligation`: a `suspend_run` returning
  `StorageFailed` after a governor failure appears in `LoopError::suspension` as the storage-failure
  variant, not as `NotSuspended` and not as an obligation.
- Unchanged: `crates/loom-governor/tests/hosted_protocol.rs`,
  `crates/loom-executor/tests/adversary_run_identity_across_looms.rs`,
  `crates/loom-executor/tests/interruption_recovery.rs`.
- Not claimed: a persistence backend, or continuing the same process after recovery.

## Scope

Scoped 2026-10-08 with `story-scoper`; confirmed by the wave 2026-10-08-w7 implementor. Every
inferred line was checked; `ess/commission/SKIPPED.md` was not needed (the conformance runner forces
the `storage-failed` outcome instead).

- `ess/commission/domains/responsibility.yaml`: changed.
- `generated/rust/commission/`: regenerated; it no longer has `Context`, `TryContext` or
  `unmet_context`.
- `crates/loom-commission/src/runtime.rs` (`LoopFailure::RunStorage`, `run_until_blocked`,
  `suspend`) and `src/outcome.rs` (`RunStore` implements the `StartRun` and `SuspendRun`
  obligations): changed.
- `crates/loom-commission-conformance/src/lib.rs` and new `src/store.rs` (forced storage failure):
  changed.
- `crates/loom-commission-testkit/tests/`: new `durable_run_storage.rs`; `run_outcomes.rs`,
  `adversary_run_outcomes.rs`, `adversary2_run_conformance.rs` changed.
- `crates/loom-commission-xtask/tests/checks.rs:266` (not scoped; found by adversary pass 1).
- `website/docs/reference/commission/` (regenerated), `CHANGELOG.md`.
- Unchanged: `crates/loom-commission/src/ports/mod.rs`, `crates/loom-intake-slice/src/run.rs`,
  `crates/loom-sdk/examples/software_change.rs`.

## Authorization
Operator approved the control-plane plan and instructed Implement the plan. This is its bounded foundation prerequisite. All running code is Rust; command lines use clap derive. Coordinator owns all planning mutations.

## Provenance

Filed 2026-10-06 by session `loom` from an untracked file in the managed worktree
`control-plane-loom` (branch `feat/control-plane-storage` at `75eeb40`, no commits of its own),
left by a session that has ended. That file recorded `draft → proposed → active` at
2026-10-05T20:09:15Z as `human:timo`; no commit holds those moves, so this record starts at
`draft`. `story:hosted-governor` (`1b25fe8`) already delivers `FallibleCaseStore` and trusted
evaluation time; what this story adds beyond it is unassessed.

## ESS probe

Probed 2026-10-08 with `ess` 0.56.0 on a scratch copy of `ess/commission/`. First `StartRun` alone:
"94 capabilities: 92 generated, 2 obligation(s), 0 refused" (unchanged specification: 93, 92, 1).

Then both halves: the error `commission.responsibility.RunStorageFailed` (field `reason: String`),
`storage-failed` on `StartRun` ("the run store cannot hold the new run") and on `SuspendRun` ("the
run store cannot record the suspension").

- `ess specify validate --path <copy> --strict-requires`: exit 0, "commission v1 — 2 file(s), valid".
- `ess generate synthesize --target rust --layout crate`: exit 0, "96 capabilities: 93 generated,
  3 obligation(s), 0 refused".
- `ess verify conform synthesize`: "15 scenario(s) (0 authored), 0 refusal(s)".
- Generated `StartRunOutcome` and `SuspendRunOutcome` gain `StorageFailed`; `impl StartRunBehavior`
  and `impl SuspendRunBehavior for Generated<P>` forward to `P`'s implementation, so the host
  implements both. The generated `RunStorage` stays infallible.

No ess change is needed.
