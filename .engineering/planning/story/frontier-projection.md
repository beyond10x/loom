---
format: aep.planning-md/3
id: story:frontier-projection
kind: story
status: draft
title: Project frontier actions into model-visible tools
refs:
- provider: taskboard
  reference: L-003
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

Loom derives the model-visible tool list from the current frontier only: admissible and
approval-gated actions are projected, blocked ones are not, and nothing consequential is visible
because it was registered at startup (Atlas ADR 0072). Loom adds no protocol or engineering
semantics: it does not know that `repository.merge` waits on `tests.pass`; the frontier says so.

## Scope

- Projection is a function of the frontier and the runtime capability Loom is given; the
  `TurnEnvironmentProvider` seam of Harness (`harness-loop/src/environment.rs:47`) is the expected
  place, per `story:harness-module-map`.
- Tool membership only. The parameter schema each projected tool carries is not decided here: it is
  held by `decision-blocker:action-argument-schema` (filed by the `epic:fast-selector`
  decomposition). The intersection with available integrations in ADR 0072 is `epic:effect-bindings`.

## ESS first

Add the projection command on `loom.run.ActionCatalogue` with its outcome and the projected entries
(action id and status), validate with `ess specify validate --path ess`, and regenerate the
synthesized model. The marker on `ActionCatalogue` (`ess/domains/run.yaml:81`, turn or session
ownership) is not closed here: it is held by `decision-blocker:catalogue-ownership`, and this story
declares no owner relation for the catalogue.

## Domain relations

- `loom.run.ActionCatalogue` to frontier: one catalogue per frontier at one case revision —
  inferable from `ess/domains/run.yaml`, entity `loom.run.ActionCatalogue`, fields `frontier` and
  `case_revision` (fields; no relation is declared).
- Catalogue entries from `Frontier.actions`: one entry per projected candidate, the frontier owns its
  candidates — inferable (inferred from `canon/crates/canon/src/lib.rs:52`,
  `actions: Vec<ActionCandidate>`, at canon `cf29c4b`; no ess/1 document declares it).

## Acceptance

With the Commission fake governor serving the software-change frontier at a revision where
`tests.pass` is not `TRUE`, a `b10x-loom` test shows the tools Loom sends to the model contain no
`repository.merge`, and once the frontier moves to a revision where `tests.pass` is `TRUE` the next
projection contains it.

## Source

TASKBOARD L-003; Atlas ADR 0072; `docs/design/loom-design.md` § Commission integration; the catalogue
clause of `epic:loom-native-harness`.
