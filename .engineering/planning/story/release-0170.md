---
format: aep.planning-md/3
id: story:release-0170
kind: story
status: implemented
title: Release Loom 0.17.0
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
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T06:38:45Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-13T06:38:46Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-13T07:22:34Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1}}}
---
## Why

Wave 2026-10-13-w1 (`story:approval-suspend-resume-slice`) merged into `main` (PR 69, `3bd6e45`).
Released on the cadence: no merged work stays unreleased for more than a day. The minor version
moves for a breaking change: `ResumeRunOutcome` and `LoopFailure` gain variants for the suspension
port, which callers that match them exhaustively must add.

## Acceptance

- A release commit on `main` through a bot pull request: workspace version 0.17.0, `Cargo.lock`
  (workspace packages only), CHANGELOG `## [0.17.0] - 2026-10-13` with its summary paragraph,
  README and the site pages that pin the tag say 0.17.0; `loom-xtask release-check --tag 0.17.0`
  exits 0.
- `check` and `Shared source gates` green on that commit.
- An annotated tag `0.17.0` by `b10x-bot[bot]` on that commit; `release` green on the tag.
- The GitHub Release `0.17.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
