---
format: aep.planning-md/3
id: epic:plugin-layer
kind: epic
status: draft
title: 'Unattended agents are Loom plugins: a Slack handler first'
relations:
- serves: vision:O1
- implements: architecture-decision-record:plugin-hooks
revision: 2
---
## Outcome

An operator runs `b10x-loom plugin run slack-handler --config <file> --state <dir>` and, without
further input, Loom walks the Slack channels its bot belongs to, finds messages nobody answered,
classifies each (ask, request, task, find), answers ask, find and request in a governed read-only
turn over Connectors data sources, and writes one record line per item;
`b10x-loom plugin report slack-handler --state <dir>` prints the proposals. Nothing is posted to
Slack.

## Acceptance

Each line is decided by the test the named story lists:
- the fixture run records 3 proposals (`story:slack-plugin`, `fixture_run_records_three_proposals`);
- the same fixture run posts nothing: the fake `connectors` saw no write operation
  (`story:slack-plugin`, `fixture_run_invokes_no_write`);
- a second run over the same fixtures records no new line (`story:slack-plugin`,
  `second_run_records_nothing_new`);
- the inbound protocol offers no write action (`story:inbound-answer-protocol`,
  `frontier_offers_no_write`);
- the data-source effect port refuses a pair outside the configured sources (`story:plugin-host`,
  `effects_refuse_an_undeclared_read`);
- the measurement page gives line counts per crate (`story:plugin-measurement`);
- the measurement page classifies every module as mechanism or policy (`story:plugin-measurement`).

## Deferred

The ADR's last step, expressing what generalises as YAML (a YAML-only plugin, and reference
enrichment before a turn), is not in this epic. It follows the measurement page; the idea stories
`yaml-only-plugin` and `reference-enrichment` (filed on wave `2026-10-09-w1`) carry it.

Source: the operator's approved plan (conductor `charters/loom.md`, DSP-20261009-05); `architecture-decision-record:plugin-hooks`.
