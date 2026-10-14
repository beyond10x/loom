---
format: aep.planning-md/3
id: story:co-pinned-consumer-check
kind: story
status: draft
title: A consumer pinning Canon and llm beside the SDK finds their references on the stability page and resolves one copy in task check
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- depends_on: story:release-names-embedder-breaks
revision: 1
---
## Outcome

A consumer that names Canon or llm beside `b10x-loom-sdk`, as control-plane does, finds on the SDK
stability page the exact reference each must be pinned at, and Loom's CI proves that a crate pinning
them so resolves one copy of each and passes Canon and llm values across `loom_sdk` without a type
mismatch. A Loom release that moves one of those references changes a checked file, so the move is
seen before a consumer's build breaks on it.

## Why

The epic's first acceptance asks that a consumer's build resolve one copy of each Loom crate it
uses. `story:pinned-consumer-check` holds that for a crate depending on `b10x-loom-sdk` alone. A real
consumer names more: control-plane, the integration point of the engineering factory (consumer C,
served first), depends on `b10x-loom-sdk` by tag and also names `b10x-canon` (tag `0.4.0`) and
`b10x-llm-core` and `b10x-llm-tool-call` (tag `0.4.0`) itself. Their types cross the SDK surface
(`CanonGovernor::with_protocol` takes a `b10x_canon::model::Protocol`; the slice's `ModelSelector::new`
takes llm's `&dyn Model`), so a consumer pinning another reference gets a second copy whose types do
not match Loom's. Today the reference a consumer must match is written only in Loom's own manifests
and in AGENTS.md, and the SDK stability page does not name it.

## Acceptance

`task consumer-check` builds a second external crate that pins the SDK by path and Canon and llm at
the references the SDK stability page names, and fails when the page, the workspace's manifests and
that crate disagree or when it resolves a second copy:

- `website/docs/reference/sdk-stability.md` gains a "Pin together" table: each package outside Loom
  whose types appear on the stable surface (at least `b10x-canon`, `b10x-llm-core`), its git URL and
  the reference Loom pins it at, with the statement that a consumer naming one of them pins it at
  exactly that reference and that a release moving one names it under `Breaking:`.
- `consumers/co-pinned/` is its own workspace depending on `b10x-loom-sdk` by path and on each
  package of the table by its git URL and reference, and its `src/main.rs` parses a protocol with its
  own `b10x_canon` and hands it to `loom_sdk::CanonGovernor::with_protocol`, and hands a
  `&dyn` llm `Model` of its own to `loom_sdk::intake::slice::selector::ModelSelector::new`; it compiles only when the types
  are one copy.
- `crates/loom-xtask/tests/co_pinned_consumer.rs::the_pin_together_table_matches_the_workspace_references`
  reads the page's table and the git references of those packages in the workspace manifests
  (`crates/*/Cargo.toml`) and asserts they are equal.
- `crates/loom-xtask/tests/co_pinned_consumer.rs::the_co_pinned_consumer_pins_what_the_table_names`
  asserts `consumers/co-pinned/Cargo.toml` names exactly the SDK by path and the table's packages at
  the table's references, with no `[patch]` section.
- `crates/loom-xtask/tests/co_pinned_consumer.rs::the_co_pinned_consumer_resolves_one_copy_of_each_shared_crate`
  runs `cargo tree --manifest-path consumers/co-pinned/Cargo.toml --offline --duplicates` and asserts
  no `b10x-loom-*`, `loom-*`, `b10x-canon*` or `b10x-llm-*` package is listed.
- With `b10x-canon`'s tag in `consumers/co-pinned/Cargo.toml` changed to another reference (a local,
  uncommitted edit), `task consumer-check` exits non-zero; the run is recorded in the pull request.
  On the unchanged tree `task check` exits 0.
- `release-check`'s set of stable-surface files (`story:release-names-embedder-breaks`) includes
  `consumers/co-pinned/`, and
  `crates/loom-xtask/tests/release_check.rs::release_check_refuses_breaking_none_when_a_shared_pin_moved_since_the_previous_tag`
  passes over a fixture repository whose two tags differ only in that crate's manifest.

## ESS first

No behaviour change: a documentation table, an out-of-workspace test crate and checks. No runtime
type, command, port or outcome changes, so `ess/` is untouched. The red step is
`the_pin_together_table_matches_the_workspace_references`, which fails before the table exists.

## Domain relations

None. A consumer, a pin and a shared package are not nouns of any `ess/1` document in this
repository, and the story introduces none.

## Depends on

`story:release-names-embedder-breaks`, whose surface-file set the last acceptance line extends.

## Not in scope

- Moving any shared reference (Canon, llm, Connectors, Mandate, Substrate); the story records the
  ones Loom pins.
- Packages whose types do not appear on the stable surface of `sdk-stability.md` (Connectors,
  Mandate, Substrate, ESS); a later surface change that exposes one adds it to the table.
- `consumers/pinned/` and its tests (`story:pinned-consumer-check`), which stay SDK-only.
- Any change inside control-plane or another consumer's repository.
- The agent platform (consumer A), which waits until the engineering factory is served.

## Scope

Crates: `loom-xtask` (tests), the new out-of-workspace crate `consumers/co-pinned`.

- `consumers/co-pinned/Cargo.toml`, `consumers/co-pinned/src/main.rs` (new)
- `crates/loom-xtask/tests/co_pinned_consumer.rs` (new)
- `crates/loom-xtask/src/release.rs`, `crates/loom-xtask/tests/release_check.rs`: the surface-file
  set and one test
- `Taskfile.yml`: `consumer-check` also copies `Cargo.lock` to and builds `consumers/co-pinned/`
- `website/docs/reference/sdk-stability.md`: the "Pin together" table
- `AGENTS.md`: the § Checks that hold the rules row for the consumer check; `CHANGELOG.md`
  (**Unreleased**)

The `.gitignore` entries `consumers/*/target/` and `consumers/*/Cargo.lock` already cover the new
crate. Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/`, `generated/`, `Cargo.toml`, `Cargo.lock`. `crates/loom-xtask/src/release.rs` is also in
`story:command-invocation-contract`'s scope (one entry each in one set), and `AGENTS.md`,
`CHANGELOG.md` and `sdk-stability.md` are shared prose; merge them.

## Ready

Ready once `story:release-names-embedder-breaks` is implemented; it needs no answer from anybody.
