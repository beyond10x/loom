---
format: aep.planning-md/3
id: story:schedule-available-after-cronjob
kind: story
status: draft
title: Available reads Scheduled only once the CronJob exists
relations:
- decomposes: epic:loom-agent-command-variant
revision: 1
---
## Outcome

A `Command` resource switching from a Job to a CronJob reports `Available` True `Scheduled` only once the CronJob exists.

## Why

Out-of-scope row of review-result:adv-w20261102w2-b-pass-1: while the switch waits for the Job to go, `Available` already reads True `Scheduled`.

## Acceptance

- During the switch `Available` reads False with a reason naming the wait, then True `Scheduled` after the CronJob is created.
