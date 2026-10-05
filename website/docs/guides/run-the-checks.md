---
title: Run the checks
sidebar_position: 4
description: The repository's gate, task check, what each step holds, and how to run one alone.
lede: task check is the one gate a change to Loom passes; each of its steps can be run alone while you work.
source: Taskfile.yml, Taskfile.commission.yml, .github/workflows/pages.yml
---

# Run the checks

You need Rust, the [Task runner](https://taskfile.dev/) and the `ess` command line of
[ESS](https://beyond10x.github.io/ess/) ([GitHub](https://github.com/beyond10x/ess)). From the
repository root:

```console
task check
```

It runs, in order:

| Step | Holds |
|---|---|
| `task spec`, `task commission:spec`, `task intake-spec` | The three ESS systems (`ess/`, `ess/commission/`, `ess/intake/`) validate |
| `task ess-gate`, `task commission:ess-gate` | The hard gate: validate strictly, compile, synthesize with no refusal, no open question |
| `task drift`, `task commission:drift` | The generated Rust model equals a fresh synthesis of the specification |
| `task no-hand-model`, `task commission:no-hand-model` | No hand-written type shadows one the specification declares |
| `task commission:conform` | Commission passes its synthesized ESS conformance suite |
| `task commission:deps-guard` | Commission's contracts depend on no Loom executor, no Canon and no model-provider crate |
| `cargo fmt --all --check`, `cargo clippy … -D warnings`, `cargo test --workspace --locked` | Format, lint and every test; no test makes a model or network call |
| `task docs-check`, `task commission:docs-drift` | The generated documentation pages are current |

`task --list` names every task. To run one alone, name it:

```console
task intake-spec
```

```text
task: [intake-spec] ess specify validate --path ess/intake
intake v1 — 2 file(s), valid
```

## Documentation

The pages under `website/docs/reference/` are generated: the [CLI reference](../reference/cli.md),
the [crate list](../reference/crates.md) and the [ESS reference](/docs/reference/ess) by
`loom-docs`, Commission's reference pages by `loom-commission-docs`. Change the source, then
regenerate:

```console
task docs-generate
task commission:docs
```

`task docs-check` fails on a page that is missing, stale or edited by hand. To build the site as
CI's `Documentation validation` workflow does:

```console
task website
```
