---
format: aep.planning-md/3
id: story:program-reports-its-cost
kind: story
status: proposed
title: A bound program can report what its step cost, and the run, the log and the status carry it
summary: usage_from pointer into the report; optional cost_micro_usd on ProgramRun, ActionFinished and status runs[], absent when unnamed; no budget acts on it yet
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:loom-sdk-for-products
- depends_on: story:bound-program-result-table
- depends_on: story:child-record-in-the-run-log
- depends_on: story:tree-status-names-what-it-read
- depends_on: story:handoff-when-a-program-decides
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: cited
  path: crates/loom-bindings/src/program.rs
- confidence: cited
  path: crates/loom-runlog
- confidence: cited
  path: crates/loom-tree/src/status.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:59Z", actor: "agent:loom", revision: 5}
---
## Outcome

A bound program can report what its step cost, and the run, the log and the status carry it.

- A program binding may name `usage_from: <JSON pointer>` into its report, the one JSON object on
  standard output within `MAX_REPORT_BYTES` that evidence pointers already read
  (`crates/loom-bindings/src/program.rs:39-47`).
- The value there must be a whole number of micro-USD, at least 0. Anything else is refused
  `ReportUnreadable` (`program.rs:447` is today's site) and records nothing, as an unreadable
  report does today:
  - an absent pointer;
  - a fraction, a negative number, a string, or a number over `i64`.
- `loom.bindings.ProgramRun`, `loom.runlog.ActionFinished` and the `runs[]` entries of `tree
  status` (`loom.tree.StatusRun`) gain an optional `cost_micro_usd`.
  - It is present only when the binding names `usage_from` and the value read.
  - It is absent otherwise, and never written as 0 for unknown.
  - The ESS comment of each says the program reported it, so it is labelled program-reported.
  - The port reads the pointer from the report it already holds, before it writes the run's
    observation (inferred order; the unit confirms it in `program.rs`).
- No budget acts on it. `tree run` stops on `--max-steps` and `--max-wall-seconds` only.

The port's `usage` (`program.rs:657-666`, today `usage: None`) is Commission's `EffectUsage`,
whose `input_tokens`, `output_tokens` and `duration_ms` are required integers
(`generated/rust/commission/src/responsibility.rs:391-402`). Filling it would write 0 tokens for
unknown.

- Default (a): `usage` stays `None`, and the cost is carried by the three members above.
- Alternative (b): fill `usage` with the cost and tokens 0. This breaks "never 0 for unknown".
- (c): first make the token members optional in Commission. That is a contract change of
  Commission.

Security-relevant: no. The value is a number the program prints, read through the same bounded
report. It grants nothing.

## Why

Runbook needs the cost of model calls its programs make. In a tree only `max_runs` acts:

- `max_turns`, `max_tokens` and `max_budget_micro_usd` are refused on any executor but an agent
  (`crates/loom-bindings/src/load.rs:266-291`);
- agent bindings are refused at `tree open`;
- no cost member exists on `ProgramRun` (`ess/domains/bindings.yaml:311-329`), `ActionFinished`
  (`ess/domains/runlog.yaml:93-108`) or `StatusRun` (`ess/domains/tree.yaml:205-217`).

`story:tree-usage-and-model-budget` covers agent cost and a tree budget, and excludes a cost for
program steps.

Readers:

- `tree status` is read by name and leaves unknown members alone, so an old reader reads the new
  status.
- The run-log decoder is closed, so a Loom from before this change refuses an `ActionFinished`
  with `cost_micro_usd`. This is handled as in `story:child-record-in-the-run-log`: `run-log-v1`
  stays, and `CHANGELOG.md` says older readers refuse the new lines.
- Runbook's decoders must accept unknown optional members of `tree status`, the decisions and the
  run log. The story asks Runbook for that, and Loom does not change Runbook.

## ESS first

`ess/` changes first, in its own commit, then `task drift`:

- `bindings.yaml`: `BoundAction` gains `usage_from: Optional<String>`, and `ProgramRun` gains
  `cost_micro_usd: Optional<Integer>`.
- `runlog.yaml`: `ActionFinished` gains `cost_micro_usd: Optional<Integer>`.
- `tree.yaml`: `StatusRun` gains `cost_micro_usd: Optional<Integer>`.

## Acceptance

Bounds for every test below: no test over 60 s, the test step within 120 s, no network, no model
call (the programs are fixture scripts that print a fixed report).

- A binding with `usage_from: /usage/cost_micro_usd` and a report holding `1234` puts
  `cost_micro_usd: 1234` in the `ProgramRun`, the `ActionFinished` line and the status entry.
  Test: `a_reported_cost_reaches_the_run_the_log_and_the_status`.
- Without `usage_from`, the three hold no `cost_micro_usd`, and no 0 appears. Test:
  `without_usage_from_no_cost_is_written`.
- A reported `0` is written as 0. Test: `a_reported_zero_cost_is_written_as_zero`.
- An absent pointer, `-1`, `1.5` or `"12"` is `ReportUnreadable`, no record is submitted, and the
  run observation holds no cost. Test: `an_unreadable_cost_is_refused_and_records_nothing`.
- A log written before this change still verifies, and its `ActionFinished` lines read with no
  cost. Test: `an_old_log_without_a_cost_still_verifies`.
- `usage_from` on a `person` executor is refused at load, `MemberNotForExecutor`. Test:
  `usage_from_is_a_program_member`.

## Not covered

- A budget on cost: `story:tree-usage-and-model-budget`.
- Agent usage: `story:executor-reports-usage` and `story:run-and-action-budgets`, for
  `b10x-loom run`.
- Token counts from a program.
- Option (c).

## Scope

- `crates/loom-bindings/src/program.rs`: `:36-48`, `:447`, `:655-666`, `run_json` at `:863`. Cited.
- `crates/loom-bindings/src/decode.rs`: the `ACTION` members at `:22-34`. Cited.
- `crates/loom-tree/src/run.rs`: the `ActionFinished` writes. Inferred.
- `crates/loom-tree/src/status.rs`: `runs` at `:238-283`. Cited.
- `crates/loom-runlog/src/lib.rs`: the encoder, `event_members` and the decoder. Cited.
- `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml` and `ess/domains/tree.yaml`, at the lines
  above. Cited.

Edges:

- depends_on `story:bound-program-result-table`.
- Collides with `story:child-record-in-the-run-log` and `story:tree-status-names-what-it-read` on
  `runlog.yaml`, `crates/loom-runlog` and `tree.yaml`, and starts after them.
