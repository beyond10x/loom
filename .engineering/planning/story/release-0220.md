---
format: aep.planning-md/3
id: story:release-0220
kind: story
status: active
title: Release Loom 0.22.0
relations:
- depends_on: story:dw-run-model-meter
- depends_on: story:dw-workflow-definition-executor
- depends_on: story:dw-typed-step-outputs
- depends_on: story:release-names-embedder-breaks
- depends_on: story:regression-evidence-integrity
- serves: vision:O1
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T21:40:19Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-14T21:40:19Z", actor: "human:timo", revision: 3}
---
## Why

Wave 2026-10-14-w2 (story:dw-run-model-meter, story:dw-workflow-definition-executor,
story:dw-typed-step-outputs, story:release-names-embedder-breaks,
story:regression-evidence-integrity) is merged into its integration branch; it ships in this
release, released on the cadence, with rustls 0.23.46 for RUSTSEC-2026-0346, RUSTSEC-2026-0347,
RUSTSEC-2026-0348 and RUSTSEC-2026-0349. The minor version moves for breaking changes: a bugfix
run without `--regression-test-command` no longer discharges `red_first.required`,
`ResumeContextData` gains `regression_test_command`, and the governor refuses a record that cites
an observation it never received or gives it another instant.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.22.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.22.0] - 2026-10-14` with its summary paragraph, README and the site pages that pin the
  tag say 0.22.0; `loom-xtask release-check --tag 0.22.0` exits 0.
- One full local gate on the final integration head; `check` and `Shared source gates` green on
  that pull request's head.
- An annotated tag `0.22.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.22.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
