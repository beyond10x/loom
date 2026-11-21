---
format: aep.planning-md/3
id: story:gates-document-evaluated-in-the-governor
kind: story
status: draft
title: The governor plans and evaluates an engineering-gates/1 document with Canon's expression crate against the vendored catalog
summary: loom_governor::gates::{plan, evaluate}; catalogs/engineering.yaml held to its sha256; not_applicable never pass; only the governor names canon_expr
tags:
- sdc
relations:
- decomposes: epic:observation-collection-as-executors
- serves: vision:O1
- depends_on: story:observe-executor-reads-workspace-files
- depends_on: story:engineering-protocols-dependency-removed
- depends_on: story:observe-git-changed
- depends_on: story:engineering-protocols-vendor-command
scope:
- confidence: cited
  path: catalogs/engineering.yaml
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/gates.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/canon_expr_only_in_governor.rs
- confidence: cited
  path: crates/loom-governor/tests/fixtures/applies-when.fixture.yaml
- confidence: cited
  path: crates/loom-governor/tests/gates.rs
- confidence: cited
  path: crates/loom-xtask/src/protocols_vendor.rs
- confidence: cited
  path: crates/loom-xtask/tests/protocols_vendor.rs
- confidence: cited
  path: ess/domains/gates.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
revision: 5
---
## Outcome

`b10x-loom-governor` plans and evaluates an `engineering-gates/1` document, and is the only crate
that does:

- `loom_governor::gates::plan(document, variables, identity) -> Result<Plan, GatesRefusal>` takes
  the document's bytes, decodes them by hand in `gates.rs` with `serde_yaml_ng` (the generated
  `loom` crate has no serde derives) into `loom.gates.GatesDocument` (format `engineering-gates/1`,
  language `canon-expr/1`, at least one gate, unique non-empty ids, readable text, at least one
  assertion, a non-empty `applies_when` when present), and compiles every `applies_when` and
  assertion with `canon_expr::compile` against the vendored engineering catalog. `identity`
  (`GatesIdentity`: `source_identity`, `context_identity`, both non-empty, as Canon's
  `validate_bindings` requires, `crates/canon-expr/src/compiler.rs:696-697` at `0.9.0`) becomes the
  plan's `Bindings.source_identity` and `context_identity`. The plan's requests come out in Canon's
  order: by request id, one per distinct operation and arguments (`compiler.rs:729`, `:735`, `:799`).
- `loom_governor::gates::evaluate(plan, observations, context) -> Result<GatesReport, GatesRefusal>`
  evaluates the plan over the collected observations with `canon_expr::evaluate`, where `context`
  is Canon's `EvalContext` (`now`, `source_identity`, `context_identity`; `model.rs:398-402`), `now`
  the trusted time the host gives. It returns each gate's result and the aggregate: a gate whose
  `applies_when` is false is `not_applicable`, one whose `applies_when` is unknown is `unknown`,
  otherwise `fail` when any assertion is false, `pass` when all are true, else `unknown`; the
  aggregate is `fail` if any gate fails, else `unknown` if any is unknown, else `pass` if any
  passes, else `not_applicable`. An error Canon's `evaluate` returns (a modified plan, too many or
  duplicate or unrequested observations, a value of the wrong type, `valid_until` before
  `observed_at`; `eval.rs:34-90`) is `GatesRefusal` `EvaluationFailed`, with Canon's code and
  message.
- `loom_governor::gates` re-exports the Canon types its callers hand it (`Plan`, `Observation`,
  `Outcome`, `Value`, `EvalContext`), so `story:gate-run-collects-a-gates-document` builds
  observations as `loom_governor::gates::Observation` and never names `canon_expr`.

The gates evaluation runs in `crates/loom-governor`, but it is called from the acting path: an
observe action (`story:gate-run-collects-a-gates-document`) calls `plan` and `evaluate` as plain
functions, and their result reaches the governor's protocol evaluation only as evidence the
binding's table writes. Whether that meets "the only crate that evaluates protocols with Canon" is
§ Open 7 of the ledger, for session `canon`; the default is this placement.

