---
format: aep.planning-md/3
id: story:program-reports-its-cost
kind: story
status: active
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
  path: crates/loom-cli/tests/tree_run.rs
- confidence: cited
  path: crates/loom-runlog
- confidence: cited
  path: crates/loom-tree/src/run.rs
- confidence: cited
  path: crates/loom-tree/src/status.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
- confidence: cited
  path: generated/ess/loom/
- confidence: cited
  path: generated/rust/loom/
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T06:37:59Z", actor: "agent:loom", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-22T07:13:51Z", actor: "agent:sdc", revision: 8, executor: "agent:loom", correlation: "runbook-model-run-order-2026-11-22"}
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
- A run that exits with an error or is killed at its timeout still carries the cost it printed,
  when its kept standard output is one JSON object within `MAX_REPORT_BYTES` and the pointer
  holds a whole number. The cost is read whatever the exit. If the output does not read, the cost
  is absent and the step ends as it does today: a failed or timed-out run gets no second refusal
  for its cost.
- No budget acts on it. `tree run` stops on `--max-steps` and `--max-wall-seconds` only, however
  large the reported cost.

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
call. The programs are fixture scripts in POSIX sh, not Python (§ No Python for permanent tools),
that print a fixed report. Every brief of the unit names that rule.

- A binding with `usage_from: /usage/cost_micro_usd` and a report holding `1234` puts
  `cost_micro_usd: 1234` in the `ProgramRun`, the `ActionFinished` line and the status entry.
  Test: `a_reported_cost_reaches_the_run_the_log_and_the_status`.
- Without `usage_from`, the three hold no `cost_micro_usd`, and no 0 appears. Test:
  `without_usage_from_no_cost_is_written`.
- A reported `0` is written as 0. Test: `a_reported_zero_cost_is_written_as_zero`.
- An absent pointer, `-1`, `1.5`, `"12"` or `9223372036854775808` (one above `i64::MAX`) is
  `ReportUnreadable`, no record is submitted, and the run observation holds no cost. Test:
  `an_unreadable_cost_is_refused_and_records_nothing`.
- A program that prints a report with cost `700` and then exits 1 leaves `cost_micro_usd: 700`
  in its `ActionFinished` line and its status entry. So does one that prints the report and is
  then killed at its timeout. Test: `a_failed_or_timed_out_run_keeps_its_reported_cost`.
- A reported cost of `9000000000000` (nine million USD) does not stop the run. The next step is
  taken, and only `--max-steps 2` ends the run, `BudgetSpent`, exit 5. Test:
  `a_large_reported_cost_stops_nothing`.
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
- `crates/loom-tree/src/run.rs`: the `ActionFinished` writes at `:281` and `:330`. Cited.
- `crates/loom-tree/src/status.rs`: `runs` at `:238-283`. Cited.
- `crates/loom-runlog/src/lib.rs`: the encoder, `event_members` and the decoder. Cited.
- `ess/domains/bindings.yaml`, `ess/domains/runlog.yaml` and `ess/domains/tree.yaml`, at the lines
  above. Cited.
- `generated/rust/loom/` and `generated/ess/loom/`, regenerated. Inferred.
- `crates/loom-cli/tests/tree_run.rs`, for the run and status tests. Cited.

Edges:

- depends_on `story:bound-program-result-table` and `story:handoff-when-a-program-decides`.
- Order on `runlog.yaml`, `crates/loom-runlog`, `tree.yaml` and the generated model files:
  `story:child-record-in-the-run-log`, `story:tree-status-names-what-it-read`,
  `story:handoff-when-a-program-decides`, then this story.
- Shares `crates/loom-tree/src/run.rs` and `tests/tree_run.rs` with those stories and with
  `story:every-printed-decision-command-runs`. It shares the `ACTION` list and `BoundAction` with
  `story:handoff-when-a-program-decides`. The later unit merges the earlier head.
