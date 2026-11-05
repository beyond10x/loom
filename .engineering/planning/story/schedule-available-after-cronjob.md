---
format: aep.planning-md/3
id: story:schedule-available-after-cronjob
kind: story
status: implemented
title: Available reads Scheduled only once the CronJob exists
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
  path: crates/loom-operator/tests/adversary_w20261102w2_b_reconcile.rs
- confidence: cited
  path: crates/loom-operator/tests/reconcile.rs
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 4, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "active", to: "implemented", at: "2026-11-05T04:37:33Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1},"asserted":{"test_result":1}}, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
---
## Outcome

A `Command` resource switching from a Job to a CronJob reports `Available` True `Scheduled` only once the CronJob exists.

## Why

Out-of-scope row of review-result:adv-w20261102w2-b-pass-1: while the switch waits for the Job to go, `Available` already reads True `Scheduled`.

## Acceptance

- During the switch `Available` reads False with the reason `WorkloadTerminating` naming the wait, then True `Scheduled` after the CronJob is created.
