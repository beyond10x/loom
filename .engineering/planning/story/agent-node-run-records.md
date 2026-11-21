---
format: aep.planning-md/3
id: story:agent-node-run-records
kind: story
status: proposed
title: The run log records per model call adapter, model, tokens and duration, and per agent step allowed tools and tool calls
summary: From Loom's observation of the session, absent where the adapter reports nothing
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bound-agent-node
- depends_on: story:hash-chained-run-log
- depends_on: story:tree-run-command
scope:
- confidence: inferred
  path: crates/loom-bindings/src
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-runlog/src
- confidence: inferred
  path: crates/loom-session-executor/src/lib.rs
- confidence: inferred
  path: crates/loom-session-executor/tests/model_calls.rs
- confidence: inferred
  path: crates/loom-tree/src
- confidence: cited
  path: crates/loom-tree/tests/run_log_agent.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/session.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:46Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
---
## Outcome

A case's run log says what each agent node did, from Loom's own observation and never from the
session's text. For each agent step: the adapter, the model, the tools it was allowed, every tool
call the session made (tool name, the decision, who decided), the turns, the duration and the
session's transcript digest. For each model call the adapter reports: the adapter, the model, the
input, output and cached tokens, and the duration in milliseconds. A figure the adapter did not
report is absent, never 0. The `action started` line of an agent step is marked as one, and
`tree run --max-agent-calls N` counts those lines over the logs of the whole tree, across runs,
and stops with exit 5 before the agent step that would pass N.

## Why

The SDC's live view reads `adapter`, `model`, `input_tokens`, `output_tokens`, `usage`,
`tool_calls` and `allowed_tools` from run events (`~/beyond10x/sdc/ui/server.mjs:130`), which
`sdc-drive` gets from a side file its shell script writes
(`~/beyond10x/sdc/docs/driver.md` § "LLM executor", step 5), and `sdc-drive run
--max-llm-calls` caps agent calls over the tree from the logs
(`~/beyond10x/sdc/driver/src/main.rs:556-560`, `676-684`). Loom has most of the figures and
writes none of them per step: `SessionReport` holds model, tokens, duration, turns and
`tool_decisions` (`crates/loom-session-executor/src/lib.rs:539-581`); the persisted
`AgentSession` row holds no tokens and no model (`ess/domains/session.yaml:129-167`); the allowed
tools are an input that is not recorded (`ToolPolicy`, `generated/rust/loom/src/agent.rs:533-541`);
`Usage` lines of `run-events-v1` carry model and tokens with no duration and no adapter
(`crates/loom-cli/src/events.rs:520-545`). The draft `story:tool-decisions-on-step` puts the
tool decisions on Commission's `ExecutorStep`; this story writes them, with the rest, into the
tree's run log and does not depend on it. Session `sdc` adapts its view to the run log's file and
contract (`runlog-<stem>.jsonl`, `docs/contracts/run-log-v1.json`).

Per-call figures do not exist today (read 2026-11-21): `SessionReport` holds session totals
only; the session executor's event loop sees a per-call usage event, keeps it only for
b10x-harness and then sums it away (`crates/loom-session-executor/src/lib.rs:1439, 1525, 1594,
1865-1876`); no event carries a call's duration. So this story changes the session executor:
`SessionReport` gains the list of calls the harness reported (request id, model, tokens), each
with a duration only where the harness's events carry the instants to compute one. The
`AgentRun` of `story:bound-agent-node` carries the report's figures to the tree.

## ESS first

`ess/domains/session.yaml` first: `ModelCallUsage` and the member on the session report.
`ess/domains/bindings.yaml`: `AgentRun` gains turns, duration, transcript digest and the calls.
`ess/domains/runlog.yaml`: `RunLogEvent` gains `AgentStep` (adapter, model, allowed tools,
tool calls, turns, duration, transcript digest) and `ModelCall` (adapter, model, tokens,
duration), `ToolCallRecord` reuses `loom.session.ToolDecision`, and the action-started event
gains `agent`. Then `ess/domains/tree.yaml`: `TreeBudget` gains `max_agent_calls`. On that commit
`task drift` fails, and `crates/loom-tree/tests/run_log_agent.rs` does not compile.

## Acceptance

With the recorded adapter of `story:bound-agent-node`, no model call:

- A recorded session of 3 model calls and 4 tool calls, one of them refused, gives 3 `ModelCall`
  lines and one `AgentStep` line with 4 tool calls in order, the refused one marked refused with
  who decided it.
