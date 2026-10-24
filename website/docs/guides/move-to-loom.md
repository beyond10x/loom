---
title: Move from Commission, the governor, intake or Harness
sidebar_label: Move to Loom's crates
sidebar_position: 3
description: The repositories Loom absorbed are archived; this page maps their crates, libraries and command to Loom's, and says what a Harness program uses instead of Loom's deleted port.
lede: Commission, the governor and intake now live in Loom under new package and library names. A consumer of the archived repositories changes its dependency lines and its use paths; behaviour is unchanged. Loom no longer carries Harness; a Harness program keeps Harness, and Loom reaches models through llm and agent work through metaharness.
source: Atlas ADRs 0090 and 0098; CHANGELOG.md (Unreleased); crates/loom-executor/tests/crate_names.rs
---

# Move from Commission, the governor, intake or Harness

`beyond10x/commission`, `beyond10x/governor` and `beyond10x/intake` are archived, and their READMEs
point here. Their code and history are in Loom (Atlas ADR 0090). Two things change for a consumer:
where the crate comes from, and its name.

## Dependency lines

Every crate now comes from the Loom repository:

```toml
[dependencies]
b10x-loom-commission = { git = "https://github.com/beyond10x/loom", tag = "0.40.0" }
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
- The engineering protocol registry is `b10x-canon-engineering` `0.9.0` from
  [engineering protocols](https://beyond10x.github.io/engineering-protocols/)
  ([GitHub](https://github.com/beyond10x/engineering-protocols)); it replaces `b10x-els`. Its
  `software.change/1` declares the case inputs `risk`, `change_kind` and
  `affects_security_boundary`, which every case opens with (`CaseInputs`, the third argument of
  `CanonGovernor::open`), and declares the write actions `tests.write` and `implementation.edit`
  where earlier releases had `repository.edit`.
- [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon)) is
  the revision `b10x-canon-engineering` names, tag `0.6.0`. A consumer that adds Canon itself
  uses the same reference, or it builds a second Canon whose types do not match.

The [crate list](../reference/crates.md) is generated from the workspace and is the current list.

## Move from Harness

Loom does not carry [Harness](https://beyond10x.github.io/ecosystem/harness/)
([GitHub](https://github.com/beyond10x/harness)). Releases up to 0.30.0 shipped a port of its
wires, turn loop and sessions under `loom_sdk::loom::harness`; Loom deleted it (Atlas ADR 0098),
and each removed path is named in a `**Breaking:**` bullet of `CHANGELOG.md`. Harness itself is
unchanged and stays in service, so a Harness program keeps its Harness dependency.

What Loom reaches instead:

- **Decisions go to llm.** Intake routing, action selection, argument generation and a workflow's
  judgement step are each one structured request to an llm `Model` the host builds, through
  [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)). A host
  that ran the ported loop over the SDK's llm bridge (`loom_sdk::llm`) hands Loom its llm `Model`
  instead: `loom_sdk::loom::model_selector` selects and generates arguments on it, and
  `loom_sdk::RunMeter` meters it ([embed the runtime](./embed-the-runtime.md)).
- **Agent work goes to metaharness.** A session in a harness (Harness, Claude Code or Codex)
  runs through [Metaharness](https://beyond10x.github.io/metaharness/)
  ([GitHub](https://github.com/beyond10x/metaharness)), behind an `AgentExecutor`: a host
  supplies its own today, and Loom's executor over a metaharness session is planned. Loom no
  longer reaches Claude Code through a `claude -p` model port.
- **Tools come from the frontier, and a call is a proposal.** The catalogue a step offers is
  projected from the case's current frontier, and a selection is revalidated before Loom proposes
  it; the Commission runtime invokes an admitted action through the effect port the embedder
  supplies ([action selection](../concepts/action-selection.md),
  [where Loom ends](../concepts/where-loom-ends.md)). A Harness program that ran its tools inside
  the loop moves that work to the effect port.
