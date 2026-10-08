---
format: aep.planning-md/3
id: story:incident-response-slice
kind: story
status: active
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
  path: crates/loom-governor/tests/fixtures/inc-492.fixture.yaml
- confidence: inferred
  path: crates/loom-governor/tests/incident_response_slice.rs
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T17:26:32Z", actor: "human:timo", revision: 12}
- {from: "proposed", to: "active", at: "2026-10-08T17:26:32Z", actor: "human:timo", revision: 13}
---
> Re-filed from `beyond10x/commission` `story:incident-response-slice` at `e61e4f0` (status there: `draft`) under Atlas ADR 0090 (loom `story:import-commission`); paths mapped to Loom.

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

- Commission -> Case, many-to-one, references: `ess/commission/domains/responsibility.yaml`,
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

The test lives under `crates/loom-governor/tests/` and drives the ELS `inc-492` fixture
(`fixtures/incident-response/inc-492.fixture.yaml` in beyond10x/engineering-protocols) through
Commission's runtime (`crates/loom-commission`) and the governor (`crates/loom-governor`,
`CanonGovernor`), using the port fakes in `crates/loom-commission-testkit`. As with
`tests/fixtures/chg-1842.fixture.yaml`, the fixture is copied byte for byte from the
`b10x-canon-engineering` release pinned in `Cargo.lock` (tag `0.1.0`). That pin moves only in a
story that names the protocol change it takes and re-runs all three slices. This story changes
tests only: no Loom crate source and no engineering-protocols source.

## Notes

- "Emergency mode" and the investigation obligation id are not defined by any ELS or Canon source
  yet. At the pinned tag `0.1.0`, `protocols/incident-response/1.yaml` partly settles it: leaving
  emergency mode is the action `emergency.leave`, admissible once `service.healthy` and
  `impact.bounded` are true; the only obligation is `restore_service`, so there is no investigation
  obligation, and that clause of the acceptance cannot be checked until the protocol names one.
- The pinned protocol's claim ids are `impact.bounded`, `service.healthy` and `cause.identified`;
  the acceptance keeps the ELS example's names. `START-HERE.md` calls the first claim "impact
  mitigated".
- Unchecked: whether `inc-492` ends with the cause unknown and the service healthy.

## Source

TASKBOARD I-002; build pack `START-HERE.md` section Second demonstrator; Atlas
`epic:ga-vertical-slices`.
