---
format: aep.planning-md/3
id: story:job-available-after-cronjob
kind: story
status: active
title: Available names the wait while a CronJob goes before a single-run Job
tags:
- from-finding
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-operator/src/reconcile.rs
- confidence: cited
  path: crates/loom-operator/src/status.rs
- confidence: cited
  path: crates/loom-operator/tests/reconcile.rs
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T06:22:00Z", actor: "human:timo", revision: 2, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-w2-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T06:22:00Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-w2-loom"}
---
## Outcome

A `Command` resource switching from a CronJob back to a single-run Job reports `Available` False
`WorkloadTerminating`, naming the CronJob it waits for, until the CronJob is gone and the Job is
created, as the opposite switch has done since 0.57.0.

## Why

Found while implementing `story:schedule-available-after-cronjob` (wave 2026-11-05-w1): removing
`spec.schedule` waits for the CronJob to go, and while it waits `Available` reads Unknown
`RunNotFinished`, which says a run is under way when none has started.

## Acceptance

- During the switch `Available` reads False with the reason `WorkloadTerminating`, naming the
  CronJob, then follows the single-run Job's conditions once the Job exists.
