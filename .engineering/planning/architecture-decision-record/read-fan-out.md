---
format: aep.planning-md/3
id: architecture-decision-record:read-fan-out
kind: architecture-decision-record
status: accepted
title: Commission's runtime performs admitted reads of one proposal concurrently
revision: 2
transitions:
- {from: "proposed", to: "accepted", at: "2026-10-14T12:20:50Z", actor: "human:timo", revision: 2}
---
## Decision

Commission's runtime may perform several reads proposed in one executor call concurrently. An
executor proposes them as `ExecutorOutcome::ProposedReads`, a non-empty list of at most 16
`(action, arguments)`. The runtime admits the fan-out only when the effect port reports every listed
action as a read (`EffectPort::reads`, default `false`; `ConnectorsInvoker` answers `true` exactly
for a binding whose effect is `Read`) and the handed frontier lists each as `Admissible`, needing
no authority. Otherwise the whole proposal is refused and nothing is invoked.

Each read becomes its own `ActionRequest` at the frontier's revision and is revalidated before any
is invoked; one that fails revalidation is recorded refused and not invoked, and the others go on.
The admitted reads are invoked concurrently, each once, through its binding, and joined. Their
outcomes are recorded and observed in the order proposed. A `Refused` read cancels nothing; an
`Err` fails the loop after every read has joined; nothing is retried. A fan-out is one step for the
step budget, the idle bound and the run meter.

Writes, and any action that needs authority, stay one per iteration.

## Context

Today one runtime iteration takes one `ProposedAction`, makes one `ActionRequest` and calls
`EffectPort::invoke` once (`crates/loom-commission/src/runtime.rs`, module docs items 6 and 8).
A troubleshooting workflow reads several sources it knows in advance; serialised, each read costs a
governor round and, under a model executor, a model call (`epic:deterministic-workflows`).

## Consequences

- `ExecutorOutcome` gains a variant: a host that matches it exhaustively breaks, and the release
  notes name it under `Breaking:`.
- `EffectPort` gains a defaulted method; existing ports compile unchanged and never fan out.
- The fan-out path requires a `Sync` effect port.
- Authority is not widened: no action that needs a grant is ever performed in a fan-out, and each
  read is revalidated as a single proposal is.

## Alternatives

- No fan-out: sources are read one per iteration, with no model call between them in a workflow.
  Rejected: latency grows with the number of sources, and the epic asks for concurrent collection.
- A binding per action and source (`ActionBindingKey` gains the source). Rejected with
  `decision-blocker:dw-incident-source-actions`.

## Source

story:dw-read-fanout (its Design section); decision-blocker:dw-read-fan-out-decision.
