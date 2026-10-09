---
format: aep.planning-md/3
id: story:release-0120
kind: story
status: implemented
title: Release Loom 0.12.0
relations:
- serves: vision:O3
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-09T16:44:10Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-09T16:44:10Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-09T17:08:49Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"verification":1}}}
---
## Outcome

Publish Loom 0.12.0 as a source release on the merged `main` commit. It carries
https://github.com/beyond10x/loom/pull/53 (wave 2026-10-09-w1: `story:reasoning-model-selector`,
`story:confidence-fallback`, `story:laya-selector`, `story:stale-governor-comments`,
`story:compaction-target-bound`). `LoopStop` gains a variant, breaking for exhaustive matches, so the
minor version moves.

## Acceptance

Workspace and lockfile versions, the dated CHANGELOG entry with its summary, and the README and site
lines that name the release or pin its tag all say `0.12.0`;
`cargo run -q --locked -p loom-xtask -- release-check --tag 0.12.0` exits 0 on the release commit.
The release pull request's required checks pass and it merges; the bot tags the merge commit
`0.12.0` (annotated); the `release` workflow on the tag is green; the bot's GitHub Release `0.12.0`
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
