---
format: aep.planning-md/3
id: story:schedule-run-deadline
kind: story
status: active
title: A scheduled run has a deadline so a hung run does not hold later runs
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: charts/loom-agents/crds/loomagents.loom.beyond10x.dev.yaml
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/lib.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: crates/loom-operator/src/resource.rs
- confidence: inferred
  path: crates/loom-operator/tests/reconcile.rs
- confidence: cited
  path: crates/loom-operator/tests/render.rs
- confidence: cited
  path: deploy/crd/loomagents.loom.beyond10x.dev.yaml
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/scenarios/resource-schedule
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference/ess
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 2, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
---
## Outcome

`spec.schedule` takes an optional run deadline rendered as the CronJob job template `activeDeadlineSeconds`, so a hung run ends and does not hold later runs under `Forbid`.

## Why

Out-of-scope row of review-result:adv-w20261102w2-b-pass-1, named as a limitation of the 0.55.0 pull request: without a deadline a hung run holds every later run.

## Acceptance

- `schedule.runDeadlineSeconds: 600` renders `activeDeadlineSeconds: 600`; a run past it ends and is reported `RunFailed` in `status.lastRun`; zero or negative is refused `Malformed`.
