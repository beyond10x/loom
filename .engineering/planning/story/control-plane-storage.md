---
format: aep.planning-md/3
id: story:control-plane-storage
kind: story
status: draft
title: Fallible storage and distinct execution identities for durable hosts
relations:
- serves: vision:O1
scope:
- confidence: cited
  path: crates/loom-commission
- confidence: cited
  path: crates/loom-executor
- confidence: inferred
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-governor
- confidence: inferred
  path: crates/loom-intake-slice
- confidence: inferred
  path: crates/loom-sdk
revision: 7
---
## Outcome
Control-plane can durably host the existing Loom case and run contracts. Every CaseStore operation reports storage failure separately from absence or duplicate insertion. Add a fallible runtime run-lifecycle port with explicit operational failure, adapting existing generated behaviors without abusing UnmetObligation. Distinct executor instances and resumed assignments never reuse selection or request identities.

## ESS first
The case and run entities already exist in ess/commission and ess; no new product noun or lifecycle is introduced. Storage failure is an operational adapter failure. Update specifications only if public domain behavior changes; record the named failing tests before implementation. Upgrade ESS requires and pinned ESS dependencies in touched systems to newest 0.53.0 before checking.

## Acceptance
Named tests: case_store_failure_never_retries_identity; case_store_failure_never_reports_missing; failed_durable_run_start_invokes_no_effect; different_executors_never_reuse_selection_identity. Existing memory-backed behavior remains unchanged. cargo test for governor, commission and executor; formatting and clippy pass. Recovery starts a fresh Run on the reconciled Case; continuing the same process/session is not claimed.

## Scope
Cited: crates/loom-governor; crates/loom-commission; crates/loom-executor. Inferred: affected callers in crates/loom-intake-slice and crates/loom-sdk. Protocol semantics and a new persistence provider are excluded. Relates to existing interruption-recovery and approval-suspend-resume-slice without claiming those broader stories complete.

## Authorization
Operator approved the control-plane plan and instructed Implement the plan. This is its bounded foundation prerequisite. All running code is Rust; command lines use clap derive. Coordinator owns all planning mutations.

## Provenance

Filed 2026-10-06 by session `loom` from an untracked file in the managed worktree
`control-plane-loom` (branch `feat/control-plane-storage` at `75eeb40`, no commits of its own),
left by a session that has ended. That file recorded `draft → proposed → active` at
2026-10-05T20:09:15Z as `human:timo`; no commit holds those moves, so this record starts at
`draft`. `story:hosted-governor` (`1b25fe8`) already delivers `FallibleCaseStore` and trusted
evaluation time; what this story adds beyond it is unassessed.
