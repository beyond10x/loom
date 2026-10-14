---
format: aep.planning-md/3
id: story:dw-typed-step-outputs
kind: story
status: draft
title: A step's typed outputs are bound into a later step's arguments without a model
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-workflow-definition-executor
revision: 1
---
## Outcome

A workflow step declares the outputs it produces, each a named, typed value taken from its
performed effect's report, and a later step's arguments name those outputs; the runtime fills them
in without a model. A test result, a read's record or a lookup reaches the next step as the
specification typed it, never re-read and re-typed by a model.

## Why

Today an effect's result is `EffectOutcomePerformed.report`, untyped `Json`
(`ess/commission/domains/responsibility.yaml`, "Commission declares no structure for it"), and the
only thing that carries a result to the next step is a model reading it in the transcript and
writing it into the next call's arguments: a model call per hand-over, and a hand-over only as
exact as the model.

## Design

- A step declares `outputs`: each a `name`, a JSON pointer into its `Performed` report, and a
  declared type (`String`, `Integer`, `Boolean`, `Json`, or a list of one of them).
- A deterministic step's arguments may hold, at any position, a reference `{ "$step": "<step>",
  "$output": "<name>" }`; a judgement step may list outputs to put into its instructions as data.
- `RecordingEffects` (`story:dw-workflow-definition-executor`) extracts and type-checks a step's
  outputs when its effect is `Performed`, and keeps them for the run, in process.
- A reference is resolved when the step is proposed. A reference to a later step, an unknown step
  or an undeclared output is refused when the definition is read. An output whose pointer is
  absent from the report, or whose value does not have the declared type, ends the call
  `NeedsHumanJudgment` naming the step and the output, and nothing is proposed: the workflow fails
  toward less effect, never toward a guessed value.
- The resolved arguments are the proposal's arguments; Commission revalidates them as any other.

## Domain relations

- Step -> output: one-to-many, owned by the step's definition; declared by this story's ESS first
  commit as a list inside the step value.
- Argument reference -> output of an earlier step: many-to-one, references; the output exists only
  once its step was performed in this run, and is dropped with the run — the ordering is the
  definition's (inferable from `story:dw-workflow-definition-executor`'s ordered steps, declared in
  `ess/domains/workflow.yaml`, `loom.workflow.WorkflowDefinition.steps`).
- Output -> performed effect: one value per output, read from exactly one
  `commission.responsibility.EffectOutcomePerformed` (`ess/commission/domains/responsibility.yaml`,
  field `report`).

## Acceptance

A later step's arguments carry an earlier step's typed output with no model call, and a missing or
mistyped output proposes nothing.

- `crates/loom-executor/tests/workflow_step_outputs.rs`
  `a_later_step_reads_an_earlier_output_without_a_model`: a read step's report `{"call": {"id":
  "c-17"}}` declares `call_id: String` at `/call/id`; the next deterministic step proposes arguments
  holding `"c-17"`; the metered model port saw no call.
- `a_mistyped_output_ends_needing_human_judgment`: `/call/id` holding a number, declared `String`,
  proposes nothing and names the step and the output.
- `an_absent_output_ends_needing_human_judgment`.
- `a_reference_to_a_later_or_unknown_step_is_refused_at_load`.
- `outputs_are_identical_on_a_rerun_with_the_same_reports`.

## ESS first

The first commit changes only `ess/`: `ess/domains/workflow.yaml` gains
`loom.workflow.StepOutput` (`name`, `pointer`, `type: loom.workflow.OutputType`),
`loom.workflow.OutputType` (enum) and `outputs: List<loom.workflow.StepOutput>` on
`loom.workflow.WorkflowStep`. On it `task drift` fails; later commits regenerate and make
`a_later_step_reads_an_earlier_output_without_a_model` pass without changing `ess/`.

## Scope

- `ess/domains/workflow.yaml`
- `generated/rust/loom/` (regenerated)
- `crates/loom-executor/src/workflow.rs`
- `crates/loom-executor/tests/workflow_step_outputs.rs` (new)
- `AGENTS.md`, `CHANGELOG.md`

Touches `ess/domains/` and `generated/rust/loom` (shared with another running unit). Touches none of
`crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`, `ess/intake/`,
`generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`, `Cargo.toml`,
`Cargo.lock`.

Ready now, after `story:dw-workflow-definition-executor`.

## Not in scope

Outputs of a model's judgement (a judgement step's structured answer as an output); typed reports
in Commission's own contract; outputs kept across a restart.

## Open

- Whether outputs survive a restart of the run (`decision-blocker:dw-run-state-across-restart`):
  a step after an approval the run was resumed from cannot read an output of a step before it
  until that is decided. This story keeps outputs in process and does not depend on the answer.
