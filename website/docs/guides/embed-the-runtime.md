---
title: Embed the runtime
sidebar_position: 2
description: Depend on b10x-loom-sdk, supply the selector, argument generator, authority provider and loop context, and run a governed case until it is blocked.
lede: An application embeds a governed agent through one crate, b10x-loom-sdk. It supplies the parts that decide and the parts that act; Loom, Commission's runtime and the governor do the rest.
source: crates/loom-sdk/src/lib.rs, crates/loom-sdk/examples/software_change.rs, crates/loom-sdk/tests/example_runs.rs
---

# Embed the runtime

[`crates/loom-sdk/examples/software_change.rs`](https://github.com/beyond10x/loom/blob/main/crates/loom-sdk/examples/software_change.rs)
is a whole embedding in one file, and `crates/loom-sdk/tests/example_runs.rs` runs it in CI. This
page walks through it. [Getting started](../getting-started.md) shows its output.

## Depend on the SDK

Nothing is on a registry. Depend on the release tag:

```toml
[dependencies]
b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.13.0" }
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

## What holds whatever you supply

- A selection outside the catalogue is refused, whatever its confidence.
- Only the runtime builds an `AdmittedRequest`, so an effect port is handed nothing the runtime did
  not revalidate.
- The case moves to a new revision only after an effect the port reports as `Performed`.
- The run stops at an approval gate before it spends its step budget, and after 64 steps when the
  context sets no budget.

[Where Loom ends](../concepts/where-loom-ends.md) explains why the effect call is the runtime's and
not Loom's.
