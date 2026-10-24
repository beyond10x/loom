---
format: aep.planning-md/3
id: story:refused-selection-stays-refused
kind: story
status: implemented
title: A refused selection stays refused under a reused selection id
relations:
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/tests/adversary2_run_identity.rs
- confidence: cited
  path: ess/domains/run.yaml
- confidence: inferred
  path: generated/rust/loom
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T13:40:11Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-21T13:40:11Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-24T02:59:14Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A selection that revalidation refused stays `Refused`: `SelectAction` sent again under its id does
not return it to `Selected`, and a later `RevalidateSelection` does not admit it. The ignored test
that says so runs in `task check`.

## Why

`ess/domains/run.yaml` says a selection is revalidated once and `Refused` is terminal. The
generated behaviour does not hold it. On `05699aae`,
`cargo test -p b10x-loom-executor --locked --test adversary2_run_identity -- --ignored` fails:

```text
SelectAction for the refused selection id 10 answered Selected { … action: "repository.merge" }
the second revalidation of selection 10 answered Admitted { … }
selection 10 now rests in Some(Admitted), not Refused
```

The test `a_refused_selection_is_never_selected_or_admitted_again`
(`crates/loom-executor/tests/adversary2_run_identity.rs:288`) is ignored with
`story:interruption-recovery; ESS-SYNTH-004`. That story is `implemented`, so nothing tracks the
ignore any more and CI never runs the test.

## Design

- `ess/domains/run.yaml`: `SelectAction` refuses a selection id the run already holds, whatever
  its state, and `RevalidateSelection` answers a selection that is not `Selected` without moving it.
- Whether ESS 0.67.0 synthesis expresses the guard that `ESS-SYNTH-004` names: I don't know. If it
  does not, the story files a `decision-blocker` against the ESS repository instead of ignoring
  the test again.
- Check that `Loom::select` and the runtime revalidation path refuse a reused selection id
  independently of the generated behaviour, and add a test there if they do not.

## ESS first

The first commit changes only `ess/domains/run.yaml`. On it the test
`a_refused_selection_is_never_selected_or_admitted_again`, with its `#[ignore]` removed, fails
against the regenerated behaviour until `task generate` and the implementation follow; the run is
recorded.

## Acceptance

- `crates/loom-executor/tests/adversary2_run_identity.rs`
  `a_refused_selection_is_never_selected_or_admitted_again` runs without `#[ignore]` and passes
  in `task check`.
- `task ess-gate` and `task drift` exit 0.

## Scope

- `ess/domains/run.yaml`
- `generated/rust/loom/` (regenerated)
- `crates/loom-executor/tests/adversary2_run_identity.rs`
