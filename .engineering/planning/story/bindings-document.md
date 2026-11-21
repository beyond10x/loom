---
format: aep.planning-md/3
id: story:bindings-document
kind: story
status: implemented
title: loom.bindings/1 declares per protocol action its executor, result table, tools, budget and timeout, checked at load
summary: The one binding format, grown from intake.bindings; document, decode and load-time checks, nothing executed
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-bindings
- confidence: inferred
  path: crates/loom-executor/tests/crate_names.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom/
- confidence: cited
  path: website/data/ess/
- confidence: inferred
  path: website/data/status.json
- confidence: inferred
  path: website/docs/guides/bindings.md
- confidence: cited
  path: website/docs/reference/crates.md
- confidence: cited
  path: website/docs/reference/ess/
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:43Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":5}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
- {from: "active", to: "implemented", at: "2026-11-21T01:32:44Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":7,"verification":1}}}
---
## Outcome

`loom.bindings/1` is the one document in which an operator declares, per action of one registered
protocol, which executor performs it and how its result becomes evidence. A new crate,
`b10x-loom-bindings` (`crates/loom-bindings`), decodes it and checks it against the compiled
protocol when it is loaded, and refuses a file that does not fit, naming the action and the
member. Nothing is executed by this story. Per action the document holds:

- `executor`, one of: `program` (an `argv` list, never a shell string), `agent` (an `adapter`
  name, a `model`, `inputs` and `outputs` as lists of workspace paths), `person` (the action is
  taken only by a named principal), `open` (a `name` or an `each` argument, and the child's
  bindings file), `copy` (an evidence `kind` and the protocol id of the case it is copied from);
- `arguments`: a literal object, or `arguments_from` (an argv whose standard output is the object);
- `evidence`: a list of entries `{kind, subject, producer, from, results, upstream}`, where `from`
  is `exit`, `effect` or a JSON pointer into the program's report, and `results` maps each value
  (or `*`) to an evidence result. `effect` is Loom's own report that it performed the action (it
  opened the children, applied the agent's outputs, took the person's action); its one value is
  `performed`, and an effect that was refused or failed records nothing;
- `tools.allow`: the tool names an `agent` executor may be offered;
- `budget` (`max_runs`, default 8, and for an agent `max_turns`, `max_tokens`,
  `max_budget_micro_usd`) and `timeout_seconds`;
- `retry_when` (`{kind: [results]}`: records about what this action wrote that send it back to
  work) and `handoff` (the text a run shows when this writer has done what it may and a person
  continues).

At the top: `format`, `protocol` (a registration, `name@major`), `artifacts` (artifact id to a
path in the workspace) and `independence` (producer to independence dimensions). The file is YAML;
JSON is read as YAML. Who uses each member: `arguments_from` and the `program` executor,
`story:bound-program-result-table`; `artifacts`, `story:bound-artifact-revisions`; the `agent`
executor, `tools` and `retry_when`, `story:bound-agent-node`; the `open` executor,
`story:child-case-runtime`; the `copy` executor, `story:record-copy-between-cases`; the `person`
executor, `handoff` and `independence`, `story:tree-run-command`.

## Why

A binding today is `{action, argv}` (`ess/intake/domains/bindings.yaml:30-36`), for one installed
protocol that declares the `intent` artifact alone and no case input
(`crates/loom-intake-slice/src/bindings.rs:150-174`). The SDC's bindings
(`~/beyond10x/sdc/driver/examples/sdc/*.bindings.json`) carry what a governed run needs beyond
that: `produces`, `subject`, `producer`, an exit-code table, `max_runs`, `timeout_s`, `llm`,
`child`, `copy`, `manual`, `args_from`, `retry_when`, `handoff`, `$artifacts`, `$independence`.
`~/beyond10x/sdc/docs/layers.md` § 2 counts ten meanings of "binding" and recommends one format
owned by Loom; `loom-sdk-and-agent-node.md` § "Loom: what a binding must declare" lists the
members. An adapter, an aperture and a remote endpoint are engine configuration and never binding
data (`layers.md` § 3). Session `sdc` reviewed this member list on 2026-11-21 and answered that it
fits.

