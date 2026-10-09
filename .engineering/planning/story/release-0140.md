---
format: aep.planning-md/3
id: story:release-0140
kind: story
status: active
title: Release Loom 0.14.0
relations:
- serves: vision:O1
scope:
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T20:14:05Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-09T20:14:05Z", actor: "human:timo", revision: 4}
---
## Why

Wave 2026-10-09-w3 (`story:loop-stop-model`, `story:laya-arguments-slice`) merged into `main`
(PR 60, `7c55d96`), after the test-fake fix (PR 59, `f7d2e0b`). Released on the cadence: no merged
work stays unreleased for more than a day. The minor version moves for a breaking change to
`LoopStop`.

## Acceptance

- A release commit on `main` through a bot pull request: workspace version 0.14.0, `Cargo.lock`
  (workspace packages only), CHANGELOG `## [0.14.0] - 2026-10-09` with its summary paragraph,
  README and the site pages that pin the tag say 0.14.0; `loom-xtask release-check --tag 0.14.0`
  exits 0.
- `check` and `Shared source gates` green on that commit.
- An annotated tag `0.14.0` by `b10x-bot[bot]` on that commit; `release` green on the tag.
- The GitHub Release `0.14.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
