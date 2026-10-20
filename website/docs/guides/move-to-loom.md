---
title: Move from Commission, the governor, intake or Harness
sidebar_label: Move to Loom's crates
sidebar_position: 3
description: The repositories Loom absorbed are archived, and Loom succeeds Harness; this page maps their crates, libraries and command to Loom's.
lede: Commission, the governor and intake now live in Loom under new package and library names. A consumer of the archived repositories changes its dependency lines and its use paths; behaviour is unchanged. A Harness program imports the ported crates from loom_sdk and keeps Harness for the crates Loom does not carry.
source: Atlas ADR 0090; CHANGELOG.md (Unreleased); crates/loom-executor/tests/crate_names.rs; docs/design/harness-map.md; crates/loom-executor/tests/harness_map.rs; crates/loom-sdk/tests/harness_paths.rs
---

# Move from Commission, the governor, intake or Harness

`beyond10x/commission`, `beyond10x/governor` and `beyond10x/intake` are archived, and their READMEs
point here. Their code and history are in Loom (Atlas ADR 0090). Two things change for a consumer:
where the crate comes from, and its name.

## Dependency lines

Every crate now comes from the Loom repository:

```toml
[dependencies]
b10x-loom-commission = { git = "https://github.com/beyond10x/loom", tag = "0.28.0" }
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

Loom succeeds [Harness](https://beyond10x.github.io/ecosystem/harness/)
([GitHub](https://github.com/beyond10x/harness)). Harness itself is unchanged and stays in
service, so a program moves when it is ready. Each Harness crate has the disposition the
repository's
[Harness to Loom module map](https://github.com/beyond10x/loom/blob/main/docs/design/harness-map.md)
gives it:

- `port`: the crate's source is carried into `b10x-loom-executor`, relicensed Apache-2.0, and a
  program imports it from `b10x-loom-sdk` (`use loom_sdk::loom::…`) instead of the Harness crate.
  The paths below are the types a Harness program reaches for first; each module keeps the
  Harness crate's other public items.
- `not carried`: Loom does not take the crate. It stays with Harness, and a program that needs it
  keeps its Harness dependency for that crate.

| Harness crate | Disposition | `loom_sdk` path | Owner instead | What changes |
|---|---|---|---|---|
| `harness-wire` | port | `loom_sdk::loom::harness::wire::ModelPort`, `loom_sdk::loom::harness::wire::ToolPort`, `loom_sdk::loom::harness::wire::BearerSource` | — | The values and ports keep their shape. In a governed run Loom implements `ToolPort` itself from the frontier, and no tool runs through it. |
| `harness-http` | port | `loom_sdk::loom::harness::http::HttpTransport`, `loom_sdk::loom::harness::http::RetryPolicy` | — | Nothing: the same framing, retry rule and status mapping. |
| `harness-responses` | port | `loom_sdk::loom::harness::responses::ResponsesClient` | — | Nothing: the same Responses projection and headers. |
| `harness-messages` | port | `loom_sdk::loom::harness::messages::MessagesClient` | — | Nothing: the same Messages projection and headers. |
| `harness-loop` | port | `loom_sdk::loom::harness::turn_loop::AgentLoop` | — | The module is `turn_loop`, as `loop` is a Rust keyword. A governed run goes through `loom_sdk::Loom` (`run_loop`, `resume_loop`), which publishes the frontier's catalogue and answers every tool call; see below. |
| `harness-cli` | port | `loom_sdk::loom::session` | — | Only session filing, resume by id, the refusal of a session recorded on another wire, and filing however the run ended are carried. A Harness version 1 session file is refused by name. |
| `harness-credential` | not carried | — | `beyond10x/harness` | A wire takes a `BearerSource`; Loom's resolves a credential reference (see below). |
| `harness-flow` | not carried | — | `beyond10x/harness` | Loom serves one bounded run; a workflow over several sessions stays with Harness. |
| `harness-substrate` | not carried | — | `beyond10x/harness` | Loom has no tool source of its own. |
| `harness-tools` | not carried | — | `beyond10x/harness` | Loom has no tool source of its own. |
| `harness-toolchain` | not carried | — | `beyond10x/harness` | Toolchain providers are engineering-domain semantics, not Loom's. |
| `harness-mcp` | not carried | — | `beyond10x/harness` | Loom has no tool source of its own. |
| `harness-app-server` | not carried | — | `beyond10x/harness` | Loom is driven through Commission's `AgentExecutor`, not a JSON-RPC server. |
| `harness-xtask` | not carried | — | `beyond10x/harness` | Harness's own gate. |

### What changes when the loop moves

- **Tools come from the frontier, not a registration.** A Harness `ToolPort` publishes whatever
  tools the program registered, and a `TurnEnvironmentProvider` can only narrow them. In Loom the
  catalogue a turn offers is projected from the case's current frontier, read again before every
  turn, so the tools change as the case moves
  ([action selection](../concepts/action-selection.md)). A program attaches no tool source to a
  governed run.
- **A tool call is a selection.** The model's call is selected from that turn's catalogue, given
  its arguments and revalidated against the case's current revision, frontier and authority
  before Loom proposes it. A call that fails is denied to the model, which chooses again on the
  next turn's catalogue ([action selection](../concepts/action-selection.md)).
- **A consequential call leaves Loom only as a proposal.** Loom runs no frontier action: the run
  stops at an approval checkpoint immediately before the effect and returns a `ProposedAction`,
  which the Commission runtime rechecks and invokes through the effect port the embedder
  supplies ([where Loom ends](../concepts/where-loom-ends.md)). A Harness program that ran its
  tools inside the loop moves that work to the effect port.
- **A credential comes from a reference.** `harness-credential` is not carried, and Loom reads no
  credential file or credential environment variable
  ([what Loom is not](../index.md)). A wire takes its credential from a `BearerSource`:
  `loom_sdk::loom::credentials::ResolvedBearer` resolves the credential reference a run
  configuration names through a resolver of llm's `b10x-llm-credentials` that the embedder
  injects, so no secret value sits in any configuration.

Budgets as a Commission suspension are planned, and `b10x-loom run` does not use the loop yet; see
[Commission and Harness](../concepts/commission-and-harness.md#harness-the-predecessor).
