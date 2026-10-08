---
format: aep.planning-md/3
id: story:control-plane-storage
kind: story
status: draft
title: Fallible storage and distinct execution identities for durable hosts
relations:
- serves: vision:O1
scope:
- confidence: inferred
  path: crates/loom-commission-testkit/tests
- confidence: inferred
  path: crates/loom-commission/src/outcome.rs
- confidence: inferred
  path: crates/loom-commission/src/ports/mod.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: crates/loom-sdk/examples/software_change.rs
- confidence: inferred
  path: ess/commission/domains/responsibility.yaml
revision: 25
---
## Outcome

A durable host can drive `run_until_blocked` over a run start/suspend interface whose storage
failures are reported as storage failures. Today `run_until_blocked` requires the generated
`StartRunBehavior + SuspendRunBehavior` (`crates/loom-commission/src/runtime.rs:327`). Their only error
is `UnmetObligation` (`generated/rust/commission/src/responsibility.rs:2277`), and the generated
`RunStorage` cannot fail (`generated/rust/commission/src/behaviour.rs:26-38`), so a durable adapter must
report a failed write as `LoopFailure::Obligation` (`runtime.rs:203`, `:227`).

This story adds a run start/suspend interface with an explicit error and a new `LoopFailure` variant
for it, implemented automatically for every existing generated behaviour, as `FallibleCaseStore`
covers `CaseStore` (`crates/loom-governor/src/lib.rs:165`). A failed `start_run` ends the loop before
any executor step or effect call, with `run_id: None`. A failed `suspend_run` is reported in
`LoopError::suspension`, never as an obligation.

Already on main and not reclaimed: case-store failure reporting and trusted evaluation time
(`story:hosted-governor`; `crates/loom-governor/src/lib.rs:152`, `tests/hosted_protocol.rs:163`), and
per-Loom selection and argument-request ids (`crates/loom-executor/src/lib.rs:140`, `:325`;
`tests/adversary_run_identity_across_looms.rs:77`; `tests/interruption_recovery.rs:380`).

## ESS first

Spec first. The generated `RunStorage` cannot fail; before implementation, decide whether the run
storage failure is declared in `ess/commission/domains/responsibility.yaml` (newest `ess`) or is an
adapter failure outside the specification, as `FallibleCaseStore` is. If ESS cannot express it,
stop and report.

## Acceptance

- `failed_durable_run_start_invokes_no_effect` (loom-commission-testkit): a run interface whose start
  fails returns `LoopError { run_id: None }` with the new storage-failure variant; executor, authority
  and effect ports are called zero times, and no `UnmetObligation` appears.
- `durable_run_suspend_failure_is_reported_not_obligation`: an injected suspend failure after a
  governor failure appears in `LoopError::suspension` as the storage-failure variant.
- Every existing `run_until_blocked` caller compiles unchanged through the automatic implementation
  (`crates/loom-intake-slice/src/run.rs:471-484`, `crates/loom-sdk/examples/software_change.rs:122`).
- Unchanged: `crates/loom-governor/tests/hosted_protocol.rs`,
  `crates/loom-executor/tests/adversary_run_identity_across_looms.rs`,
  `crates/loom-executor/tests/interruption_recovery.rs`, and Commission conformance.
- Not claimed: a persistence backend, or continuing the same process after recovery.

## Scope

Primary surface `crates/loom-commission/src/runtime.rs` (`run_until_blocked` :313-338, `LoopFailure`
:198-231), cited. Inferred: `crates/loom-commission/src/ports/mod.rs`, `crates/loom-commission-testkit/tests`,
`crates/loom-commission/src/outcome.rs` (`RunStore`), and only if the automatic implementation does not
cover them `crates/loom-intake-slice/src/run.rs`, `crates/loom-sdk/examples/software_change.rs`;
`ess/commission/domains/responsibility.yaml` if the failure is declared. A persistence provider is
excluded.

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

Probed 2026-10-08 with `ess` 0.56.0 on a scratch copy of `ess/commission/`: an error
`commission.responsibility.RunStorageFailed` (field `reason: String`) and a `StartRun` outcome
`storage-failed` with `external: the run store cannot hold the new run` and that error.

- `ess specify validate --path <copy> --strict-requires`: exit 0, "commission v1 — 2 file(s), valid".
- `ess generate synthesize --target rust --layout crate`: exit 0, "94 capabilities: 92 generated,
  2 obligation(s), 0 refused" (unchanged specification: "93 capabilities: 92 generated, 1
  obligation(s), 0 refused").
- `ess verify conform synthesize`: "14 scenario(s) (0 authored), 0 refusal(s)".
- Generated `StartRunOutcome` gains `StorageFailed`; `start_run` stops being generated and becomes
  an obligation the host implements (`impl StartRunBehavior for Generated<P>` delegates to the
  ports), so a durable host reports its failed write as that outcome. The generated `RunStorage`
  stays infallible.

So the decision's option A is expressible: declare the error and the `external:` outcome on
`StartRun` and `SuspendRun`, regenerate; no ess change is needed. The `SuspendRun` half was not
probed.
