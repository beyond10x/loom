---
format: aep.planning-md/3
id: story:release-process
kind: story
status: implemented
title: Loom releases at bare-version tags, starting with 0.1.0
relations:
- decomposes: epic:runtime-consolidation
- serves: vision:governed-autonomy
scope:
- confidence: inferred
  path: .github/workflows
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: README.md
- confidence: inferred
  path: website
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T13:06:20Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-05T13:06:20Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-05T13:50:34Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"test_result":1,"verification":1}}}
---
## Outcome

Loom releases at bare-version tags, starting with `0.1.0`: a release workflow verifies every tag,
a release commit sets the workspace version and turns `CHANGELOG.md`'s Unreleased section into the
version's entry, `b10x-bot[bot]` tags the green `main` commit and publishes the GitHub Release with
notes from that entry. Consumers pin `tag = "0.1.0"` instead of a Git revision.

## Why

Operator decision 2026-10-05 (option A, "A - also do gh release with notes"): release 0.1.0 now;
confinement ships later as 0.2.0. Loom `AGENTS.md` § Releases: "A first release starts with a
release workflow; do not push a tag before one exists".

## Acceptance

- `.github/workflows/release.yml` runs on a pushed version tag: it runs Loom's check (reusing
  `check.yml` or its steps) on the tagged commit and fails when the tag differs from the workspace
  version or `CHANGELOG.md` has no entry for it.
- The workspace version is `0.1.0`; `CHANGELOG.md` has `## [0.1.0] - 2026-10-05`; README,
  AGENTS.md § Releases and the site name the tag (install and SDK lines pin `tag = "0.1.0"`).
- Tag `0.1.0` (annotated, by the bot) is on a green `main` commit; the release workflow run on it is
  green; the GitHub Release `0.1.0` exists, by the bot, with the CHANGELOG entry as its notes.

## ESS first

None: release process; no behaviour change.
