---
format: aep.planning-md/3
id: story:command-event-contract
kind: story
status: active
title: The run --output jsonl records are a checked V1 contract that cannot break silently
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-docs/src/main.rs
- confidence: inferred
  path: crates/loom-docs/src/run_events.rs
- confidence: inferred
  path: docs/contracts/run-events-v1.json
- confidence: inferred
  path: website/docs/reference/run-events.md
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-14T02:54:28Z", actor: "human:timo", revision: 9}
---
## Outcome

The `b10x-loom` command's machine interface is a stated, checked contract: the `run --output jsonl`
records of the `intake.events` domain at `schema_version` 1, and the exit statuses the terminal
record carries. A committed snapshot of that contract, `docs/contracts/run-events-v1.json`, is
compared with the declared domain by `task docs-check`, so a change that would break a program
reading the stream fails Loom's gate unless the snapshot moves with it.

## Why

The `b10x-loom` command is Loom's second stable surface beside the crates: its `run --output jsonl`
`intake.events` records are a stable contract, they stay stable per release, and a break is named
in the CHANGELOG. Today `website/docs/reference/run-events.md` is checked for completeness (every
kind and field is on the page), but nothing notices a field renamed, retyped or removed while the
version stays `1`: the page and the declaration move together and the contract moves silently.

## Acceptance

`task docs-check` fails when the declared `intake.events` domain differs from the committed V1
contract in a way that breaks a reader, and passes on additions a reader by name ignores:

- `crates/loom-docs/src/run_events.rs::the_v1_contract_matches_the_declared_events` (unit test)
  compiles `ess/intake` and asserts the snapshot equals the declaration's record kinds, their
  fields, field types and optionality, the `SchemaVersion` variants, and the exit statuses.
- `crates/loom-docs/src/run_events.rs::a_removed_renamed_or_retyped_field_is_a_break`,
  `a_new_record_kind_is_a_break` and `a_new_optional_field_is_not_a_break` drive the comparison over
  hand-built declarations.
- `loom-docs generate` rewrites the snapshot; `loom-docs generate --check` (in `task docs-check`)
  reports each difference by kind and field and says whether it is a break.
- `website/docs/reference/run-events.md` gains a "Stability" section stating the contract as
  decided: within `schema_version` 1 a record kind or field is never removed, renamed or retyped,
  and an exit status never changes meaning; an added optional field is not a break; anything else
  is a break, named in that release's CHANGELOG entry under `Breaking:`, and changes the schema
  version. `task website` exits 0.

## ESS first

No behaviour change: the declaration in `ess/intake/domains/events.yaml` and the writer in
`crates/loom-cli` are read, not changed; the story adds a snapshot, a check and documentation. The
red step is `the_v1_contract_matches_the_declared_events`, which fails before the snapshot exists.

## Domain relations

None introduced. The snapshot records the `intake.events` domain
(`ess/intake/domains/events.yaml`: `RunEventLine` holds one `RunEvent`, a union tagged `kind`)
as declared, and adds nothing to it.

## Not in scope

- Changing any record, field or exit status, or adding a schema version.
- The command's human output and its other subcommands; flags beyond those the generated
  `reference/cli.md` already holds.
- The CHANGELOG rule at release time (`story:release-names-embedder-breaks`).

## Scope

- `docs/contracts/run-events-v1.json` (new, written by `loom-docs generate`)
- `crates/loom-docs/src/run_events.rs`, `crates/loom-docs/src/main.rs`
- `website/docs/reference/run-events.md`: the "Stability" section
- `AGENTS.md`: the snapshot in § Generated files and a row in § Checks that hold the rules
- `CHANGELOG.md` (**Unreleased**)

Touches none of `crates/loom-intake-slice`, `crates/loom-governor`, `crates/loom-cli`,
`ess/intake/`, `generated/rust/intake`, `crates/loom-mandate`, `ess/domains/authority.yaml`,
`generated/rust/loom`, `Cargo.toml`, `Cargo.lock`. It reads `ess/intake/` and the
`b10x_loom_cli::events` constants: a running change to the events domain turns this check red until
the snapshot is regenerated, which is the check working. `AGENTS.md` and `CHANGELOG.md` are shared;
merge them.

## Ready

Ready now.
