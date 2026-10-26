---
format: aep.planning-md/3
id: story:command-run-exit-status
kind: story
status: active
title: A Command LoomAgent's status reports its run's completion and exit code
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: charts/loom-agents/crds
- confidence: inferred
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-operator/src/reconcile.rs
- confidence: cited
  path: crates/loom-operator/src/status.rs
- confidence: inferred
  path: crates/loom-resource
- confidence: cited
  path: deploy/crd
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: generated/rust/loom
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T15:49:08Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T15:49:08Z", actor: "human:timo", revision: 5}
---
## Outcome

A `Command` `LoomAgent`'s status reports its run: completed or failed, the program's exit code and
when it ended, in `status` and a condition, so a caller reads the outcome from the resource without
reading the pod.

## Why

Trial S5/S6, 2026-10-26: takt's runs ended with exit 3 and 5 while the resource stayed `Running`;
the outcome was read from pod logs. Gap: "LoomAgent status reports a Command run's exit code and
completion".

## Acceptance

- ESS first: `loom.resource` gains a status member for the run's end (exit code, end time) and an
  outcome of `ObserveDeployment` (or a new command) recording it; scenarios red first.
- `a_completed_run_reports_exit_code_0`, `a_failed_run_reports_its_exit_code`: status and the
  `Available` (or a new `Completed`) condition carry them.
- `deploy/crd/` regenerated with the new status members; `task crd-drift` exits 0.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`. **cited** / **inferred** as marked.

- `crates/loom-operator/src/status.rs`, `reconcile.rs` — cited
- `ess/domains/resource.yaml` (status, `ObserveDeployment`) — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` — cited
- `crates/loom-resource/`, `crates/loom-conformance/src/resource.rs` — inferred
- AGENTS.md § Resource, CHANGELOG.md — cited
- Depends on: `story:command-variant-runs-once` (the Job's terminal state is what it reads) — inferred