## ESS first

A new domain `loom.bindings` in `ess/domains/bindings.yaml`, declared in `ess/system.yaml`:
`BindingsDocument`, `BoundAction`, `Executor` (the five variants), `EvidenceEntry`, `ResultSource`
(`Exit`, `Effect`, `Report`), `ActionBudget`, `ToolAllowList`, `RetryCondition`,
`BindingsRefusal`. On that commit `task drift` fails, and
`crates/loom-bindings/tests/bindings_document.rs` does not compile.

## Acceptance

- Decoding refuses an undeclared member at any level, a missing `format` or one other than
  `loom.bindings/1`, and an `argv` that is empty or not a list of strings, each naming the action
  and the member.
- The load-time check against a compiled protocol refuses, each by name: an action the protocol
  does not declare; a duplicate action; an evidence `kind` outside the action's `may_produce`; a
  `subject`, `upstream` entry or `artifacts` key that is not a declared artifact; a `tools`
  member, an agent budget member, `inputs` or `outputs` on a non-agent executor; a
  `timeout_seconds` outside 1 to 3600; an `artifacts`, `inputs` or `outputs` path that is
  absolute or leaves the workspace; an `independence` dimension the protocol does not declare
  (Canon's `independence_dimensions`).
- A `tools.allow` entry that is empty or holds a comma or white space is refused, naming the
  action and the entry (the session executor joins the names with commas,
  `crates/loom-session-executor/src/lib.rs:2412`).
- An entry with `from: effect` whose `results` has a key other than `performed` is refused,
  naming the action and the key; `{from: effect, results: {performed: opened}}` decodes.
- `crates/loom-bindings/Cargo.toml` names `b10x-canon` by the same reference as
  `crates/loom-governor/Cargo.toml`, and `Cargo.lock` still holds one copy
  (`the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` passes).
  `story:canon-0-9-0-pin` moves that manifest with the others and depends on this story for it.
- An action with no binding is not an error at load; a function lists the unbound actions, as
  `unbound()` does today.
- A decoded `agent` executor with no `model` holds `claude-haiku-5-5`; a decoded action with no
  `budget.max_runs` holds 8.
- `crates/loom-intake-slice/src/bindings.rs`, `intake.bindings` and `--action-bindings` are not
  touched: `git diff --stat` of the story names no file under `crates/loom-intake-slice` or
  `crates/loom-cli`.
- The domain appears in the generated ESS reference (`task docs-generate`, `task docs-check`),
  which lists every declared member; a guide page under `website/docs/` shows one complete file
  that `bindings_document.rs` decodes from the page's code block.
- `task check` exits 0.

## Out of scope

Executing any executor. The check that an `open` executor sits on an action with `case_open`
needs Canon 0.9.0 and is in `story:child-case-runtime`. A Connector operation as an executor
(`ports/connector.rs` `ActionBinding` stays as it is). Confinement profile deltas (apertures,
read-only roots) per binding. Accepting the format in `b10x-loom run --action-bindings`.

## Scope

Rewritten 2026-11-21 at the close of wave 2026-11-21-w1 from the unit's commits (`git diff --name-status ecefe180 2fe0743f`). Every line is cited from that diff.

- **Primary surface:** `crates/loom-bindings` (new crate `b10x-loom-bindings`, lib `b10x_loom_bindings`) — cited, 8 new files, 3,357 lines
- **Files:** `crates/loom-bindings/Cargo.toml` — new, cited, depends on `b10x-canon` (git tag `0.8.0`), `loom` (`../../generated/rust/loom`), `serde` and `serde_yaml_ng`; `publish = false`
- **Files:** `crates/loom-bindings/src/lib.rs` — new, cited, crate documentation, the re-exports and the limit constants
- **Files:** `crates/loom-bindings/src/decode.rs` — new, cited, the YAML or JSON decoder into the generated `loom.bindings` types
- **Files:** `crates/loom-bindings/src/load.rs` — new, cited, the load-time check against the compiled protocol and the list of unbound actions
- **Files:** `crates/loom-bindings/src/tree.rs` — new, cited, the bounded reader of the text (nesting, node count, document bytes, open brackets)
- **Files:** `crates/loom-bindings/tests/bindings_document.rs` — new, cited, the acceptance tests (991 lines)
- **Files:** `crates/loom-bindings/tests/adversary_w20261121w1_u2.rs` and `crates/loom-bindings/tests/adversary_w20261121w1_u2_pass2.rs` — new, cited, the adversary's two rounds
- **Files:** `ess/domains/bindings.yaml` — new, cited, the `loom.bindings` domain: 14 types from `Executor` to `BindingsRefusal`
- **Files:** `ess/system.yaml` — cited, `loom.bindings` added to `domains:`
- **Files:** `ess/ess-inputs.yaml` — cited, `domains/bindings.yaml` added to `specification:`
- **Files:** `crates/loom-executor/tests/crate_names.rs` — cited, `EXPECTED` goes from 28 to 29 entries, adding `b10x-loom-bindings` / `loom-bindings` / `b10x_loom_bindings`
- **Files:** `website/data/status.json` — cited, one item added, "Bindings document", status `shipped`
- **Files:** `website/docs/guides/bindings.md` — new, cited, the guide page (201 lines) holding a `loom.bindings/1` file
- **Files:** `website/docs/reference/crates.md` — cited, generated page: package count 28 to 29 and one row for `b10x-loom-bindings`
- **Files:** `generated/rust/loom/` — cited, regenerated: `src/bindings.rs` is new, `src/lib.rs` gains `pub mod bindings;`, and `Cargo.toml`, `PLAN.md` (297 to 311 capabilities), `plan.json` and the other 17 `src/*.rs` files take the new model and contract digests
- **Files:** `website/docs/reference/ess/` — cited, regenerated: `loom-bindings.md` is new, `index.mdx` gains the bindings domain graph import and the other 13 domain pages take the new digests
- **Files:** `website/data/ess/` — cited, regenerated: `loom-bindings.domain-graph.json` is new
- **Files:** `AGENTS.md` — cited, a new `## Bindings document` section (43 lines) on the crate, `decode`, `check`, `unbound`, the bounds and the refusals
- **Files:** `CHANGELOG.md` — cited, one `### Added` line for `b10x-loom-bindings`
- **Files:** `README.md` — cited, one crate row for `b10x-loom-bindings`
- **Files:** `Cargo.lock` — cited, 10 lines added for the new package
- **Symbols:** `b10x_loom_bindings::decode` — new, cited, `decode(text: &str) -> Decoded<BindingsDocument>`
- **Symbols:** `b10x_loom_bindings::check` — new, cited, `check(document, registration: &str, protocol) -> Result<(), Refusal>`
- **Symbols:** `b10x_loom_bindings::unbound` — new, cited, `unbound(document, protocol) -> Vec<String>`
- **Symbols:** `b10x_loom_bindings::Refusal` — new, cited, `pub struct Refusal(pub BindingsRefusal)`, with `Display`
- **Symbols:** `b10x_loom_bindings::{FORMAT, DEFAULT_AGENT_MODEL, DEFAULT_MAX_RUNS, MIN_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS, MAX_NESTING, MAX_OPEN_BRACKETS, MAX_NODES, MAX_DOCUMENT_BYTES}` — new, cited, public constants in `src/lib.rs`
- **Symbols:** `loom::bindings::{BindingsDocument, BoundAction, Executor, ExecutorSpec, EvidenceEntry, ResultSource, ResultRule, ActionBudget, ToolAllowList, RetryCondition, ArtifactPath, ProducerIndependence, BindingsRefusalReason, BindingsRefusal}` — new, cited, generated from `loom.bindings` (`ess/domains/bindings.yaml`)
- **Predicted and not touched:** none
