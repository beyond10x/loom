---
format: aep.planning-md/3
id: decision-blocker:regression-test-selection
kind: decision-blocker
status: cleared
title: Nobody has decided how a bug-fix run shows its test command ran the written regression test
relations:
- blocks: story:regression-evidence-integrity
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-14T10:17:40Z", actor: "human:timo", revision: 3}
---
## Question

How does a bug-fix run show that its test command ran the regression test its `tests.write`
wrote, so that only that result is submitted as `regression_test_result`?

Today any failing suite after a committing `tests.write` counts
(`crates/loom-intake-slice/src/verifier.rs`, story:regression-evidence-integrity).

## Options

- A. The host's test command names its selection: `run --regression-test-command <cmd>` with a
  `{path}` placeholder the slice fills with each path the run's `tests.write` committed; only that
  command's result is a `regression_test_result`. Cost: a new flag and `SliceRun` field; a host
  that does not give it gets no regression result, so `red_first.required` stays open for it.
- B. The slice runs each written file on its own through a per-language runner it infers. Cost:
  Loom guesses how to run a test, which the host owns today.
- C. Keep the suite result, and require that the written file's path appears in the runner's
  output. Cost: depends on the runner's output format; a false match is possible.

Recommendation: A.

## Stops

The first acceptance line of story:regression-evidence-integrity. Its second line (the governor
refuses a record whose `observed_at` differs from its cited observation) needs no answer.

## Decision (2026-10-14)

A. The host names its regression-test command: `run --regression-test-command <cmd>`, with a `{path}` placeholder the slice fills with each path the run's `tests.write` committed. Only that command's result is submitted as `regression_test_result`; a host that gives no such command gets no regression result, and `red_first.required` stays open for it. The flag and its field are optional, so a run without them behaves as before apart from that.
