---
title: Move from Commission, the governor or intake
sidebar_label: Move to Loom's crates
sidebar_position: 3
description: The repositories Loom absorbed are archived; this page maps their crates, libraries and command to Loom's.
lede: Commission, the governor and intake now live in Loom under new package and library names. A consumer of the archived repositories changes its dependency lines and its use paths; behaviour is unchanged.
source: Atlas ADR 0090; CHANGELOG.md (Unreleased); crates/loom-executor/tests/crate_names.rs
---

# Move from Commission, the governor or intake

`beyond10x/commission`, `beyond10x/governor` and `beyond10x/intake` are archived, and their READMEs
point here. Their code and history are in Loom (Atlas ADR 0090). Two things change for a consumer:
where the crate comes from, and its name.

## Dependency lines

Every crate now comes from the Loom repository:

```toml
[dependencies]
b10x-loom-commission = { git = "https://github.com/beyond10x/loom", tag = "0.13.0" }
```

An application that uses several of them can depend on `b10x-loom-sdk` alone; see
[embed the runtime](./embed-the-runtime.md).

## Names

| Before | Now | `use` |
|---|---|---|
| `b10x-commission` | `b10x-loom-commission` | `b10x_loom_commission` |
| `b10x-commission-testkit` | `b10x-loom-commission-testkit` | `b10x_loom_commission_testkit` |
| `b10x-commission-conformance` | `b10x-loom-commission-conformance` | `b10x_loom_commission_conformance` |
| `b10x-governor` (`use governor`) | `b10x-loom-governor` | `loom_governor` |
| `b10x-intake-references` (`use intake_references`) | `b10x-loom-intake-references` | `b10x_loom_intake_references` |
| `b10x-intake-router` (`use intake_router`) | `b10x-loom-intake-router` | `b10x_loom_intake_router` |
| `b10x-intake-slice` (`use intake_slice`) | `b10x-loom-intake-slice` | `b10x_loom_intake_slice` |
| `b10x-intake-cli`, command `b10x-intake run` | `b10x-loom-cli`, command `b10x-loom run` | `b10x_loom_cli` |
| `b10x-loom` | `b10x-loom-executor` | `b10x_loom_executor` |
| `b10x-intake-model` | removed: llm's `b10x-llm-tool-call` (`call_tool`, `codex_model`) | `b10x_llm_tool_call` |

`b10x-loom run` keeps the flags and exit statuses `b10x-intake run` had.

## Dependencies that moved with it

- Model calls use [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm))
  at release tag `0.3.1`, including the provider overload fix.
- The engineering protocol registry is `b10x-canon-engineering` `0.3.0` from
  [engineering protocols](https://beyond10x.github.io/engineering-protocols/)
  ([GitHub](https://github.com/beyond10x/engineering-protocols)); it replaces `b10x-els`.
- [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)) is
  the revision `b10x-canon-engineering` names, tag `0.1.0`. A consumer that adds Canon itself
  uses the same reference, or it builds a second Canon whose types do not match.

The [crate list](../reference/crates.md) is generated from the workspace and is the current list.
