---
format: aep.planning-md/3
id: story:observe-reads-documents
kind: story
status: proposed
title: An observe action reads JSON and YAML documents exactly and validates one against a local schema
summary: json.read, yaml.read, document.get, document.validates; exact numbers, YAML floats and tags refused, no remote schema reference
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-executor-reads-workspace-files
- depends_on: story:gate-program-identity-bound-to-spawn
scope:
- confidence: cited
  path: crates/loom-bindings/Cargo.toml
- confidence: cited
  path: crates/loom-bindings/src/decode.rs
- confidence: inferred
  path: crates/loom-bindings/src/lib.rs
- confidence: cited
  path: crates/loom-bindings/src/load.rs
- confidence: cited
  path: crates/loom-bindings/src/observe.rs
- confidence: cited
  path: crates/loom-bindings/src/observe/documents.rs
- confidence: cited
  path: crates/loom-bindings/src/tree.rs
- confidence: cited
  path: crates/loom-bindings/tests/fixtures/schema-verdicts
- confidence: cited
  path: crates/loom-bindings/tests/observe_documents.rs
- confidence: cited
  path: crates/loom-bindings/tests/observe_files.rs
- confidence: cited
  path: ess/domains/bindings.yaml
- confidence: cited
  path: ess/domains/observation.yaml
- confidence: inferred
  path: website/docs/guides/bindings.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:54:58Z", actor: "agent:sdc", revision: 10, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

The `observe` executor reads documents in the workspace: `json.read` and `yaml.read` (`path`)
return the document's value; `document.get` (`path`, `pointer`, optional `format` `json` or `yaml`,
default `json`) returns the value at an RFC 6901 pointer, `null` when the pointer finds nothing;
`document.validates` (`path`, `schema`, both JSON) returns whether the document validates against
the schema. Each read goes through the `WorkspaceReader` of
`story:observe-executor-reads-workspace-files`, so the confinement, the 1 MiB per-file bound
Substrate sets, the refusal record and the "no process" rule of that story hold unchanged; this
story adds the parsing and its refusals. The value is the report's `value`, which a binding reads
by pointer (`from: /value/status`). `format` is checked at load when literal and at the step
otherwise. An absent `path`, and for `document.validates` an absent `schema`, is `Unavailable`, as
upstream reads both through one function (`files.rs:154-157`). JSON is parsed with `serde_json`
under its default 128-level recursion limit (what `story:json-depth-bound` showed holds); YAML is
first checked against the bounds `crates/loom-bindings/src/tree.rs` applies to a bindings document
(`MAX_NESTING` 64, `crates/loom-bindings/src/lib.rs:36`, and the others of `AGENTS.md` § Bindings
document), through a `pub(crate)` check `tree.rs` gains that names which bound was passed, then
parsed with `serde_yaml_ng`, floats and tags refused by `documents.rs` itself.

## Why

These are four of engineering-protocols' file operations (`crates/assertion-providers/src/files.rs:131-161`,
tag `0.16.0`). In the catalog (`catalog/engineering.yaml`) they back four observation functions,
`doc.string`, `doc.integer` and `doc.boolean` over `document.get` and `schema.valid` over
`document.validates`, and one recipe, `nodejs.package.named`, through `doc.string`. `json.read` and
`yaml.read` are operations of `assertion_providers::OPERATIONS` (`src/lib.rs:120-141`) that no
catalog function names; they are held by this story's tests and by the `OPERATIONS` list of
`observe_operations_cover_the_catalog` (`story:gate-run-collects-a-gates-document`).

Their guarantees are the requirement: JSON numbers keep their exact lexeme, YAML floating-point
values and tags are refused rather than rounded (`files.rs:80-106`), a schema may only reference
itself (`files.rs:170-192`), and HTTP or file resolution of a schema is off
(`crates/assertion-providers/README.md:40-41`); an absent document is unavailable, as any read I/O
error is (`src/lib.rs:157-159`). Loom already parses numbers exactly: `arbitrary_precision` is on in
this workspace (`AGENTS.md` § Connectors), and `jsonschema` 0.52.1 is in the lock through
`b10x-loom-resource` (`Cargo.lock:769`), which takes it with `default-features = false`
(`crates/loom-resource/Cargo.toml:15`), so no HTTP or file resolver is compiled in. Upstream uses
`jsonschema` 0.33.0. The build holds no second library, so a verdict that differs between the two
versions is found against recorded fixtures: `crates/loom-bindings/tests/fixtures/schema-verdicts/`
holds schema and document pairs, each with the verdict engineering-protocols `0.16.0` gave for it
(recorded once outside the build, the command and the tag written beside the list), and a pair
whose two verdicts differ names that in its `difference` member, so the difference is reported,
never silent.

