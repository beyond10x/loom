---
format: aep.planning-md/3
id: decision-blocker:selector-threshold-scope
kind: decision-blocker
status: open
title: Nobody has decided what a selector confidence threshold is calibrated per, or who owns its value
relations:
- blocks: epic:fast-selector
revision: 1
---
## Question

What is a confidence threshold calibrated per — a protocol, an ESS domain, a case type, an action
family, a tenant — and which component owns the calibrated value Loom is given (Loom configuration,
the Commission run, or a Metaharness calibration output)?

## Why it is open

`docs/integrations/laya-fast-selection.md` § Confidence policy says "Exact thresholds must be
calibrated per domain and measured by Metaharness" and gives 0.90 / 0.60 only as an example. No ess/1
document declares a threshold or the thing it is calibrated per, so the relation threshold → domain
has no cardinality and no owner. Canon, Commission and the Loom ESS draft carry no threshold field.

## What it stops

Supplying a calibrated, per-domain threshold to a real run. `story:confidence-fallback` takes the
threshold as an input and ships no default, so it does not depend on this answer; nothing that
chooses or loads a value is drafted.

## Source

Decomposition of `epic:fast-selector`; `docs/integrations/laya-fast-selection.md`; Atlas ADR 0073.
