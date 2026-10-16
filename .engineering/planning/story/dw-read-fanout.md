---
format: aep.planning-md/3
id: story:dw-read-fanout
kind: story
status: implemented
title: Admitted reads proposed together run concurrently, each invoked once through its binding
relations:
- decomposes: epic:deterministic-workflows
- serves: vision:governed-autonomy
- informed_by: architecture-decision-record:read-fan-out
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T23:50:18Z", actor: "human:timo", revision: 2, correlation: "wave-2026-10-15-w1"}
- {from: "proposed", to: "active", at: "2026-10-14T23:50:18Z", actor: "human:timo", revision: 3, correlation: "wave-2026-10-15-w1"}
- {from: "active", to: "implemented", at: "2026-10-16T01:33:09Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

An executor can propose several reads at once, and Commission's runtime revalidates each against
the same frontier, starts every admitted read concurrently, joins them, and records each effect
once, through its binding once. Writes, and any action that needs authority, stay one at a time.

## Why

Troubleshooting collects its data from several sources, and today a run performs one action per
iteration (`crates/loom-commission/src/runtime.rs`, module docs items 6 and 8: one
`ProposedAction`, one request, one `EffectPort::invoke`), so the reads of one step are serialised
across as many iterations, each paying a governor round and, under a model executor, a model call.

## Design

- `ExecutorOutcome` gains `ProposedReads`: a non-empty list of `(action, arguments)`, at most
  `MAX_FAN_OUT` (16) long.
- `EffectPort` gains `reads(&self, action) -> bool`, default `false`. `ConnectorsInvoker` answers
  `true` exactly for an action whose binding's effect is `Read`; no other port in the repository
  answers `true` until it opts in.
- The runtime admits the fan-out only when every listed action is one the port `reads` and the
  handed frontier lists `Admissible` (no authority). Otherwise the whole proposal is refused, no
  read is invoked, and the iteration is idle. Each read becomes its own `ActionRequest` at the
  frontier's revision and is revalidated before any is invoked; a request that fails revalidation
  is recorded refused and not invoked, the others go on.
- The admitted reads are invoked concurrently (scoped threads; the fan-out path requires a `Sync`
  port), each once, and joined. Their `EffectOutcome`s are recorded in `LoopEnd::effects` and
  observed in the order proposed, not the order they finished. A `Refused` read does not cancel
  the others; an `Err` from any read fails the loop after all have joined, with nothing retried.
- A fan-out is one step for the step budget, the idle bound and the run meter.
- `WorkflowExecutor` gains a `Concurrent` step kind listing reads, proposed as `ProposedReads`, in
  `story:dw-incident-workflow`; this story is shown with a scripted executor.

## Domain relations

- Fan-out -> action requests: one fan-out makes one `ActionRequest` per listed read, each owned by
  the Run — inferable (`ess/commission/domains/responsibility.yaml`, `commission.responsibility.Run`
  relation `requests`, `owns`, cardinality many).
- Read -> binding: an action is read-only when its commission's binding declares `effect: Read` —
  inferable (`ess/commission/domains/responsibility.yaml`, `commission.responsibility.ActionBinding`
  field `effect`, `commission.responsibility.ConnectorOperationEffect`; one binding per action,
  `commission.responsibility.ActionBindingKey`).

## Acceptance

Three admitted reads proposed together are invoked concurrently, each once through its binding,
and recorded in proposal order; a fan-out holding a write invokes nothing.

- `crates/loom-commission-testkit/tests/read_fan_out.rs`
  `admitted_reads_run_concurrently_and_join`: three reads each blocking on a shared barrier of three
  complete (they would deadlock if run one at a time), within one step.
- `each_read_is_invoked_once_through_its_binding`: against `ConnectorsInvoker` and the fake service
  of `crates/loom-connectors/tests/connectors_invoker.rs`, three `POST /v1alpha2/invoke` calls, each
  `Performed` naming its own audit record.
- `effects_are_recorded_in_proposal_order`.
- `a_fan_out_with_a_write_or_an_approval_required_read_invokes_nothing`.
- `a_stale_read_is_refused_and_the_others_are_performed`.
- `an_effect_error_fails_the_loop_after_every_read_joined`.
- `task commission:conform` passes with the new variant's synthesized scenarios.

## ESS first

The first commit changes only `ess/commission/`: `commission.responsibility.ExecutorOutcome` gains
`ProposedReads: commission.responsibility.ExecutorOutcomeProposedReads` (`reads:
List<commission.responsibility.ExecutorOutcomeProposedAction>`), with its comment stating the read
and admission rule. On it `task commission:drift` fails; later commits regenerate
`generated/rust/commission/` and make `admitted_reads_run_concurrently_and_join` pass without
changing `ess/`.

## Scope

- `ess/commission/domains/responsibility.yaml`
- `generated/rust/commission/` (regenerated)
- `crates/loom-commission/src/runtime.rs`, `crates/loom-commission/src/ports/effect.rs`,
  `crates/loom-commission/src/outcome.rs`
- `crates/loom-commission-testkit/tests/read_fan_out.rs` (new)
- `crates/loom-connectors/src/lib.rs` (`reads`)
- `docs/commission/` (runtime loop contract), `website/docs/reference/commission/` (regenerated by
  `task commission:docs`)
- `AGENTS.md` (§ Checks that hold the rules), `CHANGELOG.md`, `website/data/status.json`

Does not touch `ess/domains/` or `generated/rust/loom`. Touches none of
`crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`, `ess/intake/`,
`generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`, `Cargo.toml`,
`Cargo.lock`. The new variant must compile unchanged in `crates/loom-intake-slice/src/run.rs`,
`crates/loom-plugin/src/turn.rs` and `crates/loom-executor/src/harness/governed.rs`, which match
`ExecutorOutcome` with a wildcard arm; if one does not, the story touches it and says so.

Ready now; independent of the workflow stories.

## Not in scope

Concurrent writes; fan-out of actions that need authority; a model executor proposing several
reads (the governed loop still proposes one); which incident sources are read
(`story:dw-incident-workflow`).