The catalog is engineering-protocols' `catalog/engineering.yaml` (`canon-catalog/1`, 26 functions:
20 observation functions over 18 operations, `document.get` behind three of them, and 6 recipes),
a JSON document under a `.yaml` name, copied byte for byte to `catalogs/engineering.yaml` at tag
`0.16.0`, sha256 `d86dca5d0673a0f464433b31b41523e0c2517c394aba5a433d156136d4ec2856` (the same at
`0.15.2`). The governor embeds it, decodes it as JSON with `serde_json` into Canon's `Catalog`
(checked by `canon_expr::validate_catalog`), and holds it to that digest: `gates::Catalog::from_bytes`
refuses other bytes `CatalogMismatch`. `loom-xtask protocols-vendor --checkout <dir> --tag <tag>`
(`crates/loom-xtask/src/protocols_vendor.rs`, created by
`story:engineering-protocols-vendor-command`) also writes `catalogs/engineering.yaml` from the
checkout's `catalog/engineering.yaml` at that tag, byte for byte; at a tag that holds no catalog it
leaves `catalogs/engineering.yaml` as it is.

## Why

The gate evaluator Loom would otherwise run is engineering-protocols'
(`crates/canon-engineering-assertions/src/lib.rs:209-250`, `:717-846`, tag `0.16.0`), outside any
governor; takt calls it from its gate validator (takt `25c62df`, `crates/takt/src/validator.rs:5264`,
`:5487-5545`, read by a helper). `AGENTS.md` § Governor makes the governor the only
crate that evaluates protocols with Canon and keeps clocks, network and models out of an
evaluation; a gate plan and its evaluation are Canon's (`b10x-canon-expr`, Canon `0.9.0`:
`crates/canon-expr/src/compiler.rs:624`, `crates/canon-expr/src/eval.rs:34`), so they live there.
Canon is named by one reference in every manifest that adds it (`AGENTS.md` § Governor),
so `b10x-canon-expr` takes the governor's `b10x-canon` reference.

Canon's evaluation turns an observation it cannot use into an unknown truth with a named reason
(`eval.rs:91-96`): `<request id>: plan mismatch`, `<request id>: observed in future`,
`<request id>: expired`; a source or context identity that differs from the plan's makes every
assertion `source or context identity mismatch` (`:107-116`). Loom passes each observation through
unchanged, so these reasons are what a test can see.

## Waits on

- `story:observe-git-changed` (edge added in this revision): both edit `generated/rust/loom/`
  (`plan.json`, `PLAN.md`, the module list in `src/lib.rs`) and `CHANGELOG.md`; through it this
  story is ordered after stories 1 to 5 and 2, which share the same two, and after
  `story:observe-reads-documents`, which also edits `Cargo.lock`.
- `story:observe-executor-reads-workspace-files`: both add a domain to `ess/system.yaml` and
  `ess/ess-inputs.yaml`.
- `story:engineering-protocols-vendor-command` (edge added in this revision): this story edits that
  story's `crates/loom-xtask/src/protocols_vendor.rs` and `crates/loom-xtask/tests/protocols_vendor.rs`.
- `story:engineering-protocols-dependency-removed`: Canon `0.9.0` is in place, and no
  engineering-protocols crate brings a second `b10x-canon-expr`.

Waiting on this one: `story:gate-run-collects-a-gates-document` (`generated/rust/loom/`,
`CHANGELOG.md`, and the functions it calls).

## ESS first

New domain `loom.gates` in `ess/domains/gates.yaml`, listed in `ess/system.yaml` and
`ess/ess-inputs.yaml`: `GatesDocument` (`format`, `language`, `gates`), `Gate` (`id`, `text`,
`applies_when` optional, `assertions`), `GateResult` (`Pass`, `Fail`, `Unknown`, `NotApplicable`),
`GateOutcome` (`id`, `result`, `reasons`), `GatesReport` (`plan_id`, `result`, `gates`), and
`GatesRefusal` with reasons `UnsupportedFormat`, `UnsupportedLanguage`, `EmptyDocument`,
`InvalidGate`, `CompileFailed`, `CatalogMismatch`, `EvaluationFailed`, and `GatesIdentity`
(`source_identity`, `context_identity`). Canon's plan and observation types stay
Canon's. On that commit `task drift` fails and `crates/loom-governor/tests/gates.rs` does not
compile.

