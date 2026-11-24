---
title: Embed the runtime
sidebar_position: 2
description: Depend on b10x-loom-sdk, supply the selector, argument generator, authority provider and loop context, and run a governed case until it is blocked.
lede: An application embeds a governed agent through one crate, b10x-loom-sdk. It supplies the parts that decide and the parts that act; Loom, Commission's runtime and the governor do the rest.
source: crates/loom-sdk/src/lib.rs, crates/loom-sdk/examples/software_change.rs, crates/loom-sdk/tests/example_runs.rs, crates/loom-mandate/src/lib.rs, crates/loom-executor/src/metering.rs, crates/loom-tree/src/embed.rs, crates/loom-tree/tests/embedder/src/main.rs
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
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.61.0" }
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

A host that sets `IntentRequest.bindings` also needs `IntentRequest.runner`: the bound commands
start only through `TestRunner::run_child`, never through a process the library starts, and a
request with bindings, no runner and no `"unconfined_bindings": true` in its file is refused
`NoRunner` before any model call. `EngineRunner` and `SubstrateRunner` start child commands;
`UnconfinedRunner` and a runner that keeps the defaults of `run_child` and `preflight_child` do
not, and are refused `NoRunner` too. A host opts its bound commands in to Substrate's `local`
adapter as the command line's flag does, by passing an `EngineRunner` opened on `local` with
`bindings::UNCONFINED_FLAG_REASON`; a `local` runner opened with any other reason is refused
`CommandUnconfined`.

An embedded host supplies an already delegated cgroup root to `SubstrateRunner::new`, or as
`cgroup_root` in `EngineConfinement`. Every runner of the process shares one Substrate driver per
cgroup root, workspace directory and egress, so several case trees in one process, each run from
its own thread through its own `EngineRunner`, run their confined programs at the same time. To
run them:

1. Run the process in a delegated cgroup v2 subtree, the process in a leaf below the root:
   `prepare_delegated_cgroup` does this inside a scope that
   `systemd-run --user --scope -p Delegate=yes` started.
2. Hold the root once with `hold_cgroup_root`, give every run the held root, `hold.root()`, as
   `cgroup_root`, and keep the hold until the last run ends. A second hold of the root, in this
   process or another, is refused `CgroupUndelegated`; a runner takes no hold of its own.
3. Give each run its own workspace directory, and make all of them children of
   one shared directory, for example `<state>/workspaces/<run>`.
4. Open one `EngineRunner` per run, or share one: both work.

A launch that needs a driver no runner of the process holds, while another launch runs under the
same root, is still refused `CapabilityUnserved`: a workspace in another directory, or an egress
set or certificate bundle that no running driver has. Bound programs name no egress, so for them
the directory rule is the only one. A driver closes once the last runner that used it is dropped.
A cgroup root that holds no process of this one, such as one made per run beside the process's
leaf, is refused `CgroupUndelegated`. A tree runner from `b10x_loom_tree::embed`
(`loom_sdk::tree`, [Run a case tree](#run-a-case-tree)) never discovers one: it holds the root it
is given, or the delegated scope the process runs in. Automatic systemd re-exec belongs to the
CLI.
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

## Run a case tree

An engine that runs case trees in-process, as `b10x-loom tree` does, depends on `b10x-loom-tree`
alone. Its module `embed` (also `loom_sdk::tree`) holds what the tree functions do not:

- `TreeRunner`, the runner `RunSettings::runner` takes. `TreeRunner::on_root(root)` holds a
  delegated cgroup root for the process and builds Substrate's `linux` adapter on it; a root
  another process, or another runner of this process, holds, or one that does not qualify
  (empty, writable, delegated, this process in a child of it) is refused `ConfinementRefused`
  naming it. `TreeRunner::in_own_scope()`, in a process an operator started under `systemd-run
  --user --scope -p Delegate=yes`, moves the process, every thread of it, into a leaf of that
  scope and holds the scope. `TreeRunner::local()` opens Substrate's `local` adapter, unconfined,
  for the one reason the bound-program port admits. None of them discovers a root.
- `runner()` hands out the runner, which holds the root: it is released when the `TreeRunner`
  and every clone of that runner are dropped. Keep one `TreeRunner` per root for the process;
  threads share its runner.
- `host_catalog()`, the catalog `b10x-loom` reads; `ProtocolCatalog`; `encode_line`, the bytes of
  a run-log line your `RunObserver` gets, without the line feed the log adds; `Cancel`, the handle
  a run sets from your flag; and `DEFAULT_PRINCIPAL`, `operator`, the principal an `Approve`
  decision names.

`crates/loom-tree/tests/embedder/` is such an engine, and its tests run it on a delegated root,
in a scope of its own and under `local`. This is its code: the runner, the settings, a run with an
observer and a flag another thread sets, and the approval the first run waits on. It then runs
again, closes the case with `close` and reads it with `tree_status`. `RunSettings::confinement`
and `cgroup_root` are what the `b10x-loom tree run` command a `Handoff` decision prints carries,
`--confinement` and `--cgroup-root`, so that it runs as this run did; name no root for a run in a
scope of its own, which the command starts again.

```rust
use b10x_loom_tree::embed::{self, DEFAULT_PRINCIPAL, ProtocolCatalog, TreeRunner, encode_line};
// …
let (runner, confinement, cgroup_root) = match &arguments.mode {
    Mode::Root(root) => (TreeRunner::on_root(root)?, "substrate", Some(root.clone())),
    // …
    Mode::Local => (TreeRunner::local()?, "local", None),
};
// …
let settings = RunSettings {
    max_steps: None,
    max_wall_seconds: None,
    time: time.clone(),
    runner: runner.runner(),
    confinement: Some(confinement.into()),
    cgroup_root,
};
// …
let mut observer =
    |case: &str, line: &RunLogLine| say(&format!("line {case} {}", encode_line(line)));
let flag = &AtomicBool::new(false);
let (ran, set) = std::thread::scope(|scope| {
    let setter = cancel_after.map(|after| {
        scope.spawn(move || {
            std::thread::sleep(after);
            flag.store(true, Ordering::SeqCst);
            Instant::now()
        })
    });
    let ran = run_tree(store, catalog, settings, Some(&mut observer), Some(flag));
    (ran, setter.and_then(|setter| setter.join().ok()))
});
// …
approve(&ApproveRequest {
    store,
    case: &waits.case,
    capability: waits.capability.as_deref(),
    principal: DEFAULT_PRINCIPAL,
    decision: ApprovalDecision::Allow,
    action: None,
    result: None,
    reasons: None,
    time: time.clone(),
    catalog: &catalog,
})?;
```

## What holds whatever you supply

- A selection outside the catalogue is refused, whatever its confidence.
- Only the runtime builds an `AdmittedRequest`, so an effect port is handed nothing the runtime did
  not revalidate.
- The case moves to a new revision only after an effect the port reports as `Performed`.
- The run stops at an approval gate before it spends its step budget, and after 64 steps when the
  context sets no budget.

[Where Loom ends](../concepts/where-loom-ends.md) explains why the effect call is the runtime's and
not Loom's.
