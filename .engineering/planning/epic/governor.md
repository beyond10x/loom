---
format: aep.planning-md/3
id: epic:governor
kind: epic
status: draft
title: 'Governor: Canon behind Commission''s governor and evidence ports'
refs:
- provider: governor
  reference: epic:governor
relations:
- serves: vision:governed-autonomy
revision: 1
---
> Re-filed from `beyond10x/governor` `epic:governor` at `81fcc1e` (status there: `draft`) under Atlas ADR 0090
> (loom `story:import-governor`).

## Outcome

A governor that owns a case's truth by evaluating its protocol with Canon, behind the ports
Commission defines: `Governor` (`current_revision`, `frontier`, `completion`) and `EvidencePort`.
Commission stays domain-neutral and never depends on Canon; ELS supplies the protocols.

## Acceptance

Against Commission's `Governor` and `EvidencePort` traits, the governor issues the frontier and the
completion Canon decides for the ELS `software.change/1` fixture case `chg-1842`, refuses a
proposal against a stale case revision, and keeps a case across a process restart (the last is a
later story; this epic's first story covers an in-process case).

## Source and placement

Atlas `epic:ga-aep-governor`, ADRs 0069, 0077 and 0079, and the Atlas ADR that places the governor
in its own repository (out of AEP and out of Commission, decided by the operator on 2026-10-04).
It replaces Commission's `epic:governor-adapter`, whose evidence-submission and evidence-adapter
conformance work moves here with it.

## First user

The vertical slice in beyond10x/intake (`story:case-frontier` there) opens its case through this
governor.
