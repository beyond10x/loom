---
format: aep.planning-md/3
id: story:release-0150
kind: story
status: implemented
title: Release Loom 0.15.0
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-10T04:35:36Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-10T04:35:36Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-10T05:10:21Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Why

Wave 2026-10-10-w1 (`story:ess-057-upgrade`, `story:fallback-selection-recording`,
`story:selection-telemetry`) merged into `main` (PR 63, `15c5a31`). Released on the cadence: no merged
work stays unreleased for more than a day. The minor version moves for a breaking change to
the recorded selections on a fallback, `SessionData` and `selection::Pick`.

## Acceptance

- A release commit on `main` through a bot pull request: workspace version 0.15.0, `Cargo.lock`
  (workspace packages only), CHANGELOG `## [0.15.0] - 2026-10-10` with its summary paragraph,
  README and the site pages that pin the tag say 0.15.0; `loom-xtask release-check --tag 0.15.0`
  exits 0.
- `check` and `Shared source gates` green on that commit.
- An annotated tag `0.15.0` by `b10x-bot[bot]` on that commit; `release` green on the tag.
- The GitHub Release `0.15.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
