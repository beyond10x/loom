---
format: aep.planning-md/3
id: epic:plugin-layer
kind: epic
status: draft
title: 'Unattended agents are Loom plugins: a Slack handler first'
relations:
- serves: vision:O1
- implements: architecture-decision-record:plugin-hooks
revision: 3
---
## Outcome

An operator runs `b10x-loom plugin run slack-handler --config <file> --state <dir>` and, without
further input, Loom walks the Slack channels its bot belongs to, finds messages nobody answered,
classifies each (ask, request, task, find), answers ask, find and request in a governed read-only
turn over Connectors data sources, and writes one record line per item;
`b10x-loom plugin report slack-handler --state <dir>` prints the proposals. Nothing is posted to
Slack.

## Acceptance

Each line is decided by the named test of the named story:
- the fixture run records 3 proposals: `story:slack-plugin`, `fixture_run_records_three_proposals`;
- it posts nothing: `story:slack-plugin`, `fixture_run_invokes_no_write`;
- a second run adds no line: `story:slack-plugin`, `second_run_records_nothing_new`;
- `plugin run slack-handler` resolves through the CLI: `story:slack-plugin`,
  `plugin_run_slack_handler_once`;
- the protocol offers no write action: `story:inbound-answer-protocol`, `frontier_offers_no_write`;
- a read outside the turn's projection is refused and invokes nothing: `story:plugin-host`,
  `effects_refuse_an_undeclared_read` (the projection never exceeds the configured sources:
  `project_stays_within_configured_sources`);
- the measurement page's counts equal a recount on the merge commit:
  `story:plugin-measurement`, `measurement_counts_match`;
- every module of the measured crates has a row: `story:plugin-measurement`,
  `measurement_table_is_complete`.

## Deviation from the ADR, recorded

`architecture-decision-record:plugin-hooks` said reads go behind Commission's `ConnectorInvoker`.
They go through a plain Connectors CLI client behind `DataSourceEffects`, a Commission `EffectPort`:
the poll runs outside any run (an `AdmittedRequest` exists only inside one), and a turn's read names
its operation per call where a binding is fixed per commission (design critic, round 1). The ADR
body carries the amendment.

## Deferred

The ADR's last step, expressing what generalises as YAML (a YAML-only plugin, and reference
enrichment before a turn), is not in this epic. It follows the measurement page; the idea stories
`yaml-only-plugin` and `reference-enrichment` (filed on wave `2026-10-09-w1`) carry it.

Source: the operator's approved plan (conductor `charters/loom.md`, DSP-20261009-05); `architecture-decision-record:plugin-hooks`.
