---
format: aep.planning-md/3
id: story:release-0240
kind: story
status: draft
title: Release Loom 0.24.0
relations:
- depends_on: story:cli-custom-protocol-commands
- serves: vision:O1
revision: 2
---
## Why

Wave 2026-10-16-w1 (a consumer's eight requests) is merged into its integration branch and ships in
this release, with the operator's opt-in for unconfined command bindings: the `--unconfined-bindings`
flag or `"unconfined_bindings": true` in the run configuration file, the flag winning.

## Acceptance

- A release commit on the wave's integration branch, merged into `main` through one bot pull
  request: workspace version 0.24.0, `Cargo.lock` (workspace packages only), CHANGELOG
  `## [0.24.0] - <date>` with its summary paragraph and a `Breaking:` sentence; README and the site
  pages that pin the tag say 0.24.0; `loom-xtask release-check --tag 0.24.0` exits 0.
- Package gates on the final integration head; the pull request's checks green on its head.
- An annotated tag `0.24.0` by `b10x-bot[bot]` on the merge commit; `release` green on the tag.
- The GitHub Release `0.24.0` by the bot, its notes the CHANGELOG entry.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/`.
