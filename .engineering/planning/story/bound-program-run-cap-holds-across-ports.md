---
format: aep.planning-md/3
id: story:bound-program-run-cap-holds-across-ports
kind: story
status: draft
title: A bound program's run cap holds when two ports drive one case
summary: The case store reserves a run before a bound program starts, so two ports over one case start a capped action at most max_runs times
tags:
- sdc
relations:
- depends_on: story:bound-program-result-table
- decomposes: epic:protocol-tree-on-bindings
revision: 1
---
## Outcome

An action bound to a program starts at most its `budget.max_runs` times in one case, also when two
effect ports drive that case over one case store at the same time. Today the port of
`b10x-loom-bindings` reads the number of observations and then starts the program; a second port
that reads between the two starts the action once more.

The case store takes a reservation for a run before the program starts: the count is read and the
reservation written under one hold of the store, and a port whose reservation is refused answers
`RunsSpent` and starts nothing. How the reservation is recorded (an observation written before the
start and completed after it, or a record of its own) is open.

## Why

The security review of `story:bound-program-result-table` (wave 2026-11-21-w7, pass 1, finding F3,
`review-result:bound-program-result-table-security-1`) showed it with a test: two ports over one
`FileCaseStore` both start a capped action at `max_runs - 1`. `LockedDir` documents two processes
over one store as supported. The finding was not closed in that wave, because the fix is a change
in `crates/loom-governor/`, which that story does not own; the wave left the reviewer's case in
`crates/loom-bindings/tests/security_w20261121w7_u2.rs` under `#[ignore]` with this story's id,
and the guide states the limit.

## Acceptance

- The reviewer's case `two_ports_over_one_case_store_start_a_capped_action_at_most_max_runs_times`
  runs without `#[ignore]` and is green.
- A port whose reservation is refused starts no program and answers `RunsSpent`.
- A run that started and left no result still holds its reservation, so it counts.
- The sentence on two hosts leaves `website/docs/guides/bindings.md` and AGENTS.md § Bindings
  document.

## Open

- Where the reservation lives: `ObservationData` written before the start, or a new record kind.
- Whether the intake slice's bound-command path (`crates/loom-intake-slice/src/bindings.rs`) has
  the same window, and whether this story closes it there too.
- Whether a reservation whose host died before the start is ever given back.

## Scope

Not derived. `crates/loom-governor/` (the case store), `crates/loom-bindings/src/program.rs`,
`crates/loom-bindings/tests/security_w20261121w7_u2.rs`, `ess/domains/bindings.yaml`.