## Evidence record

None: this story writes no record. It returns a `GatesReport`, which
`story:gate-run-collects-a-gates-document` turns into records through the binding's table.
`evaluate` is a function of the plan, the observations and `now`: the same three give the same
report bytes (a test evaluates twice and compares).

## Acceptance

Tests in `crates/loom-governor/tests/gates.rs`, with hand-written observations and no collector,
unless named otherwise. Each reads the `GatesReport` returned or the `GatesRefusal`.

- `not_applicable_is_never_pass`: a document whose only gate's `applies_when` is false gives
  `GatesReport.gates[0].result` `NotApplicable` and `GatesReport.result` `NotApplicable`
  (engineering-protocols' `gates_result_values.rs` case of that name).
- `applies_when_cases_keep_their_results`: each case of `fixtures/assertions/applies-when.fixture.yaml`
  (copied to `crates/loom-governor/tests/fixtures/`, its sha256 named in the test), with the
  `story.has_tag` and `git.changed` observations its case implies written by hand, gives the
  fixture's `expect.result` and per-gate results; an unknown `applies_when` gives `Unknown` with
  `applies_when`'s reasons in `GateOutcome.reasons`.
- One false assertion: `GateOutcome.result` `Fail`.
- An `Unavailable` observation for a gate's only assertion: `GateOutcome.result` `Unknown`.
- The aggregate order over the four results: `GatesReport.result` for each pair (one test per
  pair).
- An observation whose `valid_until` is before `now` (and not before its `observed_at`, which Canon
  refuses): `GateOutcome.result` `Unknown`, and `GateOutcome.reasons` holds `<request id>: expired`
  (Canon `0.9.0`, `eval.rs:95-96`).
- An observation whose plan id differs from the plan's: `Unknown`, reason `<request id>: plan
  mismatch` (`eval.rs:91-92`).
- An observation whose `observed_at` is after `now` (its `valid_until` later still): `Unknown`,
  reason `<request id>: observed in future` (`eval.rs:93-94`).
- An `EvalContext` whose `source_identity` or `context_identity` differs from the `GatesIdentity`
  the plan was made with: `Unknown`, reason `source or context identity mismatch` (`eval.rs:115`).
- Two observations for one request: `GatesRefusal` `EvaluationFailed`, carrying Canon's code.
- An observation whose `valid_until` precedes its `observed_at`: `GatesRefusal` `EvaluationFailed`,
  carrying Canon's `evidence-time`.
- A `GatesIdentity` with an empty `source_identity`: `plan` refuses `CompileFailed` with Canon's
  message.
- `requests_come_in_canons_order`: a document whose assertions name `fs.file.exists("b")`, then
  `fs.file.exists("a")`, then `fs.file.exists("b")` again gives two requests, ordered by request
  id.
- `the_vendored_catalog_is_the_released_one`: `catalogs/engineering.yaml` has the sha256 above and
  decodes as JSON to 26 functions.
- `a_changed_catalog_is_refused`: `gates::Catalog::from_bytes` over the catalog with one byte
  changed returns `GatesRefusal` `CatalogMismatch`.
- `the_vendor_command_writes_the_catalog` (in `crates/loom-xtask/tests/protocols_vendor.rs`): the
  test's Git repository gains a tag `t4` holding the `t1` protocol files and a
  `catalog/engineering.yaml`; after `protocols-vendor --tag t4`, `catalogs/engineering.yaml` holds
  the tag's bytes, and a second run writes the same bytes.
- `a_tag_without_a_catalog_leaves_it`: after `protocols-vendor --tag t1` (the vendor-command
  story's tags `t1` to `t3` hold no catalog), `catalogs/engineering.yaml` is byte for byte what it
  was, and the existing `t1` to `t3` cases pass unchanged.
- A document with another format: `GatesRefusal` `UnsupportedFormat`; another language:
  `UnsupportedLanguage`; no gates: `EmptyDocument`.
- A duplicate or empty id, empty text, no assertions or an empty `applies_when`: `GatesRefusal`
  `InvalidGate`, naming the gate.
- An expression `canon_expr::compile` refuses: `GatesRefusal` `CompileFailed` with Canon's message.
- `canon_expr_is_the_governors_alone` (`crates/loom-governor/tests/canon_expr_only_in_governor.rs`,
  new): it reads `cargo metadata --no-deps --format-version 1` as
  `crates/loom-executor/tests/crate_names.rs:3` does and asserts the workspace packages with a
  dependency named `b10x-canon-expr` are exactly `b10x-loom-governor`; then it scans every
  `crates/*/src` file for `canon_expr` and asserts at least one match and every match under
  `crates/loom-governor`. On today's tree both fail: no package depends on `b10x-canon-expr`, and
  no file names `canon_expr`.
- `crates/loom-governor` gains no clock, network or model call: `evaluate` takes `now` in its
  `EvalContext` (the determinism test above), and the dependency guard of `task check` passes with
  `b10x-canon-expr` added by the governor's Canon reference (one `b10x-canon-expr` in `Cargo.lock`).
- `crates/loom-commission/Cargo.toml` is unchanged (`AGENTS.md` § Never).
- `CHANGELOG.md` `### Added`: `loom_governor::gates::{plan, evaluate}` and the vendored catalog.
  No `**Breaking:**` bullet.
- `task check` exits 0.

Carried elsewhere: "every existing gates fixture keeps its result" (engineering-protocols'
`a-gate-sees-the-change-and-may-not-apply`, acceptance 4) needs collection for
`fixtures/assertions/gates.yaml`, `git-changed.fixture.yaml` and `program-by-name.fixture.yaml`,
so `story:gate-run-collects-a-gates-document` carries it; this story carries the evaluation half of
`applies-when.fixture.yaml` above.

## Files

`ess/domains/gates.yaml` (new), `ess/system.yaml`, `ess/ess-inputs.yaml`, `generated/rust/loom/`,
`catalogs/engineering.yaml` (new), `crates/loom-governor/Cargo.toml` (`b10x-canon-expr` beside the
governor's `b10x-canon` reference, at the same tag; `serde_json = "1"`, already in `Cargo.lock`),
`crates/loom-governor/src/gates.rs`
(new), `crates/loom-governor/src/lib.rs` (module line), `crates/loom-governor/tests/gates.rs` (new),
`crates/loom-governor/tests/canon_expr_only_in_governor.rs` (new),
`crates/loom-governor/tests/fixtures/applies-when.fixture.yaml` (copied),
`crates/loom-xtask/src/protocols_vendor.rs` (the catalog copy),
`crates/loom-xtask/tests/protocols_vendor.rs` (the catalog case), `Cargo.lock`, `CHANGELOG.md`,
`AGENTS.md` § Governor.

## Out of scope

Collecting the observations and writing records (`story:gate-run-collects-a-gates-document`).
Extension catalogs and external providers (refused by that story). A replay command.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (new `src/gates.rs`, new tests, `Cargo.toml`, one module line in `src/lib.rs`) — cited (story § Files, § Outcome)
- **Files:** `crates/loom-governor/src/gates.rs` (new), `crates/loom-governor/src/lib.rs` (module line), `crates/loom-governor/Cargo.toml` (adds `b10x-canon-expr`; the Canon line is `Cargo.toml:15`), `crates/loom-governor/tests/gates.rs` (new), `crates/loom-governor/tests/canon_expr_only_in_governor.rs` (new), `crates/loom-governor/tests/fixtures/applies-when.fixture.yaml` (copied) — cited
- **Files (vendoring):** `catalogs/engineering.yaml` (new, a top-level directory that does not exist), `crates/loom-xtask/src/protocols_vendor.rs` and `crates/loom-xtask/tests/protocols_vendor.rs` (both created by `story:engineering-protocols-vendor-command`; neither is in the tree) — cited
- **Files (ESS):** `ess/domains/gates.yaml` (new), `ess/system.yaml`, `ess/ess-inputs.yaml`, `generated/rust/loom/` — cited
- **Symbols:** `loom_governor::gates::{plan, evaluate}`, `GatesDocument`, `Gate`, `GateResult`, `GateOutcome`, `GatesReport`, `GatesRefusal`, `canon_expr::{compile, evaluate}`, `Catalog`, `Bindings`, `EvalContext`, `Observation` — cited. Canon `0.9.0` has `compile` at `compiler.rs:624` and `evaluate` at `eval.rs:34`.
- **Documents:** `AGENTS.md` § Governor, `CHANGELOG.md` `### Added` — cited. `AGENTS.md:441` ("only crate that evaluates protocols with Canon"), `:523-526` (Canon reference) and `:2293-2294` (Commission never depends on Canon) all match the story at committed `HEAD`.
- **Also likely:** none beyond the story's Files list; nothing else in the tree names `canon_expr` — inferred (`git grep` finds `canon_expr` only in `Cargo.lock:232,242`)
- **Confidence:** high — every file is named, and every Canon and engineering-protocols line the story cites says what the story says (`eval.rs:34,91-96,107-116,115`, `compiler.rs:624`, catalog sha256, 26 functions, `lib.rs:209`/`:717-846` at `0.16.0`)
- **Would collide with** (ordering from `depends_on`, read with `aep plan artifact graph --format json`):
  - `story:engineering-protocols-dependency-removed` on `crates/loom-governor/Cargo.toml`, `Cargo.lock` and `AGENTS.md` § Governor — cited. Ordered: direct `depends_on`.
  - `story:engineering-registry-calls-replaced` on `crates/loom-governor/src/lib.rs` (it rewrites the doc comments at `:129-133`) — cited. Ordered: path gates -> dependency-removed -> registry-calls-replaced.
  - `story:engineering-protocols-vendor-command` on `protocols_vendor.rs`, `tests/protocols_vendor.rs` and `Cargo.lock` — cited. Ordered: direct.
  - `story:canon-0-9-0-pin` (active) on `crates/loom-governor/Cargo.toml` and `src/lib.rs` — cited. Ordered: transitive (gates -> vendor-command -> engineering-protocol-lock -> canon-0-9-0-pin).
  - `story:bound-program-result-table` (names `loom-governor/src/lib.rs` in its body) and `story:observe-executor-reads-workspace-files` (adds an ESS domain) — cited. Ordered: both are in this story's `depends_on` closure.
  - `story:gate-run-collects-a-gates-document` on `generated/rust/loom/`, `CHANGELOG.md`, `crates/loom-governor/Cargo.toml` and `catalogs/engineering.yaml` — cited. Ordered: it `depends_on` this story, so it comes after.
  - UNORDERED on `crates/loom-governor/src/lib.rs` (one `mod gates;` line): `story:child-case-own-workspace`, `story:bound-artifact-revisions`, `story:record-copy-between-cases`, `story:child-case-runtime`, `story:standing-tree-rests-quiescent`, `story:hash-chained-run-log`, `story:decided-record-written-then-recorded`, `story:grounds-resolve-against-held-cases`, `story:question-and-outcome-decisions-hold-runs`, `story:decision-request-file-store` — cited from their store scope. No `depends_on` path reaches this story; a merge-only conflict on a module list.
  - UNORDERED on `crates/loom-governor/Cargo.toml`: `story:decision-request-file-store` — cited.
  - UNORDERED on `crates/loom-governor/tests/fixtures`: `story:child-case-runtime` lists the directory, which holds every path under it — cited.
  - UNORDERED on `ess/system.yaml` and `ess/ess-inputs.yaml` (domain list lines): `story:autonomy-grant-table`, `story:tree-run-command`, `story:decision-request-entity`, `story:hash-chained-run-log`; `story:ess-format-31` on `system.yaml` only — cited.
  - UNORDERED on `ess/ess-inputs.yaml` only: `story:send-input-in-the-declared-shape`, `story:agent-selects-the-connectors-relay`, `story:boundary-admits-declared-http-egress` — cited.
  - Stories that touch `generated/rust/loom/` or `Cargo.lock` were not enumerated; both are house-rule files — inferred.
- **Safety fact:** `canon_expr` is named in no `crates/*/src` file today and no workspace package depends on `b10x-canon-expr`, so adding it to the governor changes no existing caller — step 2 (`git grep -n 'canon_expr'` finds `Cargo.lock:232,242` only), unproven
- **Safety fact:** `crates/loom-governor/src` has no `SystemTime` or `Instant::now`, and `evaluate` takes `now` as an argument, so the "no clock" rule holds — step 2, unproven
