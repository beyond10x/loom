---
format: aep.planning-md/3
id: story:release-070
kind: story
status: active
title: Release Loom 0.7.0
relations:
- serves: vision:O3
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T10:00:57Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-08T10:00:57Z", actor: "human:timo", revision: 4}
---
## Outcome

Publish Loom 0.7.0 as a source release on the merged `main` commit. It carries
https://github.com/beyond10x/loom/pull/35 (`story:run-event-stream`, `story:llm-credentials-bearer`)
and https://github.com/beyond10x/loom/pull/36 (`story:governor-evaluate`,
`story:cli-catalog-model-route`, every llm dependency at `0.4.0`). `harness::wire::CredentialKind`
lost `PartialOrd`, `Ord` and `Hash`, so the minor version moves.

## Acceptance

Workspace and lockfile versions, the dated CHANGELOG entry with its summary, and the README and site
lines that name the release or pin its tag all say `0.7.0`;
`cargo run -q --locked -p loom-xtask -- release-check --tag 0.7.0` exits 0 on the release commit.
The release pull request's required checks pass and it merges; the bot tags the merge commit
`0.7.0` (annotated); the `release` workflow on the tag is green; the bot's GitHub Release `0.7.0`
carries the CHANGELOG entry as its notes.

## ESS first

Release metadata only; no behaviour or ESS contract change.

## Scope

`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `website/data/status.json`,
`website/docs/` (pages that name the release), and this record.

## Completion boundary

No Atlas reconciliation, Website pin promotion or deployment is part of this source release.
Documentation publication is asynchronous. A pushed tag whose `release` run is not green is queued,
not released.
