---
format: aep.planning-md/3
id: upstream-blocker:substrate-run-observation-names-the-limit
kind: upstream-blocker
status: open
title: No pinned Substrate release has a run observation that names the limit that ended the run
summary: Holds story:child-launches-through-confinement-adapters until a Substrate release carries run-observation-names-the-limit-that-ended-it and process-count-limit-hit-is-observed
tags:
- sdc
relations:
- blocks: story:child-launches-through-confinement-adapters
revision: 4
---
## What is missing

`story:child-launches-through-confinement-adapters` moves Loom's child launches onto Substrate's
confinement adapters. Landing it removes the in-process limit reason Loom reports today, unless
the ended run's `RunObservation` names the limit that ended it. Loom pins Substrate `0.21.0`
(`adfde4020b14741b167532f46c7a0e9cebf2b3d3`, `crates/loom-intake-slice/Cargo.toml:31-32`); whether
that release names the limit was not read.

## What clears it

A Substrate release that carries both of these Substrate stories, pinned by Loom:

- `story:run-observation-names-the-limit-that-ended-it` (active there on 2026-11-21, first slice):
  on the in-process linux adapter, a run that exceeds its memory bound, its CPU budget or its
  timeout ends with an observation that names that limit, and a run that exits 0 names none;
  `memory_oom_kills` on linux and systemd.
- `story:process-count-limit-hit-is-observed`: a process-count hit does not end a run; the count
  is on the ended `RunObservation` and on `run_ended`.

## What Substrate's session said, as session `sdc` relayed it on 2026-11-21

- Output over its bound is truncated, not ended; `RunOutput` states the truncation.
- Usage figures (CPU time, memory peak, wall time) are in no Substrate story yet; a sibling is
  being drafted. They are not a condition of this blocker.

Not read by session `loom` in the Substrate repository: the two stories' text and their status.

## The other condition on the story

Its second critic pass. When that pass is recorded with its outcome, the story is scoped and this
blocker is cleared, the story moves to `proposed` under
`approval-record:sdc-requests-accepted-by-default-2026-11-21` (session `sdc`, 2026-11-21: "When
both hold it is accepted by the standing rule; no further word from me needed.").

## Substrate's own words, 2026-11-21 (session `substrate` to session `loom`)

- "the plan is in PR #238 (https://github.com/beyond10x/substrate/pull/238); the epic and the five
  first-slice stories are active, accepted by Timo." Session `loom` has not read the pull request.
- "the limit that ended a run (memory, CPU, wall time) comes with
  story:run-observation-names-the-limit-that-ended-it, the process-count hits with
  story:process-count-limit-hit-is-observed. Output over its bound is truncated and does not end a
  run."
- Two draft stories after the first slice, not conditions of this blocker:
  `story:run-observation-carries-usage` (wall time, CPU time, peak memory, peak process count on
  the ended `RunObservation` and on `run_ended`; linux, systemd) and
  `story:exec-file-changes-are-recorded` (per-exec file-change record, exact for concurrent execs,
  wire and in-process; the record `upstream-blocker:substrate-exec-file-change-record` waits for).
