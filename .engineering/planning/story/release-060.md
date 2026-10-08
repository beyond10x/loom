---
format: aep.planning-md/3
id: story:release-060
kind: story
status: active
title: Release Loom 0.6.0
relations:
- informed_by: story:release-process
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
  path: website/docs/
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T07:34:59Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-08T07:34:59Z", actor: "human:timo", revision: 4}
---
## Outcome

Publish Loom 0.6.0 as a source release on the merged `main` commit. It carries the ESS 0.56.0
upgrade (`story:ess-056-upgrade`) and `RunOutcome::CaseMovedOn` (`story:moved-run-named-outcome`),
both from https://github.com/beyond10x/loom/pull/33. The new variant breaks callers that match
`RunOutcome` exhaustively, so the minor version moves.

## Acceptance

Workspace and lockfile versions, the dated CHANGELOG entry with its summary, and the README and site
lines that name the release or pin its tag all say `0.6.0`;
`cargo run -q --locked -p loom-xtask -- release-check --tag 0.6.0` exits 0 on the release commit.
The release pull request's required checks pass and it merges; the bot tags the merge commit
`0.6.0` (annotated); the `release` workflow on the tag is green; the bot's GitHub Release `0.6.0`
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
