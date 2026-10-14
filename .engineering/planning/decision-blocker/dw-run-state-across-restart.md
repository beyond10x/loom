---
format: aep.planning-md/3
id: decision-blocker:dw-run-state-across-restart
kind: decision-blocker
status: open
title: Nobody has decided whether a run's usage report and step results survive a restart of the run
relations:
- blocks: epic:deterministic-workflows
revision: 1
---
## Question

Does a run's model usage report, and the step results a workflow binds into later steps, survive a
restart of the run?

A Commission Run resumed after a restart is the same Run
(`ess/commission/domains/responsibility.yaml`, `commission.responsibility.Run`, lifecycle `resume`
keeps the run id). The usage meter and the step results the stories of
`epic:deterministic-workflows` add are in-process values. A bug-fix run usually stops at the merge
approval and is resumed after the grant, often in another process, so "per run" either means per
process segment or needs a durable record.

Relation: run usage report and step results -> `commission.responsibility.Run`, cardinality and
lifecycle coupling across a restart undecided.

## Options

- A. Per process segment: the report and the step results cover one `run_until_blocked` or one
  `resume_after_grant` call; a host sums segments itself; a resumed workflow re-reads nothing.
- B. Durable: both are written beside the governor's `FileSuspensionStore` record, keyed by the
  Run id, and a resume continues them.
- C. A now, B once a workflow needs a step result across an approval.

## Stops

The cross-restart part of the per-run report and of typed step outputs; the in-process stories are
drafted without it.
