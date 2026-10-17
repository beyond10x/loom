---
title: SDK stability
sidebar_label: SDK stability
sidebar_position: 4
description: The stable surface of b10x-loom-sdk, the ports a host implements and the loom_sdk paths control-plane imports, with the promise Loom makes for them.
source: crates/loom-sdk/src/lib.rs, crates/loom-sdk/src/llm.rs, crates/loom-sdk/src/models.rs, crates/loom-executor/src/wire/claude_code.rs, crates/loom-sdk/tests/host_ports.rs, control-plane's crates/control-plane-runtime (its loom_sdk imports), CHANGELOG.md
---

# SDK stability

`b10x-loom-sdk` (library `loom_sdk`) is the one crate an embedding depends on. This page names
the part of it Loom keeps stable: the ports a host implements, the types and functions an
embedding calls, and every `loom_sdk` path control-plane imports. It is hand-written and held by
[`crates/loom-sdk/tests/host_ports.rs`](https://github.com/beyond10x/loom/blob/main/crates/loom-sdk/tests/host_ports.rs):
that test implements every port below through `loom_sdk` paths alone, has one `use` per
control-plane path, and fails when either table here and the code disagree.

## The promise

- The surface on this page stays stable within a release line. A release that breaks any of it
  names each break in its `CHANGELOG.md` entry, on a line that starts `Breaking:`.
- Loom releases from source at bare-version tags and nothing is on a registry. A consumer pins a
  tag, `tag = "<version>"`, and moves by changing it:

  ```toml
  [dependencies]
  b10x-loom-sdk = { git = "https://github.com/beyond10x/loom", tag = "0.24.0" }
  ```

- Outside the promise:
  - items not on this page, even where `loom_sdk` reaches them through a module;
  - `#[doc(hidden)]` items;
  - generated model types (`loom_sdk::commission::model`, `loom_sdk::loom::model`,
    `loom_sdk::governor::model`). They change with their ESS declaration under `ess/`, and a
    release names such a change as a break when it makes one.

There is no 1.0 promise, deprecation window or version rule beyond the two lines above.

## The Canon release a Loom release links

A Loom release links exactly one Canon release, `b10x-canon` (`Cargo.lock` holds one), and the
engineering protocols built on it, `b10x-canon-engineering`. The governor's decision for a case is
the same JSON document that Canon release renders for the same case
(`crates/loom-governor/tests/canon_decision_bytes.rs`).

| Loom | `b10x-canon` | `b10x-canon-engineering` |
|---|---|---|
| 0.24.0 | 0.6.0 | 0.8.0 |

A release that moves either pin names it in its `CHANGELOG.md` entry.

## Ports a host implements

Each port is a trait the host implements and hands to the runtime. The path is the one the test
implements it through.

| Port | Path | What the host supplies |
|---|---|---|
| `AuthorityProvider` | `loom_sdk::commission::ports::authority::AuthorityProvider` | Whether a commission holds a capability, from organizational authority |
| `SuspensionPort` | `loom_sdk::commission::ports::suspension::SuspensionPort` | Durable storage of a run suspended on a withheld approval |
| `FallibleCaseStore` | `loom_sdk::governor::FallibleCaseStore` | A case store whose reads and writes can fail |
| `CaseStore` | `loom_sdk::governor::CaseStore` | A case store that cannot fail |
| `EvaluationTime` | `loom_sdk::governor::EvaluationTime` | The trusted time a case is evaluated at |
| `EffectPort` | `loom_sdk::commission::ports::effect::EffectPort` | Performing an admitted action |
| `ConnectorInvoker` | `loom_sdk::commission::ports::connector::ConnectorInvoker` | Invoking the connector operation an action is bound to |
| `ObservationPort` | `loom_sdk::commission::ports::evidence::ObservationPort` | Receiving the observations a run reports |
| `EvidencePort` | `loom_sdk::commission::ports::evidence::EvidencePort` | Receiving evidence attributed to its producer |
| `ActionSelector` | `loom_sdk::loom::selection::ActionSelector` | Picking one action from the catalogue |
| `WorkflowRunStore` | `loom_sdk::loom::workflow::WorkflowRunStore` | Durable storage of a workflow run's record, so the run resumes after a restart |
| `ArgumentGenerator` | `loom_sdk::loom::arguments::ArgumentGenerator` | Arguments for the selected action only |
| `ModelPort` | `loom_sdk::loom::harness::wire::ModelPort` | One model turn over a wire |
| `ToolPort` | `loom_sdk::loom::harness::wire::ToolPort` | The tools a turn may call, and their calls |
| `BearerSource` | `loom_sdk::loom::harness::wire::BearerSource` | The credential a wire sends, resolved at each call |
| `ApprovalPort` | `loom_sdk::loom::harness::turn_loop::ApprovalPort` | Approving or denying a tool call |
| `TurnEnvironmentProvider` | `loom_sdk::loom::harness::turn_loop::TurnEnvironmentProvider` | The context and tools of each turn |
| `HookPort` | `loom_sdk::loom::harness::turn_loop::HookPort` | Hooks before and after a tool call and at a stop |
| `LoopSink` | `loom_sdk::loom::harness::turn_loop::LoopSink` | Receiving the loop's events |
| `StreamSink` | `loom_sdk::loom::harness::wire::StreamSink` | Receiving a turn's stream events |
| `LoopContext` | `loom_sdk::commission::runtime::LoopContext` | Ids, the time and the step budget, which no model supplies |

`LoopContext` and `EffectPort` are also at the top (`loom_sdk::LoopContext`,
`loom_sdk::EffectPort`), as are `ActionSelector`, `ArgumentGenerator` and `CaseStore`.

## Types and functions an embedding calls

| Item | What it is |
|---|---|
| `loom_sdk::run_until_blocked` | Drives a commission until it is blocked |
| `loom_sdk::LoopEnd` | What `run_until_blocked` returns |
| `loom_sdk::Loom` | The executor: proposes one action per frontier |
| `loom_sdk::CanonGovernor` | The governor over a case store |
| `loom_sdk::MemoryCaseStore` | An in-memory `CaseStore` |
| `loom_sdk::CaseInputs` | The case inputs a host gives when it opens a case |
| `loom_sdk::CaseIndependence` | The independence dimensions a host declares for each evidence producer when it opens a case |
| `loom_sdk::ProtocolCatalog` | The protocol catalog shared by routing, case opening and admission |
| `loom_sdk::llm::LlmModelPort` | A `ModelPort` over an llm `Model`: one `Model::turn` per turn, the port `b10x-loom` runs a plugin turn on |
| `loom_sdk::models::from_config` | Loads a host's models from a models configuration (YAML): the model of each declared role, over the `llm.routes/1` document it names, relative to the configuration's directory; every endpoint of the routes document must resolve, including ones no role uses ([llm issue 36](https://github.com/beyond10x/llm/issues/36)) |
| `loom_sdk::models::from_routes` | The llm `Model` one name of an `llm.routes/1` document serves: a route, or `<endpoint>/<model-id>` of a declared endpoint; every endpoint of the document must resolve, including ones no role uses ([llm issue 36](https://github.com/beyond10x/llm/issues/36)) |
| `loom_sdk::models::parse_config` | Parses a models configuration into a `ModelsConfig`, the parser `from_config` uses |
| `loom_sdk::models::Models` | What `from_config` returns: the model of each declared role (`get`, and `name_of` for the name that serves it), any other name of the same routes document (`model`), and `check_credential`, which resolves the `env:` or `file:` secret of the endpoint a name reaches and drops the value, so a host can refuse a missing secret before any model call |
| `loom_sdk::models::ModelsError` | Why models could not be loaded; its message names the file and the role or name |
| `loom_sdk::models::ModelsConfig`, `loom_sdk::models::ModelRole`, `loom_sdk::models::RoleModel` | The models configuration, generated from `loom.models` under `ess/`; it changes with that declaration |
| `loom_sdk::loom::wire::ClaudeCodePort` | A `ModelPort` on an operator's Claude Code login: one `claude -p --output-format json` invocation per turn, the command reading its own login |
| `loom_sdk::TestRunner`, `loom_sdk::SubstrateRunner`, `loom_sdk::UnconfinedRunner`, `loom_sdk::TestExecution`, `loom_sdk::ConfinementError` | The slice's test runners |
| `loom_sdk::commission`, `loom_sdk::connectors`, `loom_sdk::loom`, `loom_sdk::governor`, `loom_sdk::protocols`, `loom_sdk::intake::router`, `loom_sdk::intake::slice` | The re-exported crates, under these module names |

## Used by control-plane

control-plane embeds `loom_sdk` in `crates/control-plane-runtime`. These are the `loom_sdk` paths
it names: its imports, the items it names inline, and the items of `loom_sdk::loom::harness::wire`
it names through that module. Associated functions and enum variants are covered by their type.
Rows marked *generated* are generated model types or modules, outside the promise above.

| Path | Kind |
|---|---|
| `loom_sdk::ActionSelector` | port |
| `loom_sdk::ArgumentGenerator` | port |
| `loom_sdk::CanonGovernor` | type |
| `loom_sdk::Loom` | type |
| `loom_sdk::LoopContext` | port |
| `loom_sdk::commission::model::behaviour::Generated` | generated |
| `loom_sdk::commission::model::json` | generated |
| `loom_sdk::commission::model::primitives::Timestamp` | generated |
| `loom_sdk::commission::model::primitives::Uuid` | generated |
| `loom_sdk::commission::model::responsibility` | generated |
| `loom_sdk::commission::outcome::RunStore` | type |
| `loom_sdk::commission::ports::authority::AuthorityProvider` | port |
| `loom_sdk::commission::ports::authority::AuthorityProviderError` | type |
| `loom_sdk::commission::ports::effect::AdmittedRequest` | type |
| `loom_sdk::commission::ports::effect::EffectError` | type |
| `loom_sdk::commission::ports::effect::EffectPort` | port |
| `loom_sdk::commission::ports::evidence::submit_evidence` | function |
| `loom_sdk::commission::ports::executor::AgentExecutor` | trait |
| `loom_sdk::commission::ports::governor::Governor` | trait |
| `loom_sdk::governor::CaseState` | type |
| `loom_sdk::governor::FallibleCaseStore` | port |
| `loom_sdk::loom::arguments::ArgumentContext` | type |
| `loom_sdk::loom::harness::turn_loop::AgentLoop` | type |
| `loom_sdk::loom::harness::turn_loop::Budget` | type |
| `loom_sdk::loom::harness::turn_loop::DenyAll` | type |
| `loom_sdk::loom::harness::turn_loop::LoopConfig` | type |
| `loom_sdk::loom::harness::turn_loop::LoopEvent` | type |
| `loom_sdk::loom::harness::turn_loop::LoopSink` | port |
| `loom_sdk::loom::harness::turn_loop::OutputSchema` | type |
| `loom_sdk::loom::harness::turn_loop::RunLedger` | type |
| `loom_sdk::loom::harness::wire` | module |
| `loom_sdk::loom::harness::wire::Item` | type |
| `loom_sdk::loom::harness::wire::ModelPort` | port |
| `loom_sdk::loom::harness::wire::StreamSink` | port |
| `loom_sdk::loom::harness::wire::ToolCall` | type |
| `loom_sdk::loom::harness::wire::ToolName` | type |
| `loom_sdk::loom::harness::wire::ToolOutcome` | type |
| `loom_sdk::loom::harness::wire::ToolPort` | port |
| `loom_sdk::loom::harness::wire::ToolSpec` | type |
| `loom_sdk::loom::harness::wire::TurnOutcome` | type |
| `loom_sdk::loom::harness::wire::TurnRequest` | type |
| `loom_sdk::loom::harness::wire::Usage` | type |
| `loom_sdk::loom::harness::wire::WireError` | type |
| `loom_sdk::loom::harness::wire::WireErrorCode` | type |
| `loom_sdk::loom::harness::wire::WireId` | type |
| `loom_sdk::loom::model::primitives::Uuid` | generated |
| `loom_sdk::loom::model::run::CatalogueEntry` | generated |
| `loom_sdk::loom::model::run::CommissionRunId` | generated |
| `loom_sdk::loom::model::run::RunEnding` | generated |
| `loom_sdk::loom::model::run::SelectionStrategy` | generated |
| `loom_sdk::loom::model::run::SessionData` | generated |
| `loom_sdk::loom::model::run::SessionId` | generated |
| `loom_sdk::loom::selection::Choice` | type |
| `loom_sdk::loom::selection::SelectionContext` | type |
| `loom_sdk::loom::selection::SelectorError` | type |
| `loom_sdk::loom::session::SessionFile` | type |
| `loom_sdk::run_until_blocked` | function |

The [embedding guide](../guides/embed-the-runtime.md) walks through a whole embedding with the
ports it supplies.
