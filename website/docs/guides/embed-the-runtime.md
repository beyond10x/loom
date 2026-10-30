---
title: Embed the runtime
sidebar_position: 2
description: Depend on b10x-loom-sdk, supply the selector, argument generator, authority provider and loop context, and run a governed case until it is blocked.
lede: An application embeds a governed agent through one crate, b10x-loom-sdk. It supplies the parts that decide and the parts that act; Loom, Commission's runtime and the governor do the rest.
source: crates/loom-sdk/src/lib.rs, crates/loom-sdk/examples/software_change.rs, crates/loom-sdk/tests/example_runs.rs, crates/loom-mandate/src/lib.rs, crates/loom-executor/src/metering.rs
---

# Embed the runtime

[`crates/loom-sdk/examples/software_change.rs`](https://github.com/beyond10x/loom/blob/main/crates/loom-sdk/examples/software_change.rs)
is a whole embedding in one file, and `crates/loom-sdk/tests/example_runs.rs` runs it in CI. This
page walks through it. [Getting started](../getting-started.md) shows its output.

CI also builds that file as an external crate would. `consumers/pinned/` in the repository is its
own Cargo workspace, depends on `b10x-loom-sdk` alone (by path, so it checks the tree before it is
tagged), includes the example by `#[path]` and runs it to `ApprovalRequired (repository.merge)`.
`task consumer-check`, a step of `task check`, builds it over the workspace's `Cargo.lock` and
fails when it no longer compiles, stops anywhere else, or resolves a second copy of a Loom or
Canon crate. Its library, `consumers/pinned/src/agency.rs`, is a second consumer: an agent
defined as data, a `loom.agent/1` document, run through the SDK alone until the budget and stop
policy the document states end it. Loom owns that schema; an Agency agent definition holds such a
document by value (Atlas ADR 0108). A change that breaks what this page tells you to compile against fails Loom's CI
before a release, not your build after one.

## Depend on the SDK

Nothing is on a registry. Depend on the release tag:

```toml
[dependencies]
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.51.0" }
```

Rust code uses it as `loom_sdk`. It re-exports, under stable module names:

| Module | Package | What it is for |
|---|---|---|
| `loom_sdk::commission` | `b10x-loom-commission` | The generated responsibility model, the ports, and the runtime loop |
| `loom_sdk::connectors` | `b10x-loom-connectors` | `ConnectorsInvoker`, Commission's `ConnectorInvoker` over a Connectors service |
| `loom_sdk::loom` | `b10x-loom-executor` | The executor and the `ActionSelector` and `ArgumentGenerator` traits |
| `loom_sdk::governor` | `b10x-loom-governor` | `CanonGovernor` over a `CaseStore` |
| `loom_sdk::intake::router`, `loom_sdk::intake::slice` | `b10x-loom-intake-router`, `b10x-loom-intake-slice` | Routing an intent; the slice's case opening, local executor, verifier and local effect adapter |

`run_until_blocked`, `LoopContext`, `LoopEnd`, `EffectPort`, `Loom`, `ActionSelector`,
`ArgumentGenerator`, `CanonGovernor`, `CaseStore` and `MemoryCaseStore` are also at the top.

[SDK stability](../reference/sdk-stability.md) names the surface Loom keeps stable within a
release line: every port a host implements, with its `loom_sdk` path, and what is outside the
promise.

Since 0.2.0 it also exports `TestRunner`, `SubstrateRunner`,
`UnconfinedRunner`, `TestExecution` and `ConfinementError`. `LocalExecutor::new` requires
Substrate by default; `with_runner` accepts an explicit `Arc<dyn TestRunner>`.
`SliceRequest.runner` selects the runner for the higher-level slice call. A runner is a trusted
host port: its results become observations from which the verifier can submit evidence.

An embedded host supplies an already delegated cgroup root to `SubstrateRunner::new`, or allows
discovery of the current delegated root. Automatic systemd re-exec belongs to the CLI.
The scripted SDK example explicitly uses `UnconfinedRunner` for its fixed local fixture;
its output and test observations say `confinement: none`.

## What you supply, and what you get

| You supply | Trait or type | In the example |
|---|---|---|
| A selector that picks one action from the catalogue | `ActionSelector` | `ScriptedSelector`: the next action of a fixed script |
| An argument generator for the selected action only | `ArgumentGenerator` | `ScriptedArguments`: the script's JSON for that action |
| An authority provider | `commission::ports::authority::AuthorityProvider` | `NoGrant`: every capability needs approval |
| Ids, the time and the step budget, which no model may supply | `LoopContext` | `Clock`: a fixed time, a counter for ids, 10 steps |
| Something that performs admitted actions | `EffectPort` | `LocalEffects`, the slice's adapter over a git work tree |
| The commission: who the run works for, on which case | `Commission` | an operator with no authority context |

An embedding that decides authority with Mandate can supply `MandateAuthority` from
`b10x-loom-mandate`, a separate opt-in crate the SDK does not re-export: built with the host's
Mandate ports, it asks Mandate's `check` for each capability on the commission's case, as the
Mandate verified context the host put in the commission's authority context (`{"verified": …}`).
Mandate's allow is a grant, its approval requirement suspends the run as the approval gate does,
and a Mandate outage or an authority context that is no Mandate context grants nothing. The case
is the Mandate resource of type `loom.case` whose id `loom_mandate::case_resource_id` derives from
the case id (a UUID version 5, never a parse of the id); the host grants on that id in Mandate.

The governor (`CanonGovernor`), the executor (`Loom`) and the loop (`run_until_blocked`) come from
the SDK. A model-backed selector and argument generator exist too: the slice's `ModelSelector` and
`ModelArguments` over any llm `Model`, which `b10x-loom run` uses.

## The loop

The governor holds the case; `case::open` starts it at the work tree's `HEAD`:

```rust
let governor = CanonGovernor::new(MemoryCaseStore::default());
let case = case::open(&governor, PROTOCOL, INTENT, &workspace)?;
```

Loom proposes; the runtime rechecks each proposal against the frontier and the authority provider,
and hands what it admits to the effect port:

```rust
let loom = Loom::new(ScriptedSelector::new(), ScriptedArguments, INTENT);
let end = run_until_blocked(
    &governor,
    &loom,
    &NoGrant,
    &effects,
    &commission(&case),
    &mut runs,
    &mut Clock,
);
```

`end` is a `LoopEnd`: its `outcome` (`NeedsAuthority` or `AwaitingApproval` here), the action
requests with their revalidation, the effects, and the last frontier. The example turns it into
`ApprovalRequired (repository.merge)`.

## Meter a run

`RunMeter`, `MeteredModel` and `MeteredExecutor` (top-level in the SDK, from
`loom_sdk::loom::metering`) report what one run spent on its model. Create one `RunMeter`, with
the run's `RateCard` (`loom_sdk::RateCard`) or none; wrap every llm `Model` the run uses in a
`MeteredModel` on that meter and hand the metered model to whatever asks it (the model
selector, an argument generator, your own requests), and the executor you hand
`run_until_blocked` in a `MeteredExecutor` on it. When the loop returns,
`meter.report(&end.run_id)` is a `loom.metering.RunUsageReport`: the model calls, the input,
output and cached input tokens the provider reported and the cost in micro-US-dollars, in total
and per step.

- Every `turn` of a metered model is one call, counted before the turn runs, so a turn that
  fails, unwinds or is dropped unfinished is a call too; it adds no tokens. A metered model
  wrapped again on the same meter counts each turn once.
- A step is one executor call, counted from 1, and holds the calls made while it ran: a `turn`
  called on the executor call's own thread, or on another thread (one it spawned, or yours) while
  it runs, whichever thread then drives the turn's future. Calls made while no executor call runs
  are step 0, listed only when there were any, so the steps always sum to the totals. When
  executor calls on one meter overlap, each holds the calls made on its own thread, and a call
  made on a thread that runs none of them is counted in the one that started last. An executor
  is wrapped by one `MeteredExecutor`.
- A step's `label` names what its executor call did: a `WorkflowExecutor` built
  `.with_meter(&meter)` labels each call with the workflow step it took, so a judgement step's
  model calls read under its name. Step 0 and a call that took no step carry no label.
- Each turn is priced once at the rate card, against the model the provider reported serving
  it, else the bound one. Without a card, for a model the card does not list, after a turn whose
  usage is missing, not final or names no input or output tokens, or after a turn that failed
  or unwound, the cost is absent, never zero.
- An executor that is not a model loop, such as one over an agent session, makes no turn of a
  metered model. It reports what each call spent by implementing `run_with_usage` (an
  `ExecutorStep`: the outcome and an optional `ExecutorUsage` of input and output tokens, wall
  time and cost), and its `run` returns that call's outcome. The runtime charges the usage to the
  run's budget, and the `MeteredExecutor` adds its tokens and cost to the call's step; it is no
  model call. A step's cost needs no card for that: without one, the step's cost is the cost the
  executor reported, absent when its usage carried none, and zero for a step that reported no
  usage and made no model call. An executor of yours that wraps another forwards `run_with_usage`,
  or the inner executor's usage reaches neither the run nor the meter.
- The report covers what the meter saw: one process segment of the run. A `WorkflowExecutor` with a
  run store (`.with_store(store)`, then `begin(&run_id)`) carries the usage in its run record, and
  `resume(&run_id)` in the next process, called after `.with_meter(&meter)`, carries it into that
  meter: the report lists the earlier segments' steps first, numbered as they were, and sums their
  effect usage with this segment's. Call `executor.save()` after `meter.charge_effects(…)` so the
  record holds the segment's effects.

`crates/loom-executor/tests/run_metering.rs` drives these over scripted llm models.

## What holds whatever you supply

- A selection outside the catalogue is refused, whatever its confidence.
- Only the runtime builds an `AdmittedRequest`, so an effect port is handed nothing the runtime did
  not revalidate.
- The case moves to a new revision only after an effect the port reports as `Performed`.
- The run stops at an approval gate before it spends its step budget, and after 64 steps when the
  context sets no budget.

[Where Loom ends](../concepts/where-loom-ends.md) explains why the effect call is the runtime's and
not Loom's.
