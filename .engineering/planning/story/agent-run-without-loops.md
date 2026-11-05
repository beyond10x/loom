---
format: aep.planning-md/3
id: story:agent-run-without-loops
kind: story
status: implemented
title: agent run runs a spec that declares no loops
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-agent/src/lib.rs
- confidence: inferred
  path: crates/loom-agent/src/run.rs
- confidence: cited
  path: crates/loom-agent/src/serve.rs
- confidence: inferred
  path: crates/loom-agent/tests/check.rs
- confidence: inferred
  path: crates/loom-agent/tests/serve.rs
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/agent_run.rs
- confidence: inferred
  path: crates/loom-cli/tests/agent_serve.rs
- confidence: inferred
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: ess/domains/agent.yaml
- confidence: cited
  path: ess/scenarios/agent-check
- confidence: cited
  path: ess/scenarios/agent-run-once
- confidence: cited
  path: generated/rust/loom
- confidence: inferred
  path: website/docs/reference/cli.md
- confidence: inferred
  path: website/docs/reference/ess
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "proposed", to: "active", at: "2026-11-05T02:46:35Z", actor: "human:timo", revision: 4, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
- {from: "active", to: "implemented", at: "2026-11-05T04:37:33Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1},"asserted":{"test_result":1}}, executor: "agent:loom-wave-lead", correlation: "wave-2026-11-05-loom"}
---
## Outcome

`b10x-loom agent run` runs a spec that declares no `loops`, and a channel binding without `list`, `get` or `search`, since a run polls nothing.

## Why

Out-of-scope row of review-result:adv-w20261102w2-a-pass-1: `agent run` inherits the serve checks "nothing would poll" (`crates/loom-agent/src/serve.rs:837-839`), so a spec written only for scheduled runs is refused.

## Acceptance

- A spec with no `loops` given to `agent run --input` runs one turn and exits 0; `agent serve` still refuses it.
- `loom.agent.CheckAgentSpec` (`ess/domains/agent.yaml`) checks the states of a spec with no `loops` against `inbound-answer@1`, and `agent run` runs such a spec as that protocol's turn: it never sends, whatever its channel binding declares (no `list`, `get` or `search` needed, since a run polls nothing).
- A spec with states and no `loops` that `inbound-answer@1` does not admit is refused `UnknownProtocol` at `states`, as today.
