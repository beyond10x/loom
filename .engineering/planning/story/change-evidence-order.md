---
format: aep.planning-md/3
id: story:change-evidence-order
kind: story
status: draft
title: The slice submits evidence that order, independence and age rules can decide
relations:
- decomposes: epic:vertical-slices
- serves: vision:O1
- depends_on: story:engineering-protocols-070-pin
revision: 1
---
## Outcome

The slice's evidence lets `software.change/1`'s order and independence rules decide: an
`implementation.edit` submits `implementation_change` about the implementation, a regression test
run can submit `regression_test_result`, every record carries `observed_at` from the trusted
clock and its `producer`, and the governor evaluates at a host instant so `max_age` expires an
old observation.

## Why

engineering-protocols `0.6.0` and `0.7.0` read evidence order (`red_first.required`,
`regression.shown_first`), producer independence (`security.reviewed`) and age
(`operational_observation` `max_age: 30m`). Loom `0.20.0` supplies the case inputs but none of
these, so a bugfix leaves `red_first.required` open and a security-boundary change can never be
verified.

## Acceptance

To be written when the story is scoped: a bugfix run that writes a red regression test before its
edit discharges `red_first.required`; an edit first leaves it open.

## ESS first

To be scoped: the evidence the slice submits is declared before it is produced.

## Not in scope

A security review producer or the independence map's identities.
