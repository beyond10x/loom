---
format: aep.planning-md/3
id: story:engineering-protocols-0-13-pin
kind: story
status: implemented
title: Loom builds on Canon 0.7.0 and engineering-protocols 0.13.0, one copy beside takt
summary: story.delivery/2 compiles; the bundled catalog gains four built-ins
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T04:09:52Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-10-29T04:09:53Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-10-29T04:30:20Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
# Story: engineering-protocols-0-13-pin

## Outcome

Loom builds on Canon 0.7.0 (tag `0.7.0`, `a4317d8`) and engineering-protocols 0.13.0 (tag
`0.13.0`, `41ae496`). A crate that depends on both resolves one copy of `b10x-canon` and one of
`b10x-canon-engineering`. The governor compiles `story.delivery/2`, whose `after` order form Canon
0.6.0 refused.

`ProtocolCatalog::engineering()` reads the registry's list, and `bundled()` starts from it. So the
bundled catalog, and with it `protocols list` and the router, gains the built-ins that
engineering-protocols 0.10.0 and 0.11.0 added: `loop-improvement@1`, `self-improvement@1`,
`story-delivery@1` and `story-delivery@2`. Each engineering entry names `0.13.0` as its revision.

`self-improvement@1` declares the case input `surface`, and `story-delivery@2` declares
`tests_first` and `gate_on`. `b10x-loom run`'s defaults give none of them a value, so a run that
picks either is refused naming the input (`OpenError::MissingInput`), as the intake slice
documents for any declared input the host does not supply. An embedder that passes the inputs
opens them.

## Why

takt pins engineering-protocols 0.13.0 beside `loom-sdk`. Loom 0.48.0 compiles protocols with
Canon 0.6.0, which refuses `story.delivery/2` (`unknown field after` at `2.yaml:208`), so takt's
bump fails 2 of 3 in `handover_after_red`. Decision D181,
`approval-record:engineering-protocols-0-13-pin-d181`.

## Acceptance

- The four manifests name `b10x-canon` at tag `0.7.0` and `b10x-canon-engineering` at tag
  `0.13.0`. `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` passes
  (`crates/loom-protocols/tests/adversary_w8_pin.rs`).
- `the_catalog_lists_what_the_changelog_says_callers_now_see` and
  `the_bundled_catalog_lists_eval_gated_change` list the nine engineering names, plus
  `system-query@1` for `bundled()`. `every_engineering_entry_carries_the_tag_the_manifest_pins`
  holds `0.13.0`.
- `adversary_every_bundled_protocol_matches_the_governor_through_a_superseded_revision`
  (`crates/loom-governor/tests/evaluate_adversary.rs`) opens every bundled protocol. A protocol
  that declares inputs opens with each at its first declared value.
- `adversary_every_registry_protocol_opens_on_a_git_workspace`
  (`crates/loom-intake-slice/tests/adversary_case.rs`): every built-in whose inputs the run
  defaults supply opens. One declaring another input is refused `MissingInput`, naming it.
- `protocols_add_names_a_registration_that_installs_when_the_file_declares_another_revision`
  uses `team.review`, a protocol id Loom does not bundle. `self-improvement@1` is now bundled and
  is never replaced.
- `task check` passes locally and in CI.

## Scope

- `crates/loom-governor/Cargo.toml`, `crates/loom-protocols/Cargo.toml`,
  `crates/loom-intake-router/Cargo.toml`, `crates/loom-intake-slice/Cargo.toml`, `Cargo.lock` —
  cited
- `crates/loom-protocols/src/lib.rs` (`ProtocolCatalog::engineering`, the revision) — cited
- `crates/loom-governor/src/lib.rs` (`# Dependencies`), `AGENTS.md` (§ Governor),
  `website/docs/guides/move-to-loom.md`, `website/docs/reference/sdk-stability.md` — cited
- The tests named in Acceptance — cited
