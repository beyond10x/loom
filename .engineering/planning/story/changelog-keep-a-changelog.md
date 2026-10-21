---
format: aep.planning-md/3
id: story:changelog-keep-a-changelog
kind: story
status: active
title: CHANGELOG.md follows Keep a Changelog, and release-check reads Breaking bullets
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-cli/tests/catalog_route_adversary.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/adversary_w20261020_u2.rs
- confidence: cited
  path: crates/loom-docs/src/main.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w20261020w3_u3.rs
- confidence: cited
  path: crates/loom-executor/tests/adversary_w4_20261009_compaction_target_bound.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_w20261020_u1.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_w20261020w2_u4.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/adversary_w20261020w2_u5.rs
- confidence: cited
  path: crates/loom-protocols/tests/adversary_w8_pin.rs
- confidence: cited
  path: crates/loom-protocols/tests/catalog.rs
- confidence: cited
  path: crates/loom-sdk/src/lib.rs
- confidence: cited
  path: crates/loom-xtask/src/main.rs
- confidence: cited
  path: crates/loom-xtask/src/release.rs
- confidence: cited
  path: crates/loom-xtask/tests/release_check.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T00:23:30Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-21T00:23:30Z", actor: "human:timo", revision: 4}
---
## Outcome

`CHANGELOG.md` follows Keep a Changelog 1.1.0: `## [Unreleased]`, then `## [x.y.z] - YYYY-MM-DD`
newest first; under each version only `### Added`, `### Changed`, `### Deprecated`, `### Removed`,
`### Fixed` and `### Security`; one bullet per change, one line of about 120 characters at most,
naming the public item in backticks; a breaking change starts with `**Breaking:**`, a story id may
follow in parentheses; link references to the tags at the bottom. No prose paragraph remains: the
detail stays in the story files, the commit bodies and git history. Today the file is 168,699 bytes
over 31 releases and Unreleased, almost all prose (Timo, 2026-10-21: "the CHANGELOG is garbage too!
fix that - there is a more official format, nobody wants to read a book").

`loom-xtask release-check` and AGENTS.md § Releases change with it: a release entry needs at least
one `**Breaking:**` bullet or the line `No breaking changes.`, where it needed a `Breaking:`
sentence; `No breaking changes.` is refused when `consumers/pinned/`,
`crates/loom-sdk/tests/host_ports.rs` or `docs/contracts/run-events-v1.json` changed since the
previous tag, as `Breaking: none.` is today. The short GitHub notes are built from the entry's
bullets.

## Acceptance

- Every version heading of today's file is kept with its date; every bullet is checked against the
  old text of that version (no change dropped that a consumer sees, no claim the old text does not
  make), and every `Breaking:` sentence of the old text becomes one or more `**Breaking:**` bullets.
- `crates/loom-xtask/tests/release_check.rs` holds the new rule (a `**Breaking:**` bullet or
  `No breaking changes.`; the refusal when the stable surfaces changed); `cargo run -q --locked -p
  loom-xtask -- release-check --tag 0.30.0` passes on the converted file.
- Every test that reads `CHANGELOG.md` passes, adjusted to the new shape where it read a paragraph.
- `task check` and `task docs-check` exit 0.

## ESS first

No noun; documentation and a release check.

## Scope

- `CHANGELOG.md`, `AGENTS.md` (§ Releases)
- `crates/loom-xtask/src/release.rs`, `crates/loom-xtask/src/main.rs`, `crates/loom-xtask/tests/release_check.rs`
- the tests that read `CHANGELOG.md`: `crates/loom-cli/tests/catalog_route_adversary.rs`,
  `crates/loom-commission-testkit/tests/adversary_w20261020_u2.rs`,
  `crates/loom-executor/tests/adversary_w20261020w3_u3.rs`,
  `crates/loom-executor/tests/adversary_w4_20261009_compaction_target_bound.rs`,
  `crates/loom-intake-slice/tests/adversary_w20261020_u1.rs`,
  `crates/loom-intake-slice/tests/adversary_w20261020w2_u4.rs`,
  `crates/loom-intake-slice/tests/adversary_w20261020w2_u5.rs`,
  `crates/loom-protocols/tests/adversary_w8_pin.rs`, `crates/loom-protocols/tests/catalog.rs`
- `crates/loom-docs/src/main.rs`, `crates/loom-sdk/src/lib.rs` (where they name the CHANGELOG)
