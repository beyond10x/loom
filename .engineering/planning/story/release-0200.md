---
format: aep.planning-md/3
id: story:release-0200
kind: story
status: active
title: Release Loom 0.20.0
relations:
- depends_on: story:engineering-protocols-070-pin
- serves: vision:O1
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T01:38:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-14T01:38:01Z", actor: "human:timo", revision: 3}
---
## Why

Wave 2026-10-13-w4 (`story:engineering-protocols-070-pin`) is merged into its integration branch;
it ships in this release, released on the cadence. The minor version moves for breaking changes:
`CanonGovernor::open` and `open_case` take the case inputs, `OpenError` gains four variants,
`CaseInputs` is built fallibly, `StoredCase` gains `inputs`, the action `repository.edit` is
replaced by `tests.write` and `implementation.edit`, and a state directory 0.19.0 wrote is not
resumed.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.20.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.20.0] - 2026-10-14` with its summary paragraph, README and the site pages that pin the
  tag say 0.20.0; `loom-xtask release-check --tag 0.20.0` exits 0.
- One full local gate on the final integration head; `check` and `Shared source gates` green on
  that pull request's head.
- An annotated tag `0.20.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.20.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
