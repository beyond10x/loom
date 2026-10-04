---
format: aep.planning-md/3
id: story:frontier-projection
kind: story
status: draft
title: Project frontier actions into model-visible tools
refs:
- provider: commission
  reference: taskboard:M-002
- provider: taskboard
  reference: L-003
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:harness-module-map
- depends_on: story:run-pipeline-skeleton
scope:
- confidence: inferred
  path: crates/loom/src/projection.rs
- confidence: inferred
  path: crates/loom/tests/frontier_projection.rs
revision: 9
---
## Outcome

Loom derives the model-visible catalogue from the current frontier only: admissible and
approval-gated actions are projected, blocked ones are not, and nothing consequential is visible
because it was registered at startup (Atlas ADR 0072). Loom adds no protocol or engineering
semantics: it does not know that `repository.merge` waits on `tests.pass`; the frontier says so.

The frontier is Commission's: `commission.responsibility.Frontier`, whose contents commission
`story:ess-hard-gate` declares in Commission's own ESS specification as Commission value types
(`FrontierAction` with its `ActionStatus`, `FrontierClaim`, `FrontierObligation`). Canon's bootstrap
`Frontier`, `ActionId`, `ActionStatus` and `ActionCandidate` are deleted by Canon's wave-1 change and
are not used.

## Scope

- Projection is a function in `b10x-loom` of the frontier and the runtime capability Loom is given.
  `story:harness-module-map` names the Harness seam it is later called from
  (`TurnEnvironmentProvider`, `harness-loop/src/environment.rs:47` at `798325f0`);
  `story:harness-loop-port` wires it there. This story does not touch the loop.
- Tool membership only. The parameter schema each projected tool carries is held by
  `decision-blocker:action-argument-schema`. The intersection with available integrations in ADR
  0072 is `epic:effect-bindings`.
- Files: `crates/loom/src/projection.rs` (created empty by `story:run-pipeline-skeleton`, filled
  here) and `crates/loom/tests/frontier_projection.rs` (new). Not `crates/loom/src/lib.rs`, `ess/`
  or `generated/`.

## Shared surface

Depends on `story:run-pipeline-skeleton` (the declarations below and the `projection` module),
`story:agent-executor` (the generated model and the Commission frontier seam) and
`story:harness-module-map` (the seam above). It shares no file with `story:harness-crate-port`
and can run beside it. `story:action-selector` depends on it for behaviour: the selector selects
over the catalogue this story projects. The whole order is in `story:agent-executor` § Shared
surface.

## ESS first

- **Declarations relied on** (landed by `story:run-pipeline-skeleton`): `loom.run.ProjectCatalogue`,
  `loom.run.CatalogueEntry` (action id and status), `loom.run.CatalogueEntryStatus` (`Admissible`,
  `ApprovalRequired`) and `ActionCatalogue.entries`. This story does not change `ess/`; if the
  projection needs an entry field beyond action id and status, it stops and reports it.
- **Red test:** the first commit adds `projection_follows_frontier` in
  `crates/loom/tests/frontier_projection.rs`; it fails on that commit because
  `crates/loom/src/projection.rs` holds no projection function yet. The implementation commit
  makes it pass.

The catalogue's owner is settled, not declared here:
one catalogue per turn (operator decision 2026-10-04 on `decision-blocker:catalogue-ownership`),
modelled by `story:ess-hard-gate` as `ActionCatalogue.turn_id` and the relation `catalogue` on
`loom.run.Turn` (owns, one). A projected catalogue carries the `turn_id` of the turn it is
projected for.

## Domain relations

- `loom.run.Turn -> loom.run.ActionCatalogue`: owns, one, via `turn_id` — declared by
  `story:ess-hard-gate` per the operator decision of 2026-10-04 on
  `decision-blocker:catalogue-ownership`.
- `loom.run.ActionCatalogue -> commission.responsibility.Frontier`: one catalogue per frontier at
  one case revision — inferable from `ess/domains/run.yaml`, entity `loom.run.ActionCatalogue`,
  fields `frontier` and `case_revision`, and from commission `ess/domains/responsibility.yaml:182-203`
  at `013e392` (`commission.responsibility.Frontier`, one per case revision). A field, not a
  `relations:` entry: ESS 0.52.0 cannot name another system's entity.
- Catalogue entries from the frontier's actions: a frontier holds its actions as
  `commission.responsibility.Frontier.actions: List<commission.responsibility.FrontierAction>` —
  one frontier, many actions — declared by commission `story:ess-hard-gate` (§ ESS). A projected
  catalogue has one entry per `FrontierAction` whose `status` is `Admissible` or
  `ApprovalRequired`, and none for one that is `Blocked`.

## Depends on, outside this store

`commission:story:ess-hard-gate`, for the frontier's actions and their statuses;
`commission:story:governor-port` (M-003) and `commission:story:local-runtime-loop` (M-009), for the
fake governor that serves it.

## Acceptance

The test `projection_follows_frontier` in `crates/loom/tests/frontier_projection.rs` passes. With
the Commission fake governor serving the software-change frontier, it checks:

1. At a revision where `tests.pass` is not `TRUE`, the projected catalogue contains no
   `repository.merge`.
2. After the fake governor moves to a revision where `tests.pass` is `TRUE`, the next projection
   contains `repository.merge`.
3. An action the frontier marks blocked is never in a projected catalogue; an approval-gated one is.
4. Each catalogue's `case_revision` equals the case revision of the frontier it was projected from.

## Source

TASKBOARD L-003; Atlas ADR 0072; `docs/design/loom-design.md` § Commission integration; the catalogue
clause of `epic:loom-native-harness`.
