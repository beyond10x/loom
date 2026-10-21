---
format: aep.planning-md/3
id: decision-blocker:ess-existing-selection-guard
kind: decision-blocker
status: open
title: ESS synthesis cannot reach a guard on an existing selection id
relations:
- blocks: story:refused-selection-stays-refused
withholds: test_result
revision: 1
---
## Question

How does `loom.run.SelectAction` declare "a selection id the run already holds is refused" so that
ESS synthesis covers it? ESS 0.67.0 refuses the declaration. Settling this needs an ESS release
whose synthesis can reach the guard, or a decision to declare the guard differently.

## What was observed

ESS 0.67.0 (the pin), on a scratch copy of `ess/` with a `selection-exists` outcome
(`existing_instance: true`) and a `loom.run.SelectionExists` error added to `SelectAction`:

```text
refused: refusal[ESS-SYNTH-004]: outcome loom.run.SelectAction/selection-exists has no scenario `loom.run.SelectAction/outcome/selection-exists`
  it needs an instance of `loom.run.Selection` resting in `Selected`, and the route runs through `loom.run.SelectAction/selected`, which no input reaches
  help: the route to this state runs through a branch no input reaches; see that outcome's own refusal
29 scenario(s) (0 authored), 1 refusal(s)
```

`ess specify validate --strict-requires` and `ess specify compile` exit 0. The unchanged
specification synthesizes 29 scenarios with 0 refusals.

| variant | refusals |
|---|---|
| both catalogue guards kept (the real specification) | 1 |
| plus an authored `ess-scenario/1` for the branch | 1 |
| `not-in-catalogue` removed | 1 |
| `revision-mismatch` removed | 1 |
| both removed | 0 |

Inferred from the ESS 0.67.0 source, not traced at runtime: to reach "the id already exists",
synthesis first creates a `Selection` from an input it picks, then builds a fresh catalogue row
that nothing steers toward that input's `case_revision` or `action`, so neither catalogue
predicate lets `selected` through.

## Options

- An ESS release whose existence check builds the catalogue row to match the chosen input; the
  ESS repository tracks it in its own store.
- Declaring `not-in-catalogue` and `revision-mismatch` as `external:` outcomes. Not chosen: it
  weakens conformance for both.

## What it blocks

`story:refused-selection-stays-refused`: its first commit is the `ess/domains/run.yaml` change,
and the ESS gate refuses that change while synthesis refuses it. The test
`a_refused_selection_is_never_selected_or_admitted_again` stays ignored until this clears.

Runtime note for that story: `selection::select_action` replaces a selection held under the same
id, and `Loom::prepare` stores a selection without checking the id. Ids come from a per-`Loom`
counter; no path through `Loom::run` that reuses one is known.
