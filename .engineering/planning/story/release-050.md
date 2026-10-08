---
format: aep.planning-md/3
id: story:release-050
kind: story
status: active
title: Release Loom 0.5.0
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
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T00:50:05Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-08T00:50:05Z", actor: "human:timo", revision: 3}
---
## Outcome

Publish Loom 0.5.0 as a source release on the merged `main` commit. It carries wave 2026-10-07-w3
(https://github.com/beyond10x/loom/pull/30): `ExecutorOutcome::CaseMoved` and the runtime's judgement of a moved
case on its current frontier (`story:moved-case-outcome`), and the docs check's private ESS output
(`story:docs-check-private-output`). The new variant breaks callers that match `ExecutorOutcome`
exhaustively, so the minor version moves.

## Acceptance

Workspace and lockfile versions, the dated CHANGELOG entry with its summary, and the README and site
lines that name the release or pin its tag all say `0.5.0`;
`cargo run -q --locked -p loom-xtask -- release-check --tag 0.5.0` exits 0 on the release commit.
The release pull request's required checks pass and it merges; the bot tags the merge commit
`0.5.0` (annotated); the `release` workflow on the tag is green; the bot's GitHub Release `0.5.0`
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
