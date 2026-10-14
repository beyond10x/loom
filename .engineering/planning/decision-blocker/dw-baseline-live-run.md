---
format: aep.planning-md/3
id: decision-blocker:dw-baseline-live-run
kind: decision-blocker
status: open
title: Nobody has approved the paid live model run that records the bug-fix baseline
relations:
- blocks: story:dw-bugfix-baseline
revision: 1
---
## Question

May the implementor of story:dw-bugfix-baseline make one paid live model run to record the
model-driven bug-fix baseline, and with which credential reference and model?

The baseline is one governed bug-fix run on `software-change@1` with every step chosen by the
model, recorded once and replayed by the test. Tests never call a model; the recording does.

## Options

- A. One live run through the operator's Codex login (read and renewed by llm, as `b10x-loom run`
  does), on the Codex model `b10x-loom run` uses by default, recorded once; any re-recording asks
  again. Cost: one run of a few model turns.
- B. A route from an llm catalog the operator names (`--catalog`), on a model they choose. Cost:
  the same run; needs the catalog file and route alias.
- C. No live run: the baseline is a scripted model whose turn count is set by hand. Cost: the
  baseline measures nothing real; the epic's target is set against a guess.

Recommendation: A.

## Stops

story:dw-bugfix-baseline, and through it story:dw-bugfix-workflow and story:dw-incident-workflow.
