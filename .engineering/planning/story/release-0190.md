---
format: aep.planning-md/3
id: story:release-0190
kind: story
status: implemented
title: Release Loom 0.19.0
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
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-13T13:35:19Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-13T13:35:19Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-13T15:13:56Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Why

Wave 2026-10-13-w3 (`story:handoff-holds-no-suspension`) is merged into its integration branch;
it ships in this release, released on the cadence. The minor version moves for a breaking change:
`HoldSuspensionOutcome` gains `Settled` and `LoopFailure` gains `NotResumed`, which callers that
match them exhaustively must add, and the run store given to `run_with_suspension` must implement
`ResumeRunBehavior`.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.19.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.19.0] - 2026-10-13` with its summary paragraph, README and the site pages that pin the
  tag say 0.19.0; `loom-xtask release-check --tag 0.19.0` exits 0.
- `check` and `Shared source gates` green on that pull request's head.
- An annotated tag `0.19.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.19.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
