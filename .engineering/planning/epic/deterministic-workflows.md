---
format: aep.planning-md/3
id: epic:deterministic-workflows
kind: epic
status: draft
title: A workflow runs its deterministic steps without a model and counts the calls it makes
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A workflow built on the Loom SDK runs its deterministic steps without a model and calls a model
only where a step needs judgement: software delivery steps, a bug-fix loop, and incident
troubleshooting that collects its data concurrently (for example a call looked up by its number,
its traces followed, every source read at once). Each run reports its model calls, tokens and
cost, so a workflow has a baseline and a target that goes down.

## Why

Today every step of a governed run is chosen by a model: Commission's `run_until_blocked` asks the
executor for each next action, and the executor's loop sends a model turn for each, even when the
protocol's frontier and the previous step's typed result already decide the next step. A workflow
whose steps are known pays a model call per step and is only as repeatable as the model.

## Gaps (to be confirmed when the epic is decomposed)

- **A workflow definition the runtime executes step by step**: an ordered or conditional set of
  steps over a protocol's actions, run by the host without a model, with a model step declared
  explicitly where judgement is needed (classify, decide, write code, summarise).
- **Typed step outputs a later step reads without a model**: a step's result (a read's record, a
  test result, a lookup) bound by name into a later step's arguments, held by the specification,
  never re-read by a model to pass it on.
- **Concurrent fan-out of data-collection effects**: several admitted reads started at once and
  joined, each still revalidated and invoked through its binding once; today a run performs one
  action at a time.
- **Per-run counts of model calls, tokens and cost**: one record per run, by step, from the model
  port and llm's usage reporting. Some counters exist (inferred from the code, not measured: the
  executor's turn loop carries a price module and usage events, and `run --output jsonl` and the
  intake measurement report carry counters); none is a per-run total a workflow can be compared on.
- **A baseline and targets**: the three measures recorded for the reference workflows above, with
  a target per workflow, so a change that adds a model call shows.

## Acceptance (draft)

- A bug-fix loop and an incident lookup each run end to end on the SDK with model calls only in
  their declared judgement steps; a run report gives model calls, tokens and cost per run and per
  step, and the counts equal the calls the model port saw.
- The deterministic steps of each run give the same result on a re-run with the same inputs.

## Open questions

- Whether the workflow definition is a protocol-level construct (Canon, engineering-protocols) or
  a Loom host construct over a protocol: unclear, to be decided before decomposition.
- Which incident data sources the troubleshooting workflow reads first: unclear, to be asked.

## Not in scope

Changes to Canon or engineering-protocols; a hosted workflow service.
