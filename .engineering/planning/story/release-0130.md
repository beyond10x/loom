---
format: aep.planning-md/3
id: story:release-0130
kind: story
status: implemented
title: Release Loom 0.13.0
relations:
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T18:20:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-09T18:20:59Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-09T18:47:47Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Why

Wave 2026-10-09-w2 (the plugin layer and the slack-handler) merged into `main` (PR 56,
`dba2a7d`). Conductor decided the release (DEC-20261009-27): no merged work stays unreleased for
more than a day, and the plugin layer is the operator's priority.

## Acceptance

- A release commit on `main` through a bot pull request: workspace version 0.13.0, `Cargo.lock`
  (workspace packages only), CHANGELOG `## [0.13.0] - 2026-10-09` with its summary paragraph,
  README and the site pages that pin the tag say 0.13.0; `loom-xtask release-check --tag 0.13.0`
  exits 0.
- `check` and `Shared source gates` green on that commit.
- An annotated tag `0.13.0` by `b10x-bot[bot]` on that commit; `release` green on the tag.
- The GitHub Release `0.13.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
