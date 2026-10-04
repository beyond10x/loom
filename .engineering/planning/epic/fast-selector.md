---
format: aep.planning-md/3
id: epic:fast-selector
kind: epic
status: proposed
title: Fast action selection inside the frontier
summary: Reasoning selector, Laya experiment, thresholds, hierarchy and fallback; the I-004 slice.
refs:
- provider: atlas
  reference: epic:ga-fast-selector
relations:
- depends_on: epic:loom-native-harness
- serves: vision:governed-autonomy
- serves: vision:O1
- serves: vision:O3
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-04T00:01:26Z", actor: "human:timo", revision: 2}
---
## Outcome

Pluggable ActionSelector strategies. Covers TASKBOARD L-007 … L-010 and I-004.

## Acceptance

In the I-004 slice the Laya selector chooses among a known candidate set while the reasoning model
only generates arguments; below-threshold selections fall back; out-of-set selections are rejected;
unauthorized-action attempts at the execution boundary stay at 0 in the selection telemetry.

## Source

Atlas `epic:ga-fast-selector`; Atlas ADR 0073; `docs/integrations/laya-fast-selection.md`.
