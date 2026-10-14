---
format: aep.planning-md/3
id: story:dw-bugfix-baseline
kind: story
status: draft
title: The model-driven bug-fix loop has a measured baseline of model calls, tokens and cost
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- depends_on: story:dw-run-model-meter
revision: 1
---
## Outcome

The bug-fix loop has a recorded baseline: one governed bug-fix run on `software-change@1` (case
input `change_kind: bugfix`), driven today by the model choosing every step, measured with the run
meter, and a page that gives its model calls, tokens and cost per step and in total, with the
target the deterministic bug-fix workflow must meet. A test replays the recorded run and fails when
the measured figures differ from the page.

## Why

The epic promises a baseline and a target that goes down. Without a measured baseline a
deterministic workflow cannot show what it saved, and a later change that adds a model call goes
unnoticed.

## Design

- `crates/loom-sdk/examples/bugfix_baseline.rs` (new): the SDK example's embedding
  (`crates/loom-sdk/examples/software_change.rs`), with `LoopExecutor` over a live `ModelPort`
  (the executor's own wire, `crates/loom-executor/src/harness/responses/`, credential through
  `crates/loom-executor/src/credentials.rs`) instead of the scripted selector, behind
  `MeteredModelPort` and `MeteredExecutor`. The scratch repository holds one failing regression
  (`grep -qx fixed check.txt` over `broken`). With `--record <file>` it writes every turn's request
  digest and `TurnOutcome`, usage included, to a fixture.
- `crates/loom-sdk/tests/fixtures/bugfix-baseline.json` (new): one recorded live run.
- `docs/qualification/workflow-baseline.md` (new): the run's date, model, rate card (`source`,
  `as_of`), and a table per step: action proposed, model calls, input, cached and output tokens,
  micro-dollars; then the totals. Then the bug-fix target: model calls only in the judgement steps
  `tests.write` and `implementation.edit`; `repository.inspect`, both `tests.run` and the merge
  request take none. The incident troubleshooting baseline row says it is measured with its
  workflow story, because no model-driven incident run exists today.

## Domain relations

None new: the story reads the report `story:dw-run-model-meter` declares.

## Acceptance

Replaying the recorded bug-fix run reports, per step and in total, the model calls, tokens and cost
the baseline page gives.

- `crates/loom-sdk/tests/workflow_baseline.rs`
  `the_recorded_bugfix_run_reports_the_baseline_per_step`: the example's `run` over a port that
  replays `bugfix-baseline.json` ends at the merge approval, and the meter's report equals the
  table in `docs/qualification/workflow-baseline.md`, parsed from the page.
- `the_baseline_page_names_a_target_per_reference_workflow`: the page has a target row for the
  bug-fix loop and for the incident lookup.
- The live recording is run once by the implementor; the command and its output are pasted into the
  page.

## ESS first

No specification change: the story measures with types `story:dw-run-model-meter` declared. Its own
new test goes red first: `the_recorded_bugfix_run_reports_the_baseline_per_step` is committed with
the fixture and the page, before the example's metering is wired.

## Scope

- `crates/loom-sdk/examples/bugfix_baseline.rs` (new)
- `crates/loom-sdk/tests/workflow_baseline.rs` (new), `crates/loom-sdk/tests/fixtures/bugfix-baseline.json` (new)
- `docs/qualification/workflow-baseline.md` (new)
- `CHANGELOG.md`

Reads `crates/loom-intake-slice` through the SDK's re-exports and changes nothing in it. Touches
none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`, `ess/intake/`,
`generated/rust/intake`, `crates/loom-mandate`, `ess/domains/`, `generated/rust/loom`,
`Cargo.toml`, `Cargo.lock` (the SDK crate's own manifest gains no dependency).

Ready now, after `story:dw-run-model-meter`.

## Not in scope

The incident baseline (no model-driven incident run exists); a hosted evaluation; comparing other
harnesses (Metaharness's).

## Open

- The live recording costs one paid run of the configured model; the implementor needs the
  operator's credential reference to make it.