## Waits on

- `story:observe-executor-reads-workspace-files`: the executor, the reader and the record path;
  both add variants to `loom.bindings.ObserveOperation` and arms to
  `crates/loom-bindings/src/observe.rs`.
- `story:gate-program-identity-bound-to-spawn` (edge added in this revision): both edit
  `ess/domains/bindings.yaml`, `generated/rust/loom/` and `CHANGELOG.md`. Through this edge and
  the chain, `story:report-formats-checked-before-the-table` is ordered after it on
  `crates/loom-bindings/src/program.rs`.

Waiting on this one: `story:observe-compares-generated-trees` (direct), and, through the chain,
`story:gates-document-evaluated-in-the-governor`, which also edits `Cargo.lock`.

## ESS first

`ess/domains/bindings.yaml`: `ObserveOperation` gains `JsonRead`, `YamlRead`, `DocumentGet`,
`DocumentValidates` (spelled `json.read`, `yaml.read`, `document.get`, `document.validates`).
`ess/domains/observation.yaml`: `ObserveRefusal` gains `DocumentMalformed`, `YamlFloat`, `YamlTag`,
`PointerInvalid`, `FormatInvalid`, `SchemaInvalid` and `SchemaReferenceRemote`;
`BindingsRefusalReason` is not changed (a literal pointer or `format` that is invalid is refused at
load as `InvalidObserveArguments`, naming the member). On that commit `task drift` fails and `crates/loom-bindings/tests/observe_documents.rs`
does not compile.

## Evidence record

As `story:observe-executor-reads-workspace-files` § Evidence record, unchanged: `format`, `id`
(fresh), `kind`, `subject`, `producer` from the entry, `result` from the table over the pointer's
value, `subject_revision` and `upstream_revisions` from the case, `observed_at` from one host clock
reading at the end. Two runs over the same bytes write equal records but for `id` and
`observed_at`; the value of `json.read` is the parsed document, so two files with the same value
and different bytes give equal reports while their `ObservedRead` digests differ.

## Acceptance

