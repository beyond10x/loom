---
format: aep.planning-md/3
id: decision-blocker:managed-composition-host
kind: decision-blocker
status: open
title: Nobody has decided which managed services Commission composes, or whether Agent Platform hosts commissions
refs:
- provider: commission
  reference: decision-blocker:managed-composition-host
relations:
- blocks: story:managed-composition
revision: 2
---
> Re-filed from `beyond10x/commission` `decision-blocker:managed-composition-host` at `e61e4f0` (status there: `open`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Question

Which managed services does a managed Commission compose (the history design lists Agent Platform,
AEP Service, Mandate, Identity, Connectors, Secrets, remote Harness workers, Substrate and event
infrastructure), and does Agent Platform host commissions — owning agent revisions and the
commissions bound to them — or only serve as one adapter among several?

## Relation

Commission (and AgentRevision) -> a managed host / control plane. Ownership and lifecycle
coupling: whether a managed commission exists only inside the host, and whether the host or
Commission owns agent revisions in managed mode.

## Why nothing settles it

- `ess/domains/responsibility.yaml` declares no host or control-plane entity and no relation to
  one.
- Build pack `docs/integrations/current-beyond10x-boundaries.md`: Agent Platform is “Likely managed
  host/control plane for Commission concepts” — “likely” is not a decision.
- `docs/history/beyond10x-agent-sdk-design-pre-commission-name.md` § 33 lists services managed mode
  “may compose”.

## What it stops

`story:managed-composition` (TASKBOARD M-012), which is deferred and unscheduled in any case.

## Status (2026-10-07)

Left open on 2026-10-07: nothing scheduled depends on it, and `story:managed-composition` stays
deferred and unscheduled. The blocker stays open.
