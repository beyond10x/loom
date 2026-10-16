---
format: aep.planning-md/3
id: story:dw-judgement-step-inputs
kind: story
status: implemented
title: A judgement step reads the earlier typed outputs it lists as data in its instructions
relations:
- decomposes: epic:deterministic-workflows
- depends_on: story:dw-typed-step-outputs
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T23:50:23Z", actor: "human:timo", revision: 2, correlation: "wave-2026-10-15-w1"}
- {from: "proposed", to: "active", at: "2026-10-14T23:50:23Z", actor: "human:timo", revision: 3, correlation: "wave-2026-10-15-w1"}
- {from: "active", to: "implemented", at: "2026-10-16T01:33:09Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A judgement step lists earlier steps' typed outputs it is given as data, and the runtime puts
exactly those values into the model's instructions for that step, typed as the specification
declares them; the model never reads them back out of the transcript.

## Why

`story:dw-typed-step-outputs` carries an earlier step's typed outputs into a later deterministic
step's arguments. Its Design also said a judgement step may list outputs to put into its
instructions as data; that part was neither specified in `ess/domains/workflow.yaml` nor built (the
adversary pass `review-result:adv-w11-u3-pass-1`, finding 2). Without it a judgement step still
learns an earlier result only from the transcript.

## Design

- `loom.workflow.JudgementStep` gains `inputs: List<loom.workflow.OutputReference>` (step and output
  name), checked at load like an argument reference: an earlier step, a declared output.
- When the step is proposed, the referenced values are rendered into the step's instructions as a
  data block, never as instructions; an absent or mistyped value ends the call `NeedsHumanJudgment`
  naming the step and the output, as for arguments.

## Acceptance

A judgement step given an earlier output sees exactly that value in its instructions, and a missing
one asks the model nothing.

- `crates/loom-executor/tests/workflow_step_outputs.rs`
  `a_judgement_step_reads_a_listed_output_as_data`
- `a_judgement_step_listing_an_absent_output_asks_no_model`

## ESS first

The first commit changes only `ess/domains/workflow.yaml` (`inputs` on `JudgementStep`, and the
reference type if it is not already declared); `task drift` is red on it.

## Scope

- `ess/domains/workflow.yaml`
- `generated/rust/loom/` (regenerated)
- `crates/loom-executor/src/workflow.rs`
- `crates/loom-executor/tests/workflow_step_outputs.rs`

## Not in scope

Outputs declared by a judgement step itself (the adversary's note: a report echoing model-written
arguments would carry a model value forward); that is judged when this story is scoped.
