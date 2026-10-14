---
format: aep.planning-md/3
id: story:dw-incident-workflow
kind: story
status: draft
title: An incident lookup runs as a workflow that reads its sources concurrently and calls a model only to summarise
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-read-fanout
- depends_on: story:dw-typed-step-outputs
- depends_on: story:dw-bugfix-baseline
revision: 2
---
## Outcome

An incident troubleshooting workflow runs on the SDK over `incident.response/1`: it looks a call up
by its number, follows its traces, and reads every configured source at once, then one judgement
step summarises what it found. The concurrent reads run live over the configured Connectors read
adapters (tracker, chat, support, forge); call-record and trace sources run against fixtures (the
fake `connectors` of `crates/loom-connectors`) until Connectors has adapters for them, and the
workflow does not wait on Connectors. Model calls happen only in the summary step, and the run is
measured into the baseline page.

## Why

The second reference workflow of the epic: data collection that is known in advance and should cost
no model call, and fan-out where a model-driven run reads one source per model turn.

## Decision

- The workflow is a Loom host definition over `incident.response/1`; Canon decides admission and
  completion.
- Live sources: the configured tracker, chat, support and forge read adapters. Call records and
  traces: fixtures through the fake `connectors` (`tests/fixtures/connectors/fake-connectors`)
  until Connectors ships adapters for them.

## Design

- `crates/loom-sdk/examples/workflows/incident-lookup.yaml` (new): a deterministic call lookup by
  number (the number is given by the host when it starts the run, as trusted context, never by a model), a deterministic trace
  read bound to the lookup's typed output (the call's trace ids), a `Concurrent` step reading the
  configured sources at once with the call's identifiers bound in, and a judgement step that
  summarises the outputs into a proposed cause.
- `WorkflowExecutor` gains the `Concurrent` step kind (`loom.workflow.StepKind`), proposed as
  `ProposedReads` (`story:dw-read-fanout`).
- An effect port for incident reads, over `loom_connectors::cli::ConnectorsCli`, refusing any
  source the commission's configuration does not list, before any command.
- `crates/loom-sdk/examples/incident_workflow.rs` (new) and a recorded run; the page
  `docs/qualification/workflow-baseline.md` gains the incident rows: measured calls, tokens, cost,
  and the target (model calls only in the summary step).

## Domain relations

- Incident data source -> `incident.response/1` read action (`metrics.inspect`, `logs.search`,
  `release.inspect`): **requires stakeholder input**, filed as
  `decision-blocker:dw-incident-source-actions`. A commission binds one operation per action
  (`ess/commission/domains/responsibility.yaml`, `commission.responsibility.ActionBindingKey`), and
  the workflow reads more sources than the protocol has read actions.
- Concurrent step -> reads: one-to-many, owned by the step; each read one `ActionRequest` of the Run
  (`ess/commission/domains/responsibility.yaml`, `commission.responsibility.Run` relation
  `requests`).

## Acceptance

The incident lookup runs end to end with model calls only in its summary step, and its source reads
run concurrently.

- `crates/loom-sdk/tests/incident_workflow.rs`
  `model_calls_happen_only_in_the_summary_step`: over the fake `connectors` and a replayed model.
- `the_configured_sources_are_read_concurrently_in_one_step`: the fake `connectors` logs overlapping
  invocations, and the meter reports one step for the fan-out.
- `the_trace_read_uses_the_lookups_typed_output`: the trace read's arguments hold the call's trace
  ids from the lookup's report; no model call between them.
- `a_source_outside_the_configuration_is_never_invoked`.
- `the_deterministic_steps_repeat_on_a_rerun`.

## ESS first

The first commit changes only `ess/`: `ess/domains/workflow.yaml` gains the `Concurrent` variant of
`loom.workflow.StepKind` (`reads: List<loom.workflow.ReadStep>`, each an `action` and `arguments`).
On it `task drift` fails; later commits regenerate and make
`model_calls_happen_only_in_the_summary_step` pass without changing `ess/`.

## Scope

- `ess/domains/workflow.yaml`, `generated/rust/loom/` (regenerated)
- `crates/loom-executor/src/workflow.rs`
- `crates/loom-sdk/src/` (incident read effect port, new module), `crates/loom-sdk/examples/workflows/incident-lookup.yaml` (new), `crates/loom-sdk/examples/incident_workflow.rs` (new)
- `crates/loom-sdk/tests/incident_workflow.rs` (new), `crates/loom-sdk/tests/fixtures/` (call-record and trace fixtures, new)
- `crates/loom-connectors/tests/fixtures/connectors/` (fake answers for the call-record and trace operations)
- `docs/qualification/workflow-baseline.md`, `CHANGELOG.md`, `website/data/status.json`

Touches `ess/domains/` and `generated/rust/loom` (shared with another running unit). Touches none of
`crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`, `ess/intake/`,
`generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`, `Cargo.toml`,
`Cargo.lock`.

Not ready: blocked on the open question below, and after `story:dw-read-fanout`,
`story:dw-typed-step-outputs` and `story:dw-bugfix-baseline`.

## Open

- Which `incident.response/1` read action each source is read under, and so how many bindings the
  commission holds (`decision-blocker:dw-incident-source-actions`). The design above (one effect
  port reading by `{source, kind}`) is that blocker's option A, not a decision.

## Not in scope

Live call-record and trace adapters (Connectors'); writes (`traffic.shift`, `release.rollback`);
changes to `incident.response/1`; the command line.
