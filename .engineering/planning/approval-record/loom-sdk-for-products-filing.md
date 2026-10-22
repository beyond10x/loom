---
format: aep.planning-md/3
id: approval-record:loom-sdk-for-products-filing
kind: approval-record
status: draft
title: Filing epic loom-sdk-for-products and 14 draft stories; the orchestrator's answers to the SDK design's questions
relations:
- decides: epic:loom-sdk-for-products
revision: 1
---
# Filing epic:loom-sdk-for-products and its stories as drafts

Session `b10x-chef-orchestrator` decided, in these words (2026-10-22):

> "A design for the SDK is at ~/.cache/b10x-layers/design-2026-10-22/loom-sdk.md (Timo asked for
> it, 2026-10-22: "create ... the design for the agentsdk (loom-sdk)"). After your 0.34.0 wave,
> file epic:loom-sdk-for-products and its 15 stories as drafts in your store from §6, checking each
> claim against the code first. My decisions on its questions, record them as approval-record: Q1 A
> (AgentSpec + ToolPolicy in a Loom ESS domain loom.agent), Q3 A (split LoopContext, blanket impl),
> Q4 A (RuntimeEvent + EventSink), Q5 B (swarm moves onto the SDK only after Agency exists), Q6 A
> (git tags + a version rule), Q7 A (move the metaharness pin to its newest release; read its
> CHANGELOG 0.11-0.13 first and report a break). Q2 (writable root) waits on Timo's substrate ADR
> answer. Do not implement the epic yet; next wave proposal comes to me."

and later:

> "While 0.34.0 waits for the ess 0.68.0 and substrate tags: file epic:loom-sdk-for-products and
> its 15 stories as drafts from ~/.cache/b10x-layers/design-2026-10-22/loom-sdk.md, with my
> decisions Q1 A, Q3 A, Q4 A, Q5 B, Q6 A, Q7 A (decisions.md 08:1x). Q2 waits on Timo's substrate
> writable-root ADR. Then propose the first SDK wave to me. Record this in your store."

| Q | Decision | Where it lands |
|---|---|---|
| Q1 | A: `AgentSpec` and `ToolPolicy` in a Loom ESS domain `loom.agent` | `story:agent-domain` |
| Q2 | open, Timo's Substrate writable-root ADR | `decision-blocker:sdk-writable-host-root` |
| Q3 | A: split `LoopContext`, blanket impl | `story:loop-context-split` |
| Q4 | A: `RuntimeEvent` and `EventSink` | `story:runtime-events` |
| Q5 | B: swarm moves onto the SDK only after Agency exists | epic § Not in scope |
| Q6 | A: git tags and a version rule | `story:stability-version-rule` |
| Q7 | A: newest metaharness | done: metaharness `0.13.0` since wave 2026-10-22-w2 |

Decisions taken while filing:

- D1: 14 stories are filed, not 15. `host-stop-after-checks` shipped as
  `story:host-stop-when-a-step-follows` (wave 2026-10-22-w2), so it is not filed again.
- D2: `tool-decisions-read-back` depends on no new pin. metaharness 0.13.0 `trace-ir/1` carries the
  run's `DecisionCensus` (`metaharness-protocol/src/trace_ir.rs:180`) and `DecidedBy`
  (`event.rs:121`), read in the cargo checkout of tag 0.13.0.
- D3: the design's claim corrections (`commission.responsibility.Agent` exists at
  `ess/commission/domains/responsibility.yaml:864-870`; `AgentExecutor` at
  `crates/loom-commission/src/ports/executor.rs:20`; the stdin cap is 16 MiB with no default) are
  written into the stories they touch.
- D4: the work is not implemented here; no wave is opened by this record.
