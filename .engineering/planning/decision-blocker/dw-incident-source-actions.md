---
format: aep.planning-md/3
id: decision-blocker:dw-incident-source-actions
kind: decision-blocker
status: cleared
title: Nobody has decided which incident.response/1 read action each incident data source is read under
relations:
- blocks: epic:deterministic-workflows
- blocks: story:dw-incident-workflow
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T02:57:52Z", actor: "human:timo", revision: 3}
---
## Question

Which `incident.response/1` read action is each incident data source read under?

`incident.response/1` (engineering-protocols at the pinned tag, `protocols/incident-response/1.yaml`)
declares three read actions: `metrics.inspect`, `logs.search` and `release.inspect`. A commission
binds one Connector operation per action (`ess/commission/domains/responsibility.yaml`,
`commission.responsibility.ActionBindingKey`: commission and action). The troubleshooting workflow
reads more sources than that at once: the configured tracker, chat, support and forge adapters
live, and call records and traces from fixtures. Changes to engineering-protocols are out of the
epic's scope.

Relation: incident data source -> `incident.response/1` read action, cardinality (one action per
source, or several sources under one action) and the binding that serves it, undecided.

## Options

- A. Several sources under one action: a host effect port reads `{source, kind}` arguments through
  `loom_connectors::cli`, as the plugin's `DataSourceEffects` does for `source.read`; one binding
  per action is kept.
- B. One binding per action and source: `ActionBindingKey` gains the source, a Commission change.
- C. A Loom-owned read protocol for troubleshooting (as `inbound-answer@1` is Loom's), with one
  read action per source kind, composed beside `incident.response/1`.

## Stops

The incident troubleshooting workflow story; the concurrent fan-out story does not depend on it.

## Decision (2026-10-14)

A. Several sources are read under one read action: one binding per action is kept, a host effect
port reads `{source, kind}` through `loom_connectors::cli` as the plugin's `DataSourceEffects`
does, and the source travels in the effect input. `ActionBindingKey` is unchanged, so hosts see no
break in the Commission contract; no second protocol.
