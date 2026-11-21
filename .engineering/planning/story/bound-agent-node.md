---
format: aep.planning-md/3
id: story:bound-agent-node
kind: story
status: proposed
title: An action bound to an agent runs one bounded session whose only effect is a new artifact revision
summary: Adapter and model as configuration, Haiku by default, adapter registered by the host, Claude Code through metaharness first
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
- depends_on: story:bindings-document
- depends_on: story:bound-artifact-revisions
scope:
- confidence: inferred
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-bindings/Cargo.toml
- confidence: inferred
  path: crates/loom-bindings/src
- confidence: cited
  path: crates/loom-bindings/tests/bound_agent.rs
- confidence: cited
  path: ess/domains/bindings.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:45Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
---
## Outcome

An action bound to an `agent` executor runs one bounded agent session and its only effect is a
new content digest of the artifacts the bindings place. The binding gives the adapter name and
the model as configuration (the decoded default is `claude-haiku-5-5`), the `inputs` and
`outputs` paths, the tool allow-list, the budget (`max_turns`, `max_tokens`,
`max_budget_micro_usd`, `max_runs`) and the timeout. The host registers adapters by name on the
engine; Loom ships one, `claude-code`, which runs the host's Claude Code through metaharness by
the existing session executor, and an adapter for a gateway, plain inference or Bedrock is a
later registration with no change to a bindings file. The session works in a scratch copy that
holds only the binding's `inputs`, is offered only the allowed tools, and Loom copies back only
the binding's `outputs`. Nothing the session says is evidence: a record about what it wrote comes
only from a bound program that observes the new revision, or from Loom's own report where the
entry says `from: effect`.

A writer is sent back to work by its binding's `retry_when`. `b10x-loom-bindings` computes the
match in one function, `retry_records`: the records held about the current revision of their
subject whose kind and result the binding's `retry_when` lists, none while that revision is
`none`. On a retry the scratch copy also holds those records and the last 60 lines of each
failing program's blob as one input file.

## Why

`~/beyond10x/sdc/docs/loom-sdk-and-agent-node.md`, verdict 4: nothing in Loom declares "this
action runs on an agent, under this budget and sandbox". A run has one executor
(`crates/loom-sdk/src/runtime.rs:30-37`). The SDC binds an agent node as a shell script
(`tools/llm-node.sh`, `~/beyond10x/sdc/docs/driver.md` § "LLM executor") that copies inputs,
calls an adapter and copies the output back: runtime work in application data. Loom already has
the session executor that proposes files and never writes them
(`crates/loom-session-executor/src/lib.rs:1375-1414`, `Restricted` adds the offered tools), and
its rules already say a trace is not evidence (`AGENTS.md` § Rules) and that Loom holds no
provider wire and no agent loop (`AGENTS.md` § Boundary). Session `sdc`, 2026-11-21: an agent
node writes a proposal file as its output and a following `program` action applies it, so a
program makes every store change; its first agent run overwrote a person's edit, which copying
back only the declared outputs prevents.

## ESS first

