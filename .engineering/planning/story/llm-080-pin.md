---
format: aep.planning-md/3
id: story:llm-080-pin
kind: story
status: draft
title: Loom links llm 0.8.0
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-intake-router/tests/adversary_classify.rs
- confidence: cited
  path: crates/loom-intake-router/tests/classify.rs
- confidence: cited
  path: crates/loom-plugin-slack/tests/support/mod.rs
- confidence: cited
  path: crates/loom-plugin/tests/support/mod.rs
revision: 2
---
## Outcome

Every `b10x-llm-*` dependency names llm tag `0.8.0`, and `Cargo.lock` holds one version of each.
llm 0.8.0 (tag object `73ba920b` on `7873aa63`, <https://github.com/beyond10x/llm/releases/tag/0.8.0>)
breaks full `Capabilities { … }` struct literals; llm's review names four in Loom, each moved to
`..Capabilities::text(…)`: `crates/loom-intake-router/tests/adversary_classify.rs:49`,
`crates/loom-intake-router/tests/classify.rs:42`, `crates/loom-plugin/tests/support/mod.rs:277`,
`crates/loom-plugin-slack/tests/support/mod.rs:266` (named by adr-changes, 2026-10-20; not read here).

## Why

The meter and the reasoning-model selector move onto llm's `Model` (story:metering-on-llm-model,
Atlas ADR 0098 point 4), and that move builds on the llm release the rest of the stack pins.

## Acceptance

- Every manifest that names llm names `tag = "0.8.0"`; `cargo tree --locked -i b10x-llm-core` prints one package.
- The four literals use `..Capabilities::text(…)`, and `git grep -n 'Capabilities {' crates` lists no other full literal.
- Recorded requests do not change byte for byte, or every change is named in `CHANGELOG.md`.
- `AGENTS.md:748` and every documented llm tag name `0.8.0` (shared with story:doc-dependency-tags-match-lock).
- `task check` exits 0.

## ESS first

No behaviour change in Loom's own code; a dependency move. If 0.8.0 changes a recorded request, the
story names the test that shows it as the red step.

## Scope

- every `crates/*/Cargo.toml` naming `b10x-llm-*`, `Cargo.lock`
- the four test files above
- `AGENTS.md`, `README.md`, `website/docs/` lines naming the llm tag, `CHANGELOG.md`
