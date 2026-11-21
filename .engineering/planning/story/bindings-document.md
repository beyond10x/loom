---
format: aep.planning-md/3
id: story:bindings-document
kind: story
status: active
title: loom.bindings/1 declares per protocol action its executor, result table, tools, budget and timeout, checked at load
summary: The one binding format, grown from intake.bindings; document, decode and load-time checks, nothing executed
tags:
- sdc
relations:
- decomposes: epic:protocol-tree-on-bindings
- serves: vision:O1
scope:
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
- confidence: inferred
  path: website/data/status.json
- confidence: inferred
  path: website/docs/guides/bindings.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-20T23:55:44Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-11-21T00:12:43Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":5}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w1"}
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

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (new crate `b10x-loom-bindings`) — cited, Outcome and Acceptance name it
- **Files:** `crates/loom-bindings/` (`Cargo.toml`, `src/`, `tests/bindings_document.rs`) — cited, the Cargo.toml and test file are named in Acceptance and ESS first; the `src/` layout is inferred
- **Files:** `ess/domains/bindings.yaml` — cited, § ESS first
- **Files:** `ess/system.yaml` (add `loom.bindings` to `domains:`) — cited, § ESS first says "declared in `ess/system.yaml`"; the edit shape is read from `adcbf763` and `7021cd30`
- **Files:** `ess/ess-inputs.yaml` (add `domains/bindings.yaml` to `specification:`) — inferred, `adcbf763` and `7021cd30` each added one line there; the story does not name the file
- **Files:** `crates/loom-executor/tests/crate_names.rs` (`EXPECTED`, 28 to 29, add `b10x-loom-bindings` / `loom-bindings`) — inferred, `25b06ad5` and `f0acace9` each edited this guard when they added a crate; it reads `cargo metadata` and requires the exact package set (`crate_names.rs:1-12`, `:18`)
- **Files:** `crates/loom-bindings/Cargo.toml` names `b10x-canon` as `crates/loom-governor/Cargo.toml:15` does — cited, Acceptance bullet 4
- **Symbols:** `loom.bindings.{BindingsDocument, BoundAction, Executor, EvidenceEntry, ResultSource, ActionBudget, ToolAllowList, RetryCondition, BindingsRefusal}` — cited, § ESS first
- **Symbols:** `b10x_canon::ir::Ir::independence_dimensions`; `Action::may_produce` via `ProtocolEntry::model` (`crates/loom-protocols/src/lib.rs:17-23`) — inferred, read-only dependencies for the load-time checks (see `crates/loom-governor/src/lib.rs:1496-1503` and `crates/loom-intake-slice/src/bindings.rs:208-222`)
- **Also likely:** `crates/loom-protocols/` as a dependency of the new crate, read only — inferred, `ProtocolEntry` is where a compiled protocol's actions, artifacts and `may_produce` are reachable
- **Also likely:** `loom = { path = "../../generated/rust/loom" }` as a dependency of the new crate, as `crates/loom-resource/Cargo.toml` does — inferred, so the crate takes `loom.bindings` types from the generated model
- **Also likely:** `website/data/status.json` — inferred, hand-written status record; `25b06ad5` added an item for a new crate, the ESS-only commit `adcbf763` did not, and the story's acceptance does not mention it
- **Documents:** one guide page under `website/docs/guides/` with a complete `loom.bindings/1` file in a code block, decoded by `crates/loom-bindings/tests/bindings_document.rs` — cited that it exists; the file name `website/docs/guides/bindings.md` is inferred. The sidebar autogenerates `guides/` (`website/sidebars.ts`), so no sidebar edit.
- **Documents:** house-rule files, not listed — `CHANGELOG.md`, `AGENTS.md`, `README.md`, `Cargo.lock`, `generated/rust/loom/**`, `website/docs/reference/crates.md`, `website/docs/reference/ess/**`, `website/data/ess/loom-bindings.domain-graph.json` — cited as needing regeneration or hand edit by `f0acace9`
- **Confidence:** high for the crate, the ESS domain file and `ess/system.yaml` (the story names them and the wiring is read from two commits). Medium for the guide page name and the `status.json` edit.
- **Would collide with:** `crates/loom-bindings/**` (the four later stories extend it); `ess/domains/bindings.yaml`; the `domains:` list of `ess/system.yaml` and the `specification:` list of `ess/ess-inputs.yaml` (any unit adding an ESS domain); the `EXPECTED` array of `crates/loom-executor/tests/crate_names.rs` (any unit adding a crate); every manifest that names `b10x-canon` (`story:canon-0-9-0-pin` moves them with this one); `website/data/status.json`.
- **Safety fact:** adding the crate edits no root file. `Cargo.toml` has `members = ["crates/*"]`, so the crate joins the workspace by directory alone — `Cargo.toml:3`, cited and unproven. The one guard that fails without an edit is `crate_names.rs`, because it requires the exact package set (`crate_names.rs:18`, `:185`), so step 2, unproven. No other guard enumerates crates. `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` scans `crates/*/Cargo.toml` (`adversary_w8_pin.rs:19-48`, `:103`), so the new manifest passes it by naming the same tag, step 2, unproven.
