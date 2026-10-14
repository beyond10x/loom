---
format: aep.planning-md/3
id: story:generated-types-release-surface
kind: story
status: draft
title: 'release-check refuses Breaking: none when a generated type control-plane names changed since the previous tag'
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:governed-autonomy
- depends_on: story:release-names-embedder-breaks
revision: 1
---
## Outcome

A Loom release that changes a generated model type control-plane names cannot say
`Breaking: none.`: the generated source files that declare the rows the SDK stability page marks
*generated* in its "Used by control-plane" table are stable-surface files of `release-check`, and a
test keeps that set equal to what the page lists.

## Why

The epic's third acceptance asks that a release name every break a consumer's pinned surface sees.
control-plane, the integration point of the engineering factory (served first; cleared
`decision-blocker:g6-first-consumer`), names twelve generated model paths through `loom_sdk`
(`loom_sdk::loom::model::run::{CatalogueEntry, SelectionStrategy, SessionData, RunEnding, ...}`,
`loom_sdk::commission::model::primitives::{Uuid, Timestamp}`, ...). It names them in its own code,
building and matching them, so a new field or variant can break its build. The stability page
already says a release names such a change as a break when it makes one, and 0.15.0 did so by habit
(`SessionData` gains `boundary_refusals`). Nothing checks it: `release-check` diffs only
`consumers/pinned/`, `crates/loom-sdk/tests/host_ports.rs` and
`docs/contracts/run-events-v1.json`, and `host_ports.rs` holds those paths only as `use` lines,
which keep compiling when a field is added. A consumer that moves to a release whose entry says
`Breaking: none.` can still fail to build.

## Acceptance

`release-check` refuses `Breaking: none.` for a release whose generated source files behind the
page's *generated* rows changed since the previous tag, and a test fails when the page lists a
generated row whose file is not in that set:

- `crates/loom-xtask/src/release.rs::tests::every_generated_row_of_the_stability_page_is_a_stable_surface`
  reads the rows of `website/docs/reference/sdk-stability.md`'s "Used by control-plane" table marked
  *generated*, maps each to the file under `generated/rust/<crate>/src/` that declares it
  (`loom_sdk::commission::model` to `generated/rust/commission`, `loom_sdk::loom::model` to
  `generated/rust/loom`), and asserts every such file, and no other generated file, is in the
  stable-surface set.
- `crates/loom-xtask/tests/release_check.rs::release_check_refuses_breaking_none_when_a_generated_type_on_the_page_changed_since_the_previous_tag`
  passes over a fixture repository whose two tags differ only in `generated/rust/loom/src/run.rs`,
  and the refusal names that file.
- `crates/loom-xtask/tests/release_check.rs::release_check_accepts_breaking_none_when_an_unlisted_generated_file_changed`
  passes over a fixture whose two tags differ only in a generated file no page row maps to (for
  example `generated/rust/loom/src/slack.rs`).
- The "Outside the promise" bullet on generated model types in `sdk-stability.md` and the header
  paragraph of `CHANGELOG.md` say that a change to the generated files behind the control-plane
  table is refused as `Breaking: none.`.
- `the_repository_passes_release_check_at_its_workspace_version` still passes, and `task check`
  and `task website` exit 0.

## ESS first

No behaviour change for the runtime or the command: the rule is in the release tool, and the
generated files are read, not changed. No ESS noun is involved, so `ess/` is untouched. The red step
is `every_generated_row_of_the_stability_page_is_a_stable_surface`, which fails before the set holds
the generated files.

## Domain relations

None. Release surfaces and page rows are not nouns of any `ess/1` document in this repository, and
the story introduces none.

## Depends on

`story:release-names-embedder-breaks`, whose stable-surface set this story extends.

## Not in scope

- Bringing generated types into the promise beyond the page's rows, or a field-level snapshot of
  them; a whole-file diff is the rule, as for `host_ports.rs`.
- Generated types no consumer names on the page; a later row on the page adds its file.
- Deciding whether a given change is a break: the release author writes the sentence.
- Rewriting earlier CHANGELOG entries; any change to `ess/` or `generated/`.
- Any change in control-plane; the agent platform (consumer A), which waits.

## Scope

Crates: `loom-xtask`.

- `crates/loom-xtask/src/release.rs`: the stable-surface set and one unit test
- `crates/loom-xtask/tests/release_check.rs`: two tests
- `website/docs/reference/sdk-stability.md`: the generated-types bullet
- `CHANGELOG.md`: the header paragraph and an **Unreleased** entry
- `AGENTS.md`: the release row in § Checks that hold the rules

Touches none of `ess/`, `generated/`, `crates/loom-intake-slice`, `crates/loom-governor`,
`crates/loom-cli`, `Cargo.toml`, `Cargo.lock`. `crates/loom-xtask/src/release.rs` and
`release_check.rs` are also in the scope of `story:co-pinned-consumer-check` and
`story:command-invocation-contract` (one entry each in one set); `sdk-stability.md`, `AGENTS.md`
and `CHANGELOG.md` are shared prose; merge them.

## Ready

Ready once `story:release-names-embedder-breaks` is implemented; it needs no answer from anybody.
