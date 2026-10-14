---
format: aep.planning-md/3
id: story:dw-workflow-definition-executor
kind: story
status: active
title: A Loom workflow definition runs its deterministic steps without a model and asks one only in judgement steps
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-run-model-meter
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T10:13:47Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-14T10:14:20Z", actor: "human:timo", revision: 3}
---
## Outcome

A host defines a workflow over a protocol, as a Loom definition, and runs it with Commission's
runtime: a deterministic step proposes its action with its declared arguments and calls no model;
a judgement step is the one place a model is asked, and only among the actions that step declares.
Canon still decides admission and completion; the workflow only sequences actions the frontier
admits. Re-running a workflow with the same inputs and the same judgement answers proposes the same
deterministic steps with the same arguments.

## Why

Today every step of a governed run is chosen by a model (`LoopExecutor`,
`crates/loom-executor/src/harness/governed.rs`), even where the protocol's frontier and the
previous step already decide the next one, so a workflow whose steps are known pays a model call
per step and is only as repeatable as the model.

## Decision

The workflow definition is a Loom host construct: a Loom-side definition sequences the actions the
protocol's frontier admits; Canon still decides admission and completion; nothing changes outside
Loom. Moving the definition into a protocol after measurement is `story:dw-workflow-into-protocol`.

## Design

- A definition names its protocol (a catalog reference such as `software-change@1`) and an ordered
  list of steps. A step has a `name`, and is either `Deterministic` (one `action`, literal
  `arguments`) or `Judgement` (the `actions` the model may choose among, and `instructions`). A
  step may carry `when`: a frontier claim and the `Truth` it must have (`FrontierClaim`), so a step
  is conditional on what Canon reports, never on a model.
- `crates/loom-executor/src/workflow.rs` (new): `WorkflowExecutor` implements `AgentExecutor`. On
  each call it takes the first step after the last performed one whose `when` holds on the handed
  frontier: a deterministic step whose action the frontier lists (`Admissible` or
  `ApprovalRequired`) is proposed as `ProposedAction` with its arguments, and no model is asked; a
  judgement step runs the governed loop (`Loom::run_loop`) narrowed to the step's actions
  (`LoopConfig::admits`) with the step's instructions. A step whose action the frontier does not
  list ends the call `NeedsHumanJudgment`, naming the step and the frontier's reasons; the
  workflow never skips to a later step on its own.
- `RecordingEffects<E: EffectPort>` wraps the host's effect port and records each admitted request
  it was handed with its `EffectOutcome`; the workflow advances past a step only once that step's
  request was `Performed`. A refused or stale step is proposed again on the next call, bounded by
  Commission's idle rule.
- A definition is read from YAML into the generated types and checked before any run: every step
  action is an action of its protocol (`ProtocolEntry::actions`, new in `crates/loom-protocols`),
  every step name is unique, and a judgement step names at least one action; a failure names the
  step.
- `run_metering` (`story:dw-run-model-meter`) counts by executor call; this story labels each step
  of the meter's report with the workflow step's name.

## Domain relations

- Workflow definition -> protocol: many definitions per protocol, one protocol per definition,
  named by its catalog reference; a definition does not own the protocol and outlives no
  protocol it names — inferable (inferred from `crates/loom-protocols/src/lib.rs:195`,
  `ProtocolCatalog::get` by name; no ess/1 document declares the definition).
- Workflow definition -> step: one-to-many, ordered, owned by the definition; a step does not exist
  outside it — declared by this story's ESS first commit as a struct holding a list, a value with
  no lifecycle.
- Workflow run -> Commission Run: one `WorkflowExecutor` is the executor of one commission's run;
  each executor call is one step attempt — inferable (inferred from
  `crates/loom-commission/src/ports/executor.rs`, `AgentExecutor::run` takes the commission and
  the frontier).
- Step -> frontier action: a step names actions of its protocol; admission is the frontier's
  (`ess/commission/domains/responsibility.yaml`, `commission.responsibility.FrontierAction`,
  `commission.responsibility.Admission`).

## Acceptance

A two-step-kind workflow runs to the merge approval with model calls only in its judgement steps,
and a re-run proposes the same deterministic steps with the same arguments.

- `crates/loom-executor/tests/workflow_executor.rs`
  `deterministic_steps_call_no_model`: a workflow of `repository.inspect` (deterministic),
  `implementation.edit` (judgement), `tests.run` (deterministic) over a scripted governor and a
  metered scripted model; the model port sees calls only during the judgement step.
- `a_rerun_with_the_same_inputs_proposes_the_same_deterministic_steps`: two runs with the same
  judgement answers produce equal sequences of proposed actions and arguments.
- `a_step_the_frontier_does_not_list_ends_needing_human_judgment`: naming the step; nothing is
  proposed.
- `a_judgement_step_offers_the_model_only_its_actions`: a model call naming another action is
  refused to the model and never proposed.
- `a_definition_naming_an_action_outside_its_protocol_is_refused`: at load, naming the step.
- `a_step_advances_only_after_its_effect_was_performed`: a refused effect proposes the same step
  again.

## ESS first

The first commit changes only `ess/`: a new domain `loom.workflow` in `ess/domains/workflow.yaml`
declaring `loom.workflow.WorkflowDefinition` (`name`, `protocol`, `steps`),
`loom.workflow.WorkflowStep` (`name`, `when: Optional<loom.workflow.StepCondition>`, `kind`),
`loom.workflow.StepKind` (union: `Deterministic` with `action` and `arguments: Json`; `Judgement`
with `actions: List<String>` and `instructions`), `loom.workflow.StepCondition` (`claim`, `value`),
and `loom.workflow` listed in `ess/system.yaml`. On it `task drift` fails; later commits regenerate
and make `deterministic_steps_call_no_model` pass without changing `ess/`.

## Scope

- `ess/domains/workflow.yaml` (new), `ess/system.yaml`
- `generated/rust/loom/` (regenerated)
- `crates/loom-executor/src/workflow.rs` (new), `crates/loom-executor/src/lib.rs`,
  `crates/loom-executor/src/metering.rs` (step labels)
- `crates/loom-executor/tests/workflow_executor.rs` (new)
- `crates/loom-protocols/src/lib.rs` (`ProtocolEntry::actions`)
- `crates/loom-sdk/src/lib.rs` (re-export)
- `AGENTS.md`, `CHANGELOG.md`, `website/data/status.json`, `README.md` (SDK section)

Touches `ess/domains/` and `generated/rust/loom` (shared with another running unit; serialise or
regenerate on top). Touches none of `crates/loom-intake-slice`, `crates/loom-governor`,
`crates/loom-cli`, `ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`,
`ess/domains/authority.yaml`, `Cargo.toml`, `Cargo.lock` (`crates/loom-executor` does not gain
`loom-protocols`: the check takes the action list; the SDK, which has both, composes them).

Ready now, after `story:dw-run-model-meter`.

## Not in scope

Step outputs bound into later steps' arguments (`story:dw-typed-step-outputs`); concurrent steps
(`story:dw-read-fanout`); the reference workflows (`story:dw-bugfix-workflow`,
`story:dw-incident-workflow`); a protocol-level definition; a command-line flag.

## Open

- Whether the workflow's position survives a restart of the run
  (`decision-blocker:dw-run-state-across-restart`); this story keeps it in process.
