---
format: aep.planning-md/3
id: story:doc-dependency-tags-match-lock
kind: story
status: draft
title: Every documented dependency tag is the one Cargo.lock resolves, held by a test
relations:
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/loom-xtask/tests/doc_dependency_tags.rs
- confidence: cited
  path: website/docs/guides/move-to-loom.md
revision: 2
---
## Outcome

Every line in `AGENTS.md`, `README.md` and `website/docs/` that names a dependency's release tag
names the tag `Cargo.lock` resolves, and a test holds them together so a pin move that forgets a
page fails the gate.

## Why

Wave 2026-10-20-w1 found two stale lines while moving engineering-protocols to 0.9.0: llm is named
`0.4.0` at `AGENTS.md:748` and `0.3.1` at `website/docs/guides/move-to-loom.md:48`, while
`Cargo.lock` resolves llm `0.7.0`. The implementor left a patch at
`~/.cache/loom-wave-2026-10-20-w1/u1/fix1-llm-tag.patch` (outside the repository). Canon,
engineering-protocols, Connectors `v0.35.0`, Mandate `v0.9.0` and `ess` `0.60.0` matched when checked.

## Acceptance

- `AGENTS.md:748` and `website/docs/guides/move-to-loom.md:48` name llm `0.7.0`.
- `crates/loom-xtask/tests/doc_dependency_tags.rs::every_named_dependency_tag_is_the_one_cargo_lock_resolves`
  reads the tags those files name for llm, Canon, engineering-protocols, Connectors and Mandate and
  compares each with `Cargo.lock`; it fails on the base for llm.
- `task check` exits 0.

## ESS first

No behaviour change: documentation and a check. `ess/` is untouched.

## Scope

- `AGENTS.md`, `website/docs/guides/move-to-loom.md`, `crates/loom-xtask/tests/doc_dependency_tags.rs` (new)
