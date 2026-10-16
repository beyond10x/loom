---
format: aep.planning-md/3
id: story:release-0230
kind: story
status: active
title: Release Loom 0.23.0
relations:
- depends_on: story:dw-read-fanout
- depends_on: story:dw-judgement-step-inputs
- depends_on: story:meter-overlapping-calls
- serves: vision:O1
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-16T01:33:25Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-16T01:33:25Z", actor: "human:timo", revision: 3}
---
## Why

Wave 2026-10-15-w1 (story:dw-read-fanout, story:dw-judgement-step-inputs,
story:meter-overlapping-calls) is merged into its integration branch with Loom's specifications
moved to ESS 0.60.0; it ships in this release. The minor version moves for breaking changes:
`ExecutorOutcome` gains the variant `ProposedReads`, `loom.workflow.JudgementStep` gains the
field `inputs`, and Loom requires ESS 0.60.0.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.23.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.23.0] - <date>` with its summary paragraph and a `Breaking:` sentence naming
  `ExecutorOutcome::ProposedReads`, README and the site pages that pin the tag say 0.23.0;
  `loom-xtask release-check --tag 0.23.0` exits 0.
- Package gates on the final integration head; `check` and `Shared source gates` green on that
  pull request's head.
- An annotated tag `0.23.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.23.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
