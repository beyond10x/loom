---
format: aep.planning-md/3
id: story:command-schedule
kind: story
status: proposed
title: A Command LoomAgent with spec.schedule runs as a CronJob and reports each run
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
- depends_on: story:agent-run-once
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T12:40:55Z", actor: "human:timo", revision: 2}
---
## Outcome

A `Command` `LoomAgent` with `spec.schedule: {cron, timeZone?, suspend?, startingDeadlineSeconds?}`
runs on that schedule: the controller renders a `batch/v1` CronJob in the Job's place (the same
name and pod template, `jobTemplate.spec.backoffLimit` 0, `concurrencyPolicy: Forbid`, history
limits 3 and 3), keeps the resource `Running`, and reports each finished run in `status.lastRun`
and the condition `LastRunSucceeded`. `suspend: true` is the kill switch.

## Why

A downstream platform's first scheduled operator-agent run is blocked (item D389, 2026-11-02): a
`Command` resource is one Job that runs once (`story:command-variant-runs-once`). The design is
written: `docs/design/loom-agent-crd-deployment.md` § 12.2 on the unmerged branch
`docs/command-sidecars-and-schedule` (commit `c7073278`, slug `command-schedule`). This story lands
that section with the code; § 12.1 (sidecars) is not part of it.

## Acceptance

Named scenarios (ESS first: `spec.schedule`, `status.lastRun` and `LastRunSucceeded` are declared
in `ess/domains/resource.yaml`, the CRD is regenerated, then code):

- `resource-schedule/renders-cronjob`: a `Command` resource with `schedule.cron: "*/20 * * * *"`
  renders a CronJob named like the Job would be, with `Forbid`, `backoffLimit` 0, history limits
  3 and 3, the Job's pod template byte for byte, and no Job.
- `resource-schedule/runs-reported-in-turn`: two finished Jobs the CronJob owns, one exiting 0 and
  one exiting 3, set `status.lastRun` and `LastRunSucceeded` (True `RunSucceeded`, then False
  `RunFailed` with the exit code and end time); before any run it reads Unknown `NoRunYet`, and
  the resource stays `Running` throughout.
- `resource-schedule/suspend-starts-nothing`: `suspend: true` renders the CronJob suspended.
- `resource-schedule/agent-serve-refused`: `spec.schedule` on an `AgentServe` resource is refused
  `Malformed` at `spec.schedule`; an invalid cron expression or time zone is refused naming it.
- `resource-schedule/switch-variant`: adding `spec.schedule` deletes the resource's Job before the
  CronJob is created; removing it deletes the CronJob before the Job is created; the two never
  coexist.
- `resource-schedule/new-generation`: a new generation patches the CronJob's template.
- The chart's Role gains `cronjobs` (create, update, patch, get, list, watch, delete);
  `tests/chart.rs` holds it.

## Out of scope

Sidecars (§ 12.1 of the design); the one-shot command a scheduled run executes
(`story:agent-run-once`).

## Amendment: a declarative input

Item D389 (2026-11-02), scope addition from the downstream platform: `spec.schedule` carries the
run's task text declaratively, so the downstream sets values only, with no wrapper script, and no
member is added to `loom.agent/1`.

- `spec.schedule.input` is exactly one of `text` (inline) or `configMapKeyRef: {name, key}`; the
  rendered CronJob's program runs `b10x-loom agent run` (`story:agent-run-once`) with
  `--input <file>`, the file holding that text. Both forms absent, both present, or an empty
  `text`, `name` or `key` are refused `Malformed` at `spec.schedule.input`; `text` is walked for
  a credential as `spec.env` is.
- `resource-schedule/input-inline`: `input.text: "check the queue"` renders the text into the
  resource's ConfigMap and the CronJob's container runs `agent run` with `--input` naming that
  file under `/config`; the run reads exactly that text.
- `resource-schedule/input-config-map`: `input.configMapKeyRef: {name: tasks, key: nightly}`
  mounts that key read-only and passes its path as `--input`; the controller reads neither the
  ConfigMap nor its content, and a missing ConfigMap leaves the run's pod unstarted, reported
  by `status.lastRun` as Kubernetes reports it.
- `story:command-schedule` now depends on `story:agent-run-once`.
