---
format: aep.planning-md/3
id: decision-blocker:suspension-durable-record
kind: decision-blocker
status: open
title: Nobody has decided which system holds a suspended commission's approval request across a restart when AEP governs
refs:
- provider: commission
  reference: decision-blocker:suspension-durable-record
revision: 1
---
> Re-filed from `beyond10x/commission` `decision-blocker:suspension-durable-record` at `e61e4f0` (status there: `open`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Question

With AEP as the governor, which system holds a suspended commission’s durable record — the pending
approval request and the action and case revision it was raised for — so that a new process can
resume it: AEP’s engineering record for the case, a Commission-owned store, or the authority
provider?

## Relation

Suspension (with its approval request) -> the system that persists it across a process restart.
Ownership and lifecycle coupling: does the record live and die with the AEP case, or with the
Commission?

## Why nothing settles it

- `ess/domains/responsibility.yaml` declares no suspension entity; `Suspended` exists only as an
  `ExecutorOutcome` variant, and no relation names where a suspension is stored.
- The build pack history design (`docs/history/beyond10x-agent-sdk-design-pre-commission-name.md`
  § 23) says the “assignment becomes durably suspended” and the “approval request obtains stable
  ID”, without saying which system keeps either. ADR 0069 gives AEP the durable engineering record,
  and says nothing about runtime suspension.
- No code exists beyond the bootstrap crate.

Distinct from `decision-blocker:suspended-run-continuity` (whether the run or the commission carries
the suspension) and `decision-blocker:authority-decision-owner` (what a decision attaches to): this
one asks where the record is kept when AEP governs.

## What it stops

The durable suspend/resume part of TASKBOARD M-011 (build pack `ROADMAP.md` Phase 6), and with it
the governor-adapter epic’s acceptance clause “a suspend/resume across a process restart”.
