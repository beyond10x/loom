---
format: aep.planning-md/3
id: story:change-evidence-order
kind: story
status: active
title: The slice submits evidence that order, independence and age rules can decide
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- depends_on: story:engineering-protocols-070-pin
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/case.rs
- confidence: cited
  path: crates/loom-intake-slice/src/effect.rs
- confidence: cited
  path: crates/loom-intake-slice/src/executor.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/intent.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-intake-slice/src/verifier.rs
- confidence: inferred
  path: ess/intake/domains/routing.yaml
- confidence: inferred
  path: generated/rust/intake
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:37:56Z", actor: "human:timo", revision: 15}
- {from: "proposed", to: "active", at: "2026-10-14T02:37:56Z", actor: "human:timo", revision: 16}
---
## Outcome

The slice's evidence lets `software.change/1`'s order, independence and age rules decide: an
`implementation.edit` submits `implementation_change` about the new `HEAD`, a bugfix case's test
runs after its first `tests.write` also submit `regression_test_result`, every record carries
`observed_at` from the host clock and the trusted `producer`, and a slice-built governor evaluates
at the host instant so `max_age` expires an old observation.

## Why

engineering-protocols `0.6.0` and `0.7.0` read evidence order (`red_first.required`,
`regression.shown_first`), producer independence (`security.reviewed`) and age
(`operational_observation` `max_age: 30m`). Loom `0.20.0` supplies the case inputs but none of
these: the slice submits only `test_result`, with no `observed_at` and no `producer` in the record,
`implementation.edit` submits nothing, and the slice's governor is built without an
`EvaluationTime`. A bugfix therefore leaves `red_first.required` open.

## Decision

- A `regression_test_result` comes from every test run on a `bugfix` case after the case's first
  `tests.write` that committed (a `tests.write` that commits nothing writes no regression test),
  beside that run's `test_result`; both rest on trusted inputs (the change kind and the run's own
  history), never on a model argument. One observation may yield both kinds.
- `producer` reaches Canon from the trusted `EvidenceData.producer` (`canon_record` sets it); a
  producer in the payload is removed before Canon reads the record, never read.
- `observed_at` is the slice's host-clock instant of the observation, written into the record; the
  governor stays clock-free.
- Canon orders at whole seconds; two records in one second order `unknown`. The executor keeps its
  observations' instants strictly increasing in whole seconds: it waits, at most 2 seconds, for the
  host clock to pass its previous observation's second, and a clock that has not passed it (one
  that stepped back) fails the step with `ExecuteError::Clock` before anything is recorded. The
  executor's clock is injectable so tests set the instants.
- The instant reaches the slice's governor by making the slice generic over `T: EvaluationTime`;
  `loom-governor`'s public API is unchanged.

## Acceptance

- A bugfix run that writes a test, runs it red, then edits discharges `red_first.required`
  (`crates/loom-cli/tests/slice_run.rs`
  `a_bugfix_red_regression_test_before_its_edit_discharges_red_first`).
- A bugfix whose suite fails before any `tests.write`, then edits, is admitted, and
  `red_first.required` stays open (`a_bugfix_edit_before_any_regression_test_leaves_red_first_open`).
- Every performed `implementation.edit` submits one `implementation_change` about the new `HEAD`;
  a refused edit submits none (`an_edit_submits_implementation_change_about_the_new_head`).
- Every slice record carries `observed_at` from the host clock and the trusted `producer`; a
  producer in the payload is ignored (`slice_evidence_carries_observed_at_and_the_trusted_producer`).
- A slice-built governor passes the host instant: an `operational_observation` older than 30
  minutes no longer applies
  (`a_slice_governor_expires_an_operational_observation_after_max_age`).
- With the default inputs and no `tests.write`, a run reaches the outcomes it reached on 0.20.0
  (`the_slice_stops_for_each_reason`, plus an evidence-count assertion).

## ESS first

The first commit changes only `ess/`: `ess/intake/domains/routing.yaml` declares the evidence the
slice submits (`intake.routing.SliceEvidence`: `kind` in `test_result`, `regression_test_result`,
`implementation_change`; `subject_revision`; `observed_at`; `producer`) and which run produces a
`regression_test_result`. On it `task intake-drift` fails; later commits regenerate and make
`a_bugfix_red_regression_test_before_its_edit_discharges_red_first` pass without changing `ess/`.

## Not in scope

A security review producer or the independence map's identities.