Tests in `crates/loom-bindings/tests/observe_documents.rs`, on a fake `WorkspaceReader` holding
fixture bytes (the reader's Substrate cases are story 1's). "Refused X" below means:
`ObserveRun.outcome` `Refused`, `ObserveRun.refusal` X, the step refused naming it, and no evidence
record. A table refusal (`ResultNotInTable`) is the step's refusal of
`story:bound-program-result-table`, with `ObserveRun.outcome` unchanged (the epic's § Refusals).

- `document.get` with `/a/b` on `{"a":{"b":1.10}}` reads the value whose JSON text is `1.10`: the
  record's `result` is the row `"1.10"`.
- The same with a table holding only `"1.1"` and no `*`: the step's table refusal
  `ResultNotInTable`, and `ObserveRun.outcome` `Known`.
- `document.get` with a pointer the document lacks: `ObserveRun.outcome` `Known` and the record's
  `result` is the row `null` (story 1's rule: `null` is looked up by its JSON text).
- `document.get` with `format: yaml` and `/x` on `x: 15`: the row `15`.
- A literal `format` other than `json` or `yaml`: refused at load, `BindingsRefusalReason`
  `InvalidObserveArguments`, naming `format`; the same arriving from a request's arguments: refused
  `FormatInvalid` (upstream's "document format must be json or yaml", `files.rs:75-76`).
- A literal pointer that is not empty and does not start with `/`: refused at load,
  `InvalidObserveArguments`, naming `pointer`; the same arriving from a request's arguments: refused
  `PointerInvalid`.
- `yaml.read` of `x: 1.5`: refused `YamlFloat`.
- `yaml.read` of `x: !tag 1`: refused `YamlTag`.
- `yaml.read` of `x: 15`: `ObserveRun.outcome` `Known`, and `from: /value/x` reads the row `15`.
- `json.read` of bytes that are not JSON, and `yaml.read` of bytes that are not YAML: refused
  `DocumentMalformed`, `ObserveRun.reason` naming the path and the parser's position; no record with
  a `null` or `false` row exists.
- `json.read`, `yaml.read`, `document.get` and `document.validates` on an absent `path`:
  `ObserveRun.outcome` `Unavailable`, `ObserveRun.code` `resource.not-found`, and the table's
  `unavailable` row.
- `document.validates` with an absent `schema`: `ObserveRun.outcome` `Unavailable`, `code`
  `resource.not-found`.
- A YAML document nested 65 deep: refused `DocumentMalformed`, `reason` naming `MAX_NESTING` (64);
  64 deep reads.
- A JSON document nested 129 deep: refused `DocumentMalformed`, `reason` naming the 128-level
  recursion limit; 128 deep reads.
- `document.validates` is `true` for a valid document and `false` for an invalid one (the record's
  `result` is the row for each).
- A schema with a `$ref` or `$dynamicRef` not starting with `#`: refused `SchemaReferenceRemote`; a
  test gives a schema whose only `$ref` is `file:///etc/passwd` and the fake reader counts no read
  of it.
- A schema `jsonschema` cannot compile: refused `SchemaInvalid`.
- `schema_verdicts_match_upstream_or_name_the_difference`: for each pair under
  `tests/fixtures/schema-verdicts/`, `document.validates` gives the recorded upstream verdict, or
  the pair's `difference` member names the difference; a pair whose verdict differs with no
  `difference` fails the test, naming the pair and both verdicts.
- A document of 1 MiB + 1 byte: refused `FileOverBound` (story 1's).
- `no_file_or_document_operation_starts_a_process` (story 1) covers the four operations: its
  counting runner counts no launch for each.
- Two runs over the same file compare equal but for `id` and `observed_at`.
- `CHANGELOG.md` `### Added`: the four operations. `### Changed`: `**Breaking:**` for the new
  `ObserveOperation` and `ObserveRefusal` variants.
- `task check` exits 0.

## Files

`ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `generated/rust/loom/`,
`crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/documents.rs` (new),
`crates/loom-bindings/src/decode.rs` (the four spellings), `crates/loom-bindings/src/load.rs` (the
argument sets: `path`; `path`, `pointer`, optional `format`; `path`, `schema`; the literal
`pointer` and `format` checks), `crates/loom-bindings/src/tree.rs` (the `pub(crate)` bound check
naming the bound passed), `crates/loom-bindings/Cargo.toml` (`jsonschema = { version = "0.52.1",
default-features = false }`, as `crates/loom-resource/Cargo.toml:15`; `serde_json` is story 1's;
`Cargo.lock` gains `jsonschema 0.52.1` in `b10x-loom-bindings`' dependency list and no package),
`crates/loom-bindings/tests/observe_documents.rs` (new),
`crates/loom-bindings/tests/fixtures/schema-verdicts/` (new), `crates/loom-bindings/tests/observe_files.rs`
(the no-process test's operation list), `Cargo.lock`, `CHANGELOG.md`.

## Out of scope

YAML documents with exact decimals (refused, as upstream). Schemas spread over several files.
Report formats (`story:report-formats-checked-before-the-table`).

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-bindings` (observe executor, documents module) and `ess/domains/bindings.yaml` / `ess/domains/observation.yaml` — cited (story § Files, § ESS first)
- **Files:** `ess/domains/bindings.yaml`, `ess/domains/observation.yaml`, `crates/loom-bindings/src/observe.rs`, `crates/loom-bindings/src/observe/documents.rs` (new), `crates/loom-bindings/Cargo.toml`, `crates/loom-bindings/tests/observe_documents.rs` (new), `crates/loom-bindings/tests/observe_files.rs` — cited (story § Files). None of the observe files exists in this tree: `crates/loom-bindings/src` holds `decode.rs lib.rs load.rs tree.rs`, `ess/domains` has no `observation.yaml`, and no `ObserveOperation` exists. They arrive with `story:observe-executor-reads-workspace-files`.
- **Symbols:** `loom.bindings.ObserveOperation` (+`JsonRead`, `YamlRead`, `DocumentGet`, `DocumentValidates`), `loom.observation.ObserveRefusal` (+6 variants), `WorkspaceReader`, `ObserveRun`, `jsonschema::validator_for` — cited (story). All except `jsonschema::validator_for` are declared by story 1, not yet in the tree. `jsonschema::validator_for` is at `jsonschema-0.52.1/src/lib.rs:1428`, and Loom already calls it in `crates/loom-resource/tests/crd.rs:718` (tests only).
- **Also likely:** `crates/loom-bindings/src/decode.rs`, `crates/loom-bindings/src/load.rs` — inferred. Story 1 puts per-operation argument checks (`InvalidObserveArguments`) there, and this story needs the four operations' argument sets and the literal-pointer check at load.
- **Also likely:** `crates/loom-bindings/src/tree.rs` — inferred. The YAML bounds live there (`MAX_NESTING` etc.; `parse` is `pub(crate)` at `tree.rs:75`). `parse` returns a `Node` and a `String` error. It already refuses tags, `<<` and duplicate keys, with no typed `YamlTag`/`YamlFloat` distinction (`Node::Real` at `tree.rs:25`). Reuse needs a change here, or a second parse path in `documents.rs`.
- **Also likely:** `crates/loom-bindings/src/lib.rs` (`mod` line, constants `MAX_*` at `lib.rs:36-44`), `website/docs/guides/bindings.md` (story 1 edits it for the operations) — inferred.
- **Documents:** `CHANGELOG.md` (`### Added`, `### Changed` **Breaking**); `AGENTS.md` § Bindings document is likely, as story 1 edits it — cited (acceptance) / inferred (AGENTS)
- **Confidence:** high for the surface (story names every file, `loom-bindings` is one crate); medium for the file list, because decode/load/tree are missing from the story's Files (see defects).
- **Would collide with:**
  - Ordered before it by `depends_on`: `story:observe-executor-reads-workspace-files` (direct; `observe.rs`, `observe_files.rs`, both ESS files, `decode.rs`, `load.rs`, `lib.rs`), `story:gate-program-identity-bound-to-spawn` (direct; `bindings.yaml`, `Cargo.lock`), and by chain `story:bound-program-result-table`, `story:canon-0-9-0-pin` (`crates/loom-bindings/Cargo.toml`, `Cargo.lock`), `story:engineering-protocols-dependency-removed` (the `b10x-assertion-providers` entry in `Cargo.lock`).
  - Ordered after it by `depends_on`: `story:observe-compares-generated-trees` (direct; `observe.rs`, `observation.yaml`, `bindings.yaml`, `observe_files.rs`, `tree.rs`), and by chain `story:report-formats-checked-before-the-table` (`observe.rs`, `program.rs`), `story:observe-git-changed`, `story:gate-run-collects-a-gates-document`, `story:gates-document-evaluated-in-the-governor`, `story:tcp-reachability-through-an-aperture`, `story:dns-resolution-through-a-resolver-aperture` (all name `observe.rs`/`observation.yaml`).
  - **No `depends_on` path either way**, and each names `ess/domains/bindings.yaml`: `story:bound-agent-node` (also `crates/loom-bindings/Cargo.toml`), `story:bound-artifact-revisions`, `story:record-copy-between-cases`, `story:child-case-own-workspace`, `story:run-caps-per-arrival`, `story:capacity-given-to-an-admission-program`, `story:concurrent-cases-in-the-tree-daemon`, `story:limit-hits-in-run-log`, `story:tree-usage-and-model-budget`, `story:action-limits-enforced`, `story:confinement-level-required`, `story:named-egress-per-action`, `story:engine-configuration-document`, `story:agent-node-run-records`, `story:agent-session-through-substrate`, `story:bindings-child-path-refusal-names-the-bindings-directory`. All are marked cited or inferred in their own bodies, not by me. Of these, `bound-agent-node` and `bindings-child-path-refusal-names-the-bindings-directory` / `child-case-runtime` / `run-caps-per-arrival` / `child-case-own-workspace` / `record-copy-between-cases` also name `decode.rs`, `load.rs` or `lib.rs`. `story:autonomy-grant-table` only reads `loom-bindings/Cargo.toml:17`. — cited (graph and store grep)
  - `story:json-depth-bound` is `implemented`, so it is not a collision.
- **Contradicted by the code:** see Story defects 1-3 (all cited).
- **Safety fact:** `jsonschema` must be added with `default-features = false`, as `crates/loom-resource/Cargo.toml:15` has it. The 0.52.1 default features are `resolve-http`, `resolve-file`, `tls-aws-lc-rs`, `idna` (`jsonschema-0.52.1/Cargo.toml` `[features]`). The lock's 0.52.1 entry (`Cargo.lock:3412`) lists no `reqwest`/`rustls`, so a default-features add would also rewrite the lock. Step 2, unproven.
