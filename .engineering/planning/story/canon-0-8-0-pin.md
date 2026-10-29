---
format: aep.planning-md/3
id: story:canon-0-8-0-pin
kind: story
status: implemented
title: Loom builds on Canon 0.8.0 and engineering-protocols 0.14.0, one copy beside takt
summary: takt moves to Canon 0.8.0 without a second Canon
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T12:57:53Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"approval":1}}}
- {from: "proposed", to: "active", at: "2026-10-29T12:57:53Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-10-29T13:26:26Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"approval":1}}}
---
# Story: canon-0-8-0-pin

## Outcome

Loom builds on Canon 0.8.0 (tag `0.8.0`, `cf17175`) and engineering-protocols 0.14.0 (tag
`0.14.0`, `9a30403`). A crate that depends on both resolves one copy of `b10x-canon` and one of
`b10x-canon-engineering`. Each engineering entry names `0.14.0` as its revision. The bundled
`story.delivery/2` is engineering-protocols 0.14.0's, which admits `implementation.edit` under a
red gate only after a failing run observed after the latest `tests_change` (Canon's `since`).

## Why

takt moves to Canon 0.8.0 (`task:canon-0-8-0` in its store) and must not carry two Canon versions
beside `loom-sdk`. Loom 0.49.1 builds on Canon 0.7.0. Decision D203,
`approval-record:canon-0-8-0-pin-d203`.

## Acceptance

- The four manifests name `b10x-canon` at tag `0.8.0` and `b10x-canon-engineering` at tag
  `0.14.0`. `the_lockfile_holds_one_canon_and_one_registry_from_the_pinned_tags` passes
  (`crates/loom-protocols/tests/adversary_w8_pin.rs`).
- `every_engineering_entry_carries_the_tag_the_manifest_pins` holds `0.14.0`.
- `canon_decision_bytes.rs`: the governor's decision equals Canon 0.8.0's for each fixture.
- `task check` passes locally and in CI.

## Scope

- `crates/loom-governor/Cargo.toml`, `crates/loom-protocols/Cargo.toml`,
  `crates/loom-intake-router/Cargo.toml`, `crates/loom-intake-slice/Cargo.toml`, `Cargo.lock` —
  cited
- `crates/loom-protocols/src/lib.rs` (`ProtocolCatalog::engineering`, the revision) — cited
- `crates/loom-governor/src/lib.rs` (`# Dependencies`), `AGENTS.md` (§ Governor),
  `website/docs/guides/move-to-loom.md`, `website/docs/reference/sdk-stability.md` — cited
