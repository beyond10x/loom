---
format: aep.planning-md/3
id: story:dw-bugfix-workflow
kind: story
status: draft
title: The bug-fix loop runs as a workflow with model calls only in its judgement steps
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-bugfix-baseline
- depends_on: story:dw-typed-step-outputs
revision: 1
---
## Outcome

A bug-fix loop runs end to end on the SDK as a Loom workflow over `software-change@1` with
`change_kind: bugfix`: `repository.inspect`, `tests.write` (judgement), `tests.run` (expected red),
`implementation.edit` (judgement), `tests.run` (expected green), then the merge request, which
stops at its approval. Model calls happen only in the two judgement steps; the run report gives
calls, tokens and cost per step; and the measured figures are set against the baseline.

## Why

This is the first reference workflow of the epic: it shows a governed bug fix that pays for a model
only where code or a test has to be written, and is the measurement the baseline's target is held
to.

## Design

- `crates/loom-sdk/examples/workflows/bugfix.yaml` (new): the definition, a Loom host definition;
  `tests.run` takes its command from the host, never the model; the second `tests.run` carries
  `when` on the claim the protocol reports for the regression being shown first, so the edit is
  never run before the red test.
- `crates/loom-sdk/examples/bugfix_workflow.rs` (new): the baseline example's embedding with
  `WorkflowExecutor` over the definition, the slice's local effects behind `RecordingEffects`, and
  the metered port; with `--record` it records a live run as the baseline example does.
- `docs/qualification/workflow-baseline.md`: a second table for the workflow run, and the
  difference from the baseline per step.

## Domain relations

None new: the definition uses `loom.workflow` (`story:dw-workflow-definition-executor`) and the
report `loom.metering` (`story:dw-run-model-meter`).

## Acceptance

The bug-fix workflow reaches the merge approval with model calls only in `tests.write` and
`implementation.edit`, and the deterministic steps propose the same actions and arguments on a
re-run.

- `crates/loom-sdk/tests/bugfix_workflow.rs`
  `model_calls_happen_only_in_the_judgement_steps`: over a replayed recorded model, the report's
  steps other than `tests.write` and `implementation.edit` have zero calls, and the total equals the
  calls the port saw.
- `the_deterministic_steps_repeat_on_a_rerun`: two runs with the same replayed answers give equal
  proposals for every deterministic step.
- `the_workflow_meets_the_baseline_target`: the report's total calls are at most the page's target
  and below the baseline's total.
- `red_first_is_discharged`: the governor reports `red_first.required` discharged at the merge
  approval (as `software-change@1` decides it; needs the evidence order the slice submits).

## ESS first

No specification change: the story writes a definition in the types
`story:dw-workflow-definition-executor` declared. Its own test goes red first:
`model_calls_happen_only_in_the_judgement_steps` is committed with the definition before the example
runs it.

## Scope

- `crates/loom-sdk/examples/workflows/bugfix.yaml` (new), `crates/loom-sdk/examples/bugfix_workflow.rs` (new)
- `crates/loom-sdk/tests/bugfix_workflow.rs` (new), `crates/loom-sdk/tests/fixtures/bugfix-workflow.json` (new)
- `docs/qualification/workflow-baseline.md`
- `README.md`, `website/` (SDK page: a workflow example), `CHANGELOG.md`, `website/data/status.json`

Reads `crates/loom-intake-slice` through the SDK and changes nothing in it. Touches none of
`crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`, `ess/intake/`,
`generated/rust/intake`, `crates/loom-mandate`, `ess/domains/`, `generated/rust/loom`,
`Cargo.toml`, `Cargo.lock`.

Ready once `story:dw-bugfix-baseline` and `story:dw-typed-step-outputs` are implemented.

## Not in scope

The command line (`b10x-loom run`); the merge itself (it stays an approval); a resume after the
grant (`decision-blocker:dw-run-state-across-restart`).

## Open

- `red_first_is_discharged` needs the slice's local effects to submit `regression_test_result`
  and `implementation_change` evidence, which `story:change-evidence-order` adds; if that story is
  not implemented when this one starts, this criterion moves to it.
