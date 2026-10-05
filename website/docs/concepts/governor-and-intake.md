---
title: The governor and intake
sidebar_position: 6
description: How a case gets its frontier from Canon, and how an intent becomes a governed case that a local slice works until it is blocked.
status: shipped
lede: The governor evaluates a case's protocol with Canon and decides; it never acts. Intake turns an intent into a governed case and works it with a local slice whose only evidence is a test command it ran itself.
source: Atlas ADRs 0074, 0089 and 0090; crates/loom-governor, crates/loom-intake-references, crates/loom-intake-router, crates/loom-intake-slice; ess/intake/domains/routing.yaml
---

## The governor

`CanonGovernor` (`b10x-loom-governor`) implements Commission's governor, evidence and observation
ports over [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)).
A case is opened on a protocol of the
[engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)) registry, named `<name>@<major>`
(`software-change@1`), with the caller's revision of every artifact the protocol declares.

- **It evaluates afresh every time.** Each frontier and completion compiles the protocol and
  evaluates the case's evidence with Canon. It reads no clock, makes no network or model call, and
  supplies no authority decision.
- **It never invents authority.** An action that requires a capability is at best *approval
  required*; Commission asks its authority provider.
- **It decides and never acts.** It reports the frontier and whether one outcome is legitimate. It
  executes nothing.
- **Evidence applies to a revision.** A test result for the previous revision does not satisfy a
  claim about the current one: after an edit, the tests run again before the merge becomes
  possible.

The case store is a port (`CaseStore`); `MemoryCaseStore` keeps cases in memory.

## Intake

Intake takes what someone asked for, as given, and runs it.

| Crate | Does | Does not |
|---|---|---|
| `b10x-loom-intake-references` | Extracts tracker keys, chat permalinks, merge and pull requests and URLs from the intent, in order, without a model | Fetch what they point at |
| `b10x-loom-intake-router` | Offers a model every protocol of the registry and takes one forced `pick_protocol` call | Turn a pick into authority. A pick outside the registry or below the threshold is refused |
| `b10x-loom-intake-slice` | Opens the case through the governor, runs the Commission runtime with Loom proposing, performs `software.change/1` actions in the work tree, and submits test results as evidence | Merge, push or deploy; run a loop of its own; trust a model's account of what happened |

The slice's selector and argument generator (`ModelSelector`, `ModelArguments`) each ask a model
for one forced tool call through [llm](https://beyond10x.github.io/llm/)
([GitHub](https://github.com/beyond10x/llm)). The model sees the intent, its references and a
bounded transcript of what the executor reported. Whatever else it says is dropped.

Evidence comes only from the slice's verifier, from the exit status of the test command the
executor ran (Atlas ADR 0074). A transcript line that a model-written file imitates can mislead the
next choice; it cannot become evidence.

:::note[Test confinement in current source]
Tests use [Substrate](https://beyond10x.github.io/substrate/)
([GitHub](https://github.com/beyond10x/substrate)) with no network, read-only source and toolchain,
and workspace writes only to `target/`. Dependencies are prefetched explicitly. A missing
confinement guarantee stops the run; only `--confinement none` opts out. Every executed test
observation names its actual confinement. These changes follow release 0.1.0.
The slice's own git calls run none of
the work tree's hooks, no `core.fsmonitor` command and no signing program, a case does not open
on a work tree whose own git configuration names a program, and a call is refused once the test
command changed that configuration or the git directory.
:::

The `intake.routing` domain specifies these nouns; its [reference](/docs/reference/ess/intake-routing)
is generated from `ess/intake/`. [Run an intent](../guides/run-an-intent.md) shows the slice from
the command line.
