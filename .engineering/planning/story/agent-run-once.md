---
format: aep.planning-md/3
id: story:agent-run-once
kind: story
status: proposed
title: b10x-loom agent run runs one turn of a spec on a given task text and exits
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T12:40:55Z", actor: "human:timo", revision: 2}
---
## Outcome

`b10x-loom agent run --spec FILE --state DIR --input FILE` runs one turn of a `loom.agent/1` spec
on one item, the task text the caller gives, and exits: it records the turn in the state
directory as `agent serve` records a handled item, performs at most the sends `writes` admits,
prints one line per proposal, and ends with an exit status that says how the turn ended. It needs
neither a `channel` nor `expose.a2a`, so a scheduled `Command` resource (`story:command-schedule`)
can run an operator agent once per tick with the published `loom-agent` image and no wrapper.

## Why

A downstream platform's first scheduled operator-agent run is blocked (item D389, 2026-11-02):
`agent serve --once` without a channel exits `ONCE_WITHOUT_A_CHANNEL`
(`crates/loom-cli/src/main.rs:294`, `crates/loom-cli/src/a2a.rs:30`), and the downstream reports
that "no command in the image runs the spec's task prompt once and exits". The spec has no member
named a prompt today; this story takes the task text from the caller (`--input`), so the spec's
format does not change. Whether the downstream means a member of the spec by "task prompt" is
unclear and is asked before the wave opens.

## Acceptance

Named conformance scenarios (ESS first: the command, its input and its outcomes are declared in
`ess/domains/agent.yaml` before code):

- `agent-run-once/one-turn-recorded`: a spec with no `channel` and no `expose.a2a`, given an
  `--input` file, runs one turn against recorded model responses, appends one line to
  `record.jsonl`, prints one proposal line and exits 0.
- `agent-run-once/send-under-writes`: with `writes: {mode: auto, max_per_hour: 1}`, a turn that
  proposes a send performs it as `agent serve` performs one; a second run inside the hour is refused `RateLimited`, records it and exits 3.
- `agent-run-once/writes-zero-sends-nothing`: `writes.max_per_hour: 0` performs no send and
  records `RateLimited`.
- `agent-run-once/input-missing`: without `--input`, or with an unreadable or empty file, it
  exits 1 naming the file, before any model call.
- `agent-run-once/refused-spec`: a spec `agent check` refuses exits 3 with the same refusal and
  makes no model call.
- `agent-run-once/budgets-hold`: the turn stops at the spec's turn budgets as `agent serve`'s do.

`website/docs/reference/cli.md` is regenerated, and README and AGENTS.md name the command.

## Out of scope

The schedule itself (`story:command-schedule`); a spec member for a standing task text.
