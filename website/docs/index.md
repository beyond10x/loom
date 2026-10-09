---
slug: /
title: Overview
sidebar_label: Overview
sidebar_position: 1
description: What Loom is, what it holds, where it stops, and which neighbours it works with.
---

# Loom

Loom is the runtime for governed agents. It makes a model behave like an agent for one bounded
run, inside a governed frontier, and it holds everything that run needs in one repository:

| Part | Package | What it does |
|---|---|---|
| The executor | `b10x-loom-executor` | Projects the frontier into a catalogue, selects one action, generates its arguments, and ends at a `ProposedAction` |
| Commission | `b10x-loom-commission` | The contracts and the runtime loop: it rechecks every proposal against the frontier, the case revision and authority, then invokes it through an effect port |
| The governor | `b10x-loom-governor` | Evaluates a case's protocol with Canon and issues the frontier; it decides and never acts |
| Intake | `b10x-loom-intake-*` | Routes an intent to a protocol and runs a local slice that edits a git work tree and runs its tests |
| The command line | `b10x-loom-cli` | `b10x-loom run`: an intent in, a governed run until it is blocked |
| The SDK | `b10x-loom-sdk` | One crate an application depends on to embed all of the above |

```text
intent            "make the failing test pass"
   ↓
router            proposes a protocol (a proposal, never authority)
   ↓
governor          evaluates the case with Canon → frontier
   ↓
Loom              catalogue → one action → its arguments → ProposedAction
   ↓
Commission        rechecks frontier, revision, authority → effect port
   ↓
stopped           ApprovalRequired (repository.merge), or another stated reason
```

A selector can be wrong about which admissible action is best. It cannot produce an action the
frontier does not contain, and nothing it says becomes evidence.

:::caution[Early]
Loom's current release is `0.13.0`, from source at that tag; consumers pin `tag = "0.13.0"`.
`b10x-loom run` has completed a live run against a hosted model ([qualification record][live]); the
SDK example runs on scripted fake models. Each page says whether what it describes is
**shipped**, **decided** or **planned**, and the [status page](/docs/status) lists every capability.
:::

[live]: https://github.com/beyond10x/loom/blob/main/docs/qualification/2026-10-05-b10x-loom-live-run.md

## What Loom does not do

- It does not decide what is legitimate. Protocol semantics belong to
  [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)), and
  the engineering protocols it runs (`software-change@1`) to
  [engineering protocols](https://beyond10x.github.io/engineering-protocols/)
  ([GitHub](https://github.com/beyond10x/engineering-protocols)).
- It does not hold credentials or speak a provider's wire. Model calls go through
  [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)); the
  credential is the operator's, read by llm.
- It does not grant authority. A capability an action needs comes from an authority provider the
  embedder supplies; the command line's provider grants none, so a run stops at the merge.
- It does not merge, push or deploy. It confines tests with Substrate; an explicit
  `--confinement none` opts out.
- It does not keep the engineering record of a case or compare harnesses. Both are neighbours'
  work: [AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep)) keeps
  the record, and [Metaharness](https://beyond10x.github.io/metaharness/)
  ([GitHub](https://github.com/beyond10x/metaharness)) drives and compares harnesses, with Loom as
  the native one.

Loom is specified in [ESS](https://beyond10x.github.io/ess/)
([GitHub](https://github.com/beyond10x/ess)); the [specification reference](/docs/reference/ess)
is generated from the repository.

## Two rules about trust

- **The model is not trusted context.** It never supplies identity, authority, case revision,
  trusted time, approval results or tenant context. Those come from the governed side of the run.
- **A trace is not evidence.** What Loom records about a run helps you understand and compare runs.
  Evidence comes from a trusted verifier: in the slice, the exit status of the test command it ran
  itself.

## Where to go next

- [Getting started](./getting-started.md): build it, run the SDK example, read a live run
- [Run an intent](./guides/run-an-intent.md) and [embed the runtime](./guides/embed-the-runtime.md)
- [One run, five steps](/docs/concepts/one-run) and [where Loom ends](./concepts/where-loom-ends.md)
- [The governor and intake](./concepts/governor-and-intake.md)
- [Crates](./reference/crates.md) and the [`b10x-loom` reference](./reference/cli.md)