`ess/domains/bindings.yaml` first: `AgentRun` (adapter, model, allowed tools, tool decisions,
copied outputs), `AgentBudget`, and the refusals `AdapterUnknown`, `ToolNotAllowed` and
`AgentBudgetSpent`. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bound_agent.rs` does not compile.

## Acceptance

Every test uses a recorded adapter that replays a fixed session; none calls a model
(`AGENTS.md` § Never).

- A binding that names an adapter the engine has not registered is refused `AdapterUnknown`
  before any session starts, naming the action and the adapter.
- The adapter is called with the decoded model and with exactly the binding's `tools.allow`.
- A tool call outside the allow-list in the recorded session is refused `ToolNotAllowed`, and
  the step's observation payload lists that call with decision `refused`.
- `crates/loom-session-executor` is used as it is: `crates/loom-bindings` stages the `inputs`
  (and the feedback file) in a directory of its own, hands that directory to the session executor
  as its workspace, takes only the `outputs` from the proposal and checks them against the real
  workspace itself. `git diff --stat` of the story names no file under
  `crates/loom-session-executor`.
- The scratch copy holds the binding's `inputs` and no other workspace file. A file the session
  wrote outside the binding's `outputs` is not in the workspace afterwards; an output is, and the
  artifact's revision moves (`story:bound-artifact-revisions`).
- A workspace file that is not among the outputs and was edited between two agent steps has the
  same bytes after the second step.
- An agent action with no `evidence` entry records no evidence; one with `{kind, from: effect,
  results: {performed: R}}` records `R` with Loom's effect as the observation's source, and a
  result named in the session's text is ignored.
- A session stopped at `max_turns`, `max_budget_micro_usd` or the timeout, and one whose report
  shows more tokens than `max_tokens` after it ended (the session executor has no token input to
  stop it earlier), is refused `AgentBudgetSpent` naming the limit, and what it wrote is not
  copied back.
- A recorded tool decision `Deny` is the decision `refused` of the step's observation.
- `retry_records` returns a held `invalid` record about the current revision when `retry_when`
  lists its kind and `invalid`; it returns nothing for a record about an earlier revision, a
  listed kind with another result, or a subject at revision `none`.
- On a step with a non-empty `retry_records`, the scratch copy holds one feedback file naming
  each record's kind and result and at most 60 lines of each blob.
- The shipped `claude-code` adapter starts Claude Code only through the session executor; a test
  asserts the metaharness argv it would run, without running it.
- `task check` exits 0.

## Out of scope

Adapters for a gateway, plain inference or Bedrock. A shell tool narrowed per call. Run records
of the session in the run log (`story:agent-node-run-records`). Reviews by an agent as an
independent producer. Confining the session by Substrate: the session executor's harness spawn
stays under metaharness's tool policy.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings`, the new crate that `story:bindings-document` creates — cited (story Outcome names `b10x-loom-bindings` and `retry_records`; ESS first names `crates/loom-bindings/tests/bound_agent.rs`)
- **Files:** `crates/loom-bindings/tests/bound_agent.rs` — cited (story § ESS first)
- **Files:** `ess/domains/bindings.yaml`: `AgentRun`, `AgentBudget`, `AdapterUnknown`, `ToolNotAllowed`, `AgentBudgetSpent` — cited (story § ESS first)
- **Symbols:** `retry_records`, the adapter registry on the engine, the `claude-code` adapter — cited (story Outcome); the source files that hold them under `crates/loom-bindings/src` are not named anywhere — inferred
- **Also likely:** `crates/loom-bindings/Cargo.toml` gains `b10x-loom-session-executor` by path, and probably `b10x-loom-commission`, because `AgentExecutor::run_with_usage` takes a `Commission<Assigned>` and a `Frontier<Issued>` (`crates/loom-session-executor/src/lib.rs:1724-1728`) — inferred
- **Also likely:** the recorded-adapter fixtures sit under `crates/loom-bindings/tests/`, which the crate path already covers — inferred
- **Not changed:** `crates/loom-session-executor`. Its public fields cover the model, tools, turns, timeout and input files (`lib.rs:601-659`); the budget goes through `overrides.max_budget_micro_usd` (`lib.rs:357-373`); `SessionExecutor::new` builds it with no `loom.agent/1` document (`lib.rs:715`); `with_runner` takes a recording runner for the argv test (`lib.rs:1123`); `pub use metaharness` (`lib.rs:122`) means `loom-bindings` needs no direct metaharness dependency — cited for the API, inferred that it is enough
- **Documents:** none cited. A guide page under `website/docs/` extending `story:bindings-document`'s page is possible but not named — inferred
- **Confidence:** medium — the test file and the ESS file are cited, but the crate does not exist yet, so source file names are guesses, and "the executor needs no change" depends on a design choice the story does not state
- **Would collide with:** any unit touching `crates/loom-bindings` or `ess/domains/bindings.yaml` (and therefore the generated `loom.bindings` model). The executor stays unchanged only with staging, so a unit changing `ScratchCopy`, `changes`/`stale` or the action-line reply in `crates/loom-session-executor/src/lib.rs` would undercut this story without a file clash — inferred
- **Safety fact:** pointed at the code (level 2), not run. Copy-back can be limited to the binding's `outputs` without changing the executor only if `loom-bindings` stages the `inputs` (plus the retry feedback file) in a directory of its own, gives that directory to `SessionExecutor.workspace`, takes only the `outputs` from the `Proposal`, and does its own stale check against the real workspace. Pointed at the real workspace, the executor breaks three acceptance lines:
  (a) `ScratchCopy::make` copies the workspace's git state into the copy, so the copy holds more than the inputs (`lib.rs:3543-3546`, `carry_git`).
  (b) `stale` drops the whole proposal, outputs included, when the session adds a file outside `outputs` at a path the workspace already has (`lib.rs:1694-1701`, `3656-3683`).
  (c) The copy holds only workspace files, so the feedback file has nowhere to come from (`lib.rs:3374-3380`).
  Two traps hold even with staging:
  (d) A non-UTF-8 file anywhere in the copy fails the call (`lib.rs:3629-3631`).
  (e) A session that does not end with a JSON line naming an action is suspended (`lib.rs:1689-1693`, `2809`).
  Tool names containing a comma or whitespace are refused only in `from_spec` (`lib.rs:781-791`); through `new` they reach `--tools` joined with commas (`lib.rs:2412`). So `loom-bindings` or the decoder of `story:bindings-document` must refuse them — unproven.
