---
format: aep.planning-md/3
id: epic:downstream-adoption
kind: epic
status: implemented
title: A downstream factory runs its agent phases on Loom
relations:
- serves: vision:O3
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T07:47:45Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-08T16:22:11Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-08T16:22:11Z", actor: "human:timo", revision: 5}
---
## Outcome

A downstream software factory can run its agent phases on Loom as the native executor, driven by a
process supervisor (metaharness or another), with no gap it has to patch around. Source:
https://github.com/beyond10x/loom/issues/9 (read at `08a893e`, 2026-10-05).

## Gaps, as of `b460496` (2026-10-08)

1. `b10x-loom run` prints human lines only. A driver needs a versioned JSONL event stream on stdout
   (selected by a flag): turns, tool calls, approvals, usage and a documented terminal record.
2. Loom takes a `BearerSource` from its embedder (`crates/loom-executor/src/harness/wire/bearer.rs`).
   No adapter over llm's `llm-credentials` exists, so a run cannot use an OAuth subscription login.
   Ask: an adapter selectable by reference from the run's configuration, with no secret value in
   the configuration.
3. Effect binding: `story:effect-invocation` and `story:connector-action-binding` are implemented
   (`ConnectorEffects` over the `ConnectorInvoker` port). No Connectors client fills
   `ConnectorInvoker` yet, so a coding phase cannot edit files or run tests through Connectors.
   `decision-blocker:connector-substrate-containment` is cleared.
4. A caller that keeps its own case record must embed Commission's types and implement `CaseStore`
   to ask Canon for a decision (`crates/loom-governor/src/lib.rs`). Ask: a narrow evaluation entry
   point (library function or `b10x-loom` subcommand) taking protocol, case snapshot and evidence
   and returning Canon's decision, without Commission types.

## Done when

Each gap has an implemented story, or a recorded decision that it is out of Loom's scope with the
owning repository named, and issue 9 is closed by the pull request that lands the last one.
