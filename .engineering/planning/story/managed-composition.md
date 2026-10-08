---
format: aep.planning-md/3
id: story:managed-composition
kind: story
status: draft
title: Managed composition (deferred, unscheduled)
refs:
- provider: commission
  reference: story:managed-composition
relations:
- depends_on: epic:commission-core
- serves: vision:O1
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-commission/src/ports
- confidence: inferred
  path: crates/loom-sdk/examples
- confidence: inferred
  path: crates/loom-sdk/tests/example_runs.rs
- confidence: inferred
  path: docs/commission/design/commission-design.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
revision: 6
---
> Re-filed from `beyond10x/commission` `story:managed-composition` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

Commission can be composed from managed services as well as local ones, and the same agent
application code runs under either: the differences live behind the port adapters and the
composition root, not in the application.

**Deferred and unscheduled.** Atlas `epic:ga-aep-governor` (“Not in this epic”) puts managed
composition outside every phase of the Governed Autonomy roadmap; it is recorded here so it is not
lost, and stays in draft until a later epic takes it. Do not schedule it from this store.

## Acceptance

One example application that commissions an agent revision to a case builds against a local
composition and against a managed composition whose sources differ only in the composition root,
and both builds reach the same `RunOutcome` on the same case.

## Domain relations

None assumed. Which managed services Commission composes, and whether Agent Platform hosts
commissions, is not decided: `decision-blocker:managed-composition-host` blocks this story.

## Notes

- Local composition is the TASKBOARD M-009 runtime loop (`epic:commission-core`); managed
  composition replaces its adapters, not its application-facing API.
- Local users must not be forced through Agent Platform
  (build pack `docs/integrations/current-beyond10x-boundaries.md`, Agent Platform).

## Source

TASKBOARD M-012; build pack `REPOS.md` (“local/managed composition”);
`docs/history/beyond10x-agent-sdk-design-pre-commission-name.md` § 32–33 (“The same
agent/application logic should remain usable. The differences should largely live behind
adapters.”); `docs/ess/what-to-specify-with-ess.md` (“local-vs-managed parity”); Atlas
`epic:ga-aep-governor`, “Not in this epic”.