- The `AgentStep` line's turns, duration and transcript digest equal the recorded session's.
- A recorded session whose events carry an instant per call gives each `ModelCall` line the
  duration between its instants; one whose events carry none gives lines with no duration.
- `allowed tools` equals the binding's `tools.allow`, also when the session called none.
- A session whose adapter reports no token counts gives lines with the token members absent.
- The `AgentStep` line carries the session's total tokens as the harness reported them; no test
  requires the `ModelCall` lines to sum to it (Claude Code reports per message beside a total).
- `SessionReport`'s new member is empty for a harness that reports no per-call usage, and the
  existing tests of `crates/loom-session-executor` pass unchanged.
- With a canary string in the recorded prompt, in a tool argument and in a tool result, no log
  line contains the canary.
- In a fixture tree with three agent steps over two cases, `tree run --max-agent-calls 2` exits
  5 before the third and names its case; a second `tree run --max-agent-calls 2` on the same
  store takes no agent step; `--max-agent-calls 3` completes.
- `tree verify` exits 0 after the run.
- `docs/contracts/run-log-v1.json` is regenerated and names both events.
- `task check` exits 0.

## Out of scope

Cost in currency. A per-run total report. Tool decisions on Commission's `ExecutorStep`
(`story:tool-decisions-on-step`). The SDC view itself and its `llm-calls.jsonl` layout.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree` (the run that writes the log lines and counts `action started` lines for `--max-agent-calls`) with the ESS domain `ess/domains/runlog.yaml` — cited, story ESS first and `crates/loom-tree/tests/run_log_agent.rs`; neither exists yet (siblings `story:hash-chained-run-log`, `story:tree-run-command`)
- **Files:** `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `crates/loom-tree/tests/run_log_agent.rs` — cited, named in the story's ESS first section
- **Files:** `crates/loom-tree/src`, `crates/loom-runlog/src`, `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/main.rs` — inferred, the story names no source file; the event writer is the tree run, the log crate "declares the events", the `tree run` flag sits where `RunArgs` and `--max-steps` do today (`lib.rs:766-791`, `main.rs:1074`)
- **Files:** `crates/loom-session-executor/src/lib.rs` — inferred, per-call tokens and durations are not in `SessionReport` (see finding), so keeping them means a change at the event loop (`lib.rs:1439`, `1525`, `1594`) and a new `SessionReport` member
- **Files:** `crates/loom-session-executor/tests/model_calls.rs` — inferred, a new test file for per-call figures beside `event_observer.rs` and `tool_decisions.rs`; the name is a guess
- **Symbols:** `RunLogEvent`, `AgentStep`, `ModelCall`, `ToolCallRecord`, `TreeBudget::max_agent_calls` — cited, the story's ESS first section; none exist yet
- **Symbols:** `SessionReport`, `SessionReport::tool_decisions`, `SessionTrace::transcript_digest`, `loom.session.ToolDecision` — cited, the story's Why (`lib.rs:539-581`, `ess/domains/session.yaml:75`)
- **Also likely:** `crates/loom-bindings/src` and `ess/domains/bindings.yaml` — inferred; `AgentRun` of `story:bound-agent-node` holds adapter, model, allowed tools and decisions but not turns, duration, digest or per-call tokens, and nothing in this story names how the session's report gets from the agent executor to `loom-tree`
- **Documents:** none hand-written — cited; the one named artifact, `docs/contracts/run-log-v1.json`, is generated (see House-rule files)
- **Confidence:** medium — the ESS, the test file and the data source are named, but the crates they land in do not exist yet, and the story does not say where per-call figures come from or how the report crosses from the bindings crate to the tree
- **Would collide with:** any unit touching `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `crates/loom-tree`, `crates/loom-runlog`, the `tree` flags of `crates/loom-cli`, `crates/loom-session-executor/src/lib.rs` (its drafted `story:tool-decisions-on-step` names that crate too), and `ess/domains/bindings.yaml` / `crates/loom-bindings` if the `AgentRun` change is needed
- **Safety fact:** `SessionReport` is built and read only inside `crates/loom-session-executor` (`git grep` shows struct literals at `lib.rs:1736`, `4168`, `4173`, and test readers via `last_report`) and derives `Default`, so a new member changes no caller outside that crate — step 2 (`git grep -n 'SessionReport'`), unproven
