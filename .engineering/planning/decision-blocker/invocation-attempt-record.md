---
format: aep.planning-md/3
id: decision-blocker:invocation-attempt-record
kind: decision-blocker
status: cleared
title: Nobody has decided whether an action request references the Connector attempt its invocation produced, or how many
refs:
- provider: commission
  reference: decision-blocker:invocation-attempt-record
relations:
- blocks: story:effect-invocation
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T00:06:18Z", actor: "human:timo", revision: 3, executor: "agent:loom"}
---
> Re-filed from `beyond10x/commission` `decision-blocker:invocation-attempt-record` at `e61e4f0` (status there: `open`) under Atlas ADR 0090
> (loom `story:import-commission`). Paths below are Commission's: `ess/` is now `ess/commission/`,
> `docs/` is now `docs/commission/`.

## Question

When the Commission runtime invokes an action request's effect (Atlas ADR 0082), does Commission
record the Connector attempt it produced, and how many attempts may one action request have (exactly
one, or retries)?

## Relation

`commission.responsibility.ActionRequest` -> `connectors.mutations.AttemptRecord`. Direction,
cardinality, ownership and lifecycle coupling: UNMAPPED. The Connectors side is typed (an
`AttemptRecord` references one Connection and one service configuration, connectors
`ess/domains/mutations.yaml`, per loom `story:connector-action-binding` § Domain relations at its
revision 1); nothing on the Commission side refers to it.

## What it stops

`story:effect-invocation`: the outcome its command returns after an invocation, and whether evidence
or completion can later cite the attempt.

## Clears when

An accepted decision states whether the request references its attempt(s) and the cardinality, and
the Commission ESS domain declares it as a `relations:` entry.

## Decision (2026-10-07)

Option A. An invoked action request references exactly one Connector attempt, the one its invocation
produced: `commission.responsibility.ActionRequest` -> `connectors.mutations.AttemptRecord`,
cardinality one, referenced (Connectors owns the attempt). A request refused at the recheck is
never invoked and references none. Commission does not retry an invocation: a retry is a new
action request, rechecked against the current frontier, case revision and authority.

The relation is declared in `ess/commission/` before `story:effect-invocation` implements it.
Option B (several attempts per request) is not taken because attempts after the first would skip
the recheck; option C (no reference) leaves completion unable to trace an effect to its request.
