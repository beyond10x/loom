---
format: aep.planning-md/3
id: story:fallback-selection-recording
kind: story
status: draft
title: A confidence fallback is recorded as two linked selections
relations:
- decomposes: epic:fast-selector
- depends_on: story:confidence-fallback
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A confidence fallback is recorded as `decision-blocker:fallback-selection-record` (option B)
decided: two `loom.run.Selection`s, the fast selection and the stronger selector's selection that
replaced it. The fast one references its replacement, zero or one; a fast selection that was not
overruled has none. Both belong to the run's turn.

ESS first: `ess/domains/run.yaml` gains the relation from `loom.run.Selection` to the selection
that replaced it, and the way it is written (an optional input of `loom.run.SelectAction`, or a
command that marks a fast selection overruled). Neither is proven expressible at the system's
`format: ess/20`: no relation in this repository goes through an `Optional` field, and a guard on
an optional input needs a later format. Validate on a copy first; if ESS refuses, stop and report.

## Acceptance

When the fast selection is overruled, Loom's record holds both selections and the fast one names
the replacement's `selection_id`; when it is not overruled, the record holds one selection with no
replacement; a fast selector that errs or names an action outside the candidates leaves no fast
selection (`selection::chosen` refuses it before a `Selection` exists).

## Dependencies

`story:confidence-fallback` (the hybrid selector this records). `story:selection-telemetry`
records `fell_back_to` on the same selections and follows this story.

## Source

Split out of `story:confidence-fallback` at scoping, wave 2026-10-09-w1: the story's acceptance
checks only which choice is returned, and the recording needs a specification change that is not
yet shown to validate.
