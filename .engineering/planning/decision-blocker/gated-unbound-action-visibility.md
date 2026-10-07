---
format: aep.planning-md/3
id: decision-blocker:gated-unbound-action-visibility
kind: decision-blocker
status: cleared
title: Nobody has decided whether the unbound-action filter hides an action behind an authority gate
relations:
- blocks: story:effect-invocation
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-07T02:53:13Z", actor: "human:timo", revision: 3, executor: "agent:loom", correlation: "wave/2026-10-07-w2"}
---
## Question

The decision of 2026-10-07 on `decision-blocker:action-operation-binding` (option B) removes every
action the effect port does not perform from the frontier before an executor runs. Does that filter
also remove an action that needs authority, such as `repository.merge`, when no port performs it?

## Why it matters

The local slice's effect port performs only `repository.inspect`, `repository.edit` and `tests.run`
(`crates/loom-intake-slice/src/effect.rs:201`: `self.local && [INSPECT, EDIT, TESTS_RUN].contains(&action)`).
Today a `b10x-loom run` stops at the human gate, `stopped: ApprovalRequired (repository.merge)`, exit
status 0 (README § Run the command line; `website/docs/guides/run-an-intent.md`; the qualification
record of 2026-10-05). The authority check comes before the effect port is asked. With the filter as
decided, `repository.merge` is never offered, the run ends on another stop, and the exit status
becomes 3: a change to the command line's documented contract.

## Options

- A: filter every action no port performs, gated or not. The slice's run no longer stops at the
  merge approval; README, the run-an-intent guide and the exit-status contract change.
- B: filter only actions that no port performs **and** that need no authority. An action behind an
  authority gate stays visible, so the run stops at the gate as today; an approved action no port
  performs still ends `NoPerformableAction` at invocation, as today.
- C: the filter is per port: `EffectPort` gains a method saying which actions it offers (default:
  every action, as today), and only the Connector-backed port answers from its bindings. The local
  slice keeps today's behaviour; a Connector-backed composition hides unbound actions.

## What it stops

`story:effect-invocation`, item 2 of its Outcome (unbound actions are never offered).

## Decision (2026-10-07)

Option B. The filter removes only actions that no effect port performs **and** that need no
authority. An action behind an authority gate, such as `repository.merge` on the local slice, stays
in the frontier an executor sees, so a run stops at the gate as it does today
(`stopped: ApprovalRequired (repository.merge)`, exit status 0). An approved action that no port
performs ends the run `NoPerformableAction` at invocation, as today. `story:effect-invocation`
carries both in its acceptance; the decision on `decision-blocker:action-operation-binding` is
narrowed accordingly.
