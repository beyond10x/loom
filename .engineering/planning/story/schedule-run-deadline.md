---
format: aep.planning-md/3
id: story:schedule-run-deadline
kind: story
status: draft
title: A scheduled run has a deadline so a hung run does not hold later runs
relations:
- decomposes: epic:loom-agent-command-variant
revision: 1
---
## Outcome

`spec.schedule` takes an optional run deadline rendered as the CronJob job template `activeDeadlineSeconds`, so a hung run ends and does not hold later runs under `Forbid`.

## Why

Out-of-scope row of review-result:adv-w20261102w2-b-pass-1, named as a limitation of the 0.55.0 pull request: without a deadline a hung run holds every later run.

## Acceptance

- `schedule.runDeadlineSeconds: 600` renders `activeDeadlineSeconds: 600`; a run past it ends and is reported `RunFailed` in `status.lastRun`; zero or negative is refused `Malformed`.
