---
format: aep.planning-md/3
id: story:release-0180
kind: story
status: active
title: Release Loom 0.18.0
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
- {from: "draft", to: "proposed", at: "2026-10-13T10:22:31Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-13T10:22:31Z", actor: "human:timo", revision: 4}
---
## Why

Wave 2026-10-13-w2 (`story:host-suspension-store`, `story:cli-run-holds-suspension`,
`story:cli-resume-after-grant`) merged into `main` (PR 72, `bd5312c`). Released on the cadence: no
merged work stays unreleased for more than a day. The minor version moves for a breaking change:
`SliceError` gains `State` and `Resume` and `b10x_loom_cli::Command` gains `Approve` and `Resume`,
which callers that match them exhaustively must add; `SliceRun` gains `gate` and `RunArgs` gains
`state`, which break a struct literal.

## Acceptance

- A release commit on `main` through a bot pull request: workspace version 0.18.0, `Cargo.lock`
  (workspace packages only), CHANGELOG `## [0.18.0] - 2026-10-13` with its summary paragraph,
  README and the site pages that pin the tag say 0.18.0; `loom-xtask release-check --tag 0.18.0`
  exits 0.
- `check` and `Shared source gates` green on that commit.
- An annotated tag `0.18.0` by `b10x-bot[bot]` on that commit; `release` green on the tag.
- The GitHub Release `0.18.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
