---
title: Status
sidebar_position: 3
description: What Loom implements today and what is planned.
---

# Status

Loom is at the bootstrap stage. One crate, `b10x-loom`, holds the executor contract and a
deterministic bootstrap selector. No model is called yet.

| capability | state |
|---|---|
| Loom implements Commission's `AgentExecutor` | **Shipped.** A run selects, checks, and returns an executor outcome. |
| Selections outside the frontier are refused | **Shipped.** An unknown or blocked action id fails the run, whatever its confidence. A unit test holds this. |
| Approval-gated actions suspend the run | **Shipped.** Loom returns `Suspended` naming the capability required, and proposes nothing. |
| The run ends at a `ProposedAction` | **Shipped.** No effect is invoked from Loom. That this is where Loom stops is [decided design](./concepts/where-loom-ends.md). |
| `ActionSelector` and `ArgumentGenerator` traits | **Shipped,** in a bootstrap shape: a selector sees the frontier and a prompt; arguments are a JSON string. |
| Bootstrap selector and empty-arguments generator | **Shipped,** for tests and examples only: the first admissible action, with `{}` as its arguments. |
| ESS specification and its hard gate | **Shipped.** A draft `loom.run` domain ([reference](./reference/ess/index.md)); `task check` fails unless it validates, compiles, synthesizes with no refusals and carries no open question. |
| Catalogue projection, schema validation of arguments, revalidation inside Loom | **Planned.** |
| Reasoning-model, fast typed and hybrid selectors; confidence fallback; selection telemetry | **Planned.** |
| Harness port: model wires, turn loop, sessions and transcripts, streaming, compaction, budgets, interruption and recovery | **Planned.** The module map is done; the port is not. |

## Build from source

```bash
git clone https://github.com/beyond10x/loom.git
cd loom
task check
```

`task check` needs Rust, the [Task runner](https://taskfile.dev/) and the `ess` CLI. It includes
`task docs-check`, which fails when the generated reference pages have fallen behind the
specification. `task website` builds this site.
