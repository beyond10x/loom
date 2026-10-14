---
format: aep.planning-md/3
id: story:harness-migration-guide
kind: story
status: active
title: A Harness program finds what each Harness crate becomes in Loom on one guide page
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-executor/tests/harness_map.rs
- confidence: inferred
  path: crates/loom-sdk/tests/harness_paths.rs
- confidence: inferred
  path: website/docs/concepts/commission-and-harness.md
- confidence: inferred
  path: website/docs/guides/move-to-loom.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 8}
---
## Outcome

A program built on Harness finds, on one page of Loom's site, what each Harness crate it uses
becomes in Loom: the `loom_sdk` path it imports instead, or the statement that the crate stays with
Harness, and what changes in behaviour when it moves (tools come from the governed frontier, not a
registration). `website/docs/guides/move-to-loom.md` gains that section beside the existing
Commission, governor and intake map.

## Why

Loom succeeded Harness as the runtime consumers build on. The engineering factory, which is served
first, and the programs still on Harness need the move written down; today
`move-to-loom.md` covers only the three archived repositories, and the Harness map
(`docs/design/harness-map.md`) is a design table for porting, not a consumer's guide: it names
Loom source directories, not the `loom_sdk` paths a consumer imports.

## Acceptance

`move-to-loom.md` has a "Move from Harness" section whose table names every Harness crate of the
Harness map with the same disposition, and every `loom_sdk` path it gives compiles:

- `crates/loom-executor/tests/harness_map.rs::the_migration_guide_names_every_harness_crate_with_its_map_disposition`
  reads `docs/design/harness-map.md` and `website/docs/guides/move-to-loom.md` and asserts each of
  the 14 crates has one row in the guide, a `port` row names a `loom_sdk::` path and a `not carried`
  row names `beyond10x/harness` as its owner.
- `crates/loom-sdk/tests/harness_paths.rs::every_path_the_harness_guide_names_resolves` is a
  compile witness: one `use` per `loom_sdk::` path the guide's table names (for example
  `loom_sdk::loom::harness::wire::ModelPort`, `ToolPort`, `BearerSource`,
  `loom_sdk::loom::harness::turn_loop::AgentLoop`, `loom_sdk::loom::session`), and the test asserts
  the set of paths in the guide equals the set of `use` lines in the file.
- The section states, each with the Loom page that holds it: a Harness `ToolPort` registers tools
  freely while Loom's catalogue is projected from the case's current frontier
  (`concepts/action-selection.md`); a tool call is selected, given arguments and revalidated before
  Loom proposes it; a consequential call leaves Loom only as a proposal (`concepts/where-loom-ends.md`);
  `harness-credential` is not carried and a wire takes its credential from an llm-credentials
  reference.
- `task website` exits 0 and `task check` exits 0.

## ESS first

No behaviour change: documentation and a compile-only test. No type, port or outcome changes, so
`ess/` is untouched. The red step is
`the_migration_guide_names_every_harness_crate_with_its_map_disposition`, which fails before the
section exists.

## Domain relations

None. The guide maps crates to module paths; it introduces no domain noun.

## Not in scope

- Porting any `not carried` Harness crate.
- Any change in `beyond10x/harness` or in a program that uses it.
- The stability Loom promises for these paths (`story:sdk-host-port-stability`).
- The `Planned` caution in `website/docs/concepts/commission-and-harness.md`, beyond a link to the
  new section.

## Scope

- `website/docs/guides/move-to-loom.md`: a "Move from Harness" section; the frontmatter `title`,
  `description`, `lede` and `source` widened to name Harness
- `website/docs/concepts/commission-and-harness.md`: one link to that section
- `crates/loom-executor/tests/harness_map.rs`: one new test
- `crates/loom-sdk/tests/harness_paths.rs` (new)
- `CHANGELOG.md` (**Unreleased**)

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`generated/rust/loom`, `Cargo.toml`, `Cargo.lock`. `CHANGELOG.md` is a shared prose file; merge it.

## Ready

Ready now.
