---
format: aep.planning-md/3
id: story:incident-response-slice
kind: story
status: draft
title: 'Incident-response vertical slice: leave emergency mode with cause unknown'
refs:
- provider: commission
  reference: story:incident-response-slice
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: crates/loom-commission-testkit
- confidence: inferred
  path: crates/loom-governor/tests
- confidence: inferred
  path: crates/loom-governor/tests/fixtures/inc-492.fixture.yaml
- confidence: inferred
  path: crates/loom-governor/tests/incident_response_slice.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/effect.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
revision: 7
---
> Re-filed from `beyond10x/commission` `story:incident-response-slice` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Outcome

The second demonstrator, as an integration test: an `incident.response/1` case driven through
Commission and Loom on the AEP governor, showing that the kernel is not a software-delivery
workflow. The case becomes operationally recoverable while the investigation stays open.

Observable steps (build pack `START-HERE.md`, section Second demonstrator; ELS
`docs/examples/incident-response.md`; Atlas `epic:ga-vertical-slices` I-002):

1. the case starts with `customer_impact_bounded = True`, `service_healthy = False`,
   `cause_identified = Unknown` and the urgent obligation `restore_service`;
2. `release.rollback` (which requires approval) executes under a granted authority;
3. fresh health evidence is admitted;
4. the case reports `service_healthy = True`, `customer_impact_bounded = True`,
   `cause_identified = Unknown`, leaves emergency mode, and the investigation obligation stays open.

## Domain relations

- Commission -> Case, many-to-one, references: `ess/domains/responsibility.yaml`,
  `commission.responsibility.Commission` relation `case`.
- Frontier -> Case, many-to-one, references, carrying `case_revision`:
  `commission.responsibility.Frontier` relation `case`.
- Evidence -> Case, many-to-one, references: `commission.responsibility.Evidence` relation `case`.

## Acceptance

Driving the ELS `incident.response/1` fixture case through Commission and Loom on the AEP governor,
after `release.rollback` is executed under a granted authority and fresh health evidence is
admitted, the governor frontier reports `service_healthy = True`, `customer_impact_bounded = True`
and `cause_identified = Unknown`, reports the case out of emergency mode as `incident.response/1`
defines it, and still lists the investigation obligation as open.

## Tests

The tests live under commission `tests/` and drive ELS fixtures through Loom (pinned
dev-dependency). Loom is a dev-dependency pinned by `Cargo.lock`; the pin moves only in a story that
names the Loom change it takes and re-runs all three slices. No loom or els source changes here.

## Notes

- "Emergency mode" and the investigation obligation id are not defined by any ELS or Canon source
  yet: ELS `docs/examples/incident-response.md` uses the phrase and names only the urgent obligation
  `restore_service`. Both come from `incident.response/1` (TASKBOARD E-004); until it names them,
  that clause of the acceptance cannot be checked.
- Claim ids follow ELS `docs/examples/incident-response.md` and Atlas `epic:ga-vertical-slices`;
  `START-HERE.md` calls the first claim "impact mitigated".

## Source

TASKBOARD I-002; build pack `START-HERE.md` section Second demonstrator; Atlas
`epic:ga-vertical-slices`.
