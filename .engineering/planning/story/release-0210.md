---
format: aep.planning-md/3
id: story:release-0210
kind: story
status: implemented
title: Release Loom 0.21.0
relations:
- depends_on: story:change-evidence-order
- depends_on: story:mandate-authority
- depends_on: story:mandate-approval-on-resume
- depends_on: story:pinned-consumer-check
- depends_on: story:harness-migration-guide
- depends_on: story:command-event-contract
- depends_on: story:sdk-host-port-stability
- serves: vision:O1
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T07:10:30Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-14T07:10:30Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-14T09:49:16Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
## Why

Wave 2026-10-14-w1 (story:change-evidence-order, story:mandate-authority,
story:mandate-approval-on-resume, story:pinned-consumer-check, story:harness-migration-guide,
story:command-event-contract, story:sdk-host-port-stability) is merged into its integration
branch; it ships in this release, released on the cadence. The minor version moves for breaking
changes: the slice's items that take a governor are generic over `T: EvaluationTime`,
`executor::Report::Edited` gains `change`, `ExecuteError` gains `Clock`, and the slice's grant
API carries the Mandate approval.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.21.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.21.0] - 2026-10-14` with its summary paragraph, README and the site pages that pin the
  tag say 0.21.0; `loom-xtask release-check --tag 0.21.0` exits 0.
- One full local gate on the final integration head; `check` and `Shared source gates` green on
  that pull request's head.
- An annotated tag `0.21.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.21.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
