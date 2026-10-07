---
format: aep.planning-md/3
id: story:release-040
kind: story
status: active
title: Release Loom 0.4.0 with llm 0.3.1
relations:
- informed_by: story:release-process
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: crates
- confidence: cited
  path: website
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-07T13:49:07Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-07T13:49:07Z", actor: "human:timo", revision: 4}
---
## Outcome

The operator explicitly requested merging, tagging and cutting releases for both Loom and llm.
Publish Loom0.4.0 as a source release on the merged main commit, with llm0.3.1 as a released dependency.
The release includes bounded context, system queries, custom protocol catalogs, transient model
retries and the other changes already listed under Unreleased. Legacy context remains default.

## Acceptance

Merge the green feature and retry PRs in order. Set workspace and lockfile versions, dated changelog,
README/install/dependency lines and status documentation consistently. Generate derived docs.
Validate repository/planning/documentation gates and release-check. Merge the green release PR,
create an annotated bot tag on that merge commit, wait for the exact tag's release/source checks,
and publish/verify the GitHub Release with changelog notes. Verify remote tag and release identity.

## ESS first

Release metadata and released dependency pin only; no behavior or ESS contract change.

## Scope

Cargo.toml, Cargo.lock, llm dependency manifests, CHANGELOG.md, README.md, AGENTS.md, website and AEP release record.

## Completion boundary

No Atlas reconciliation, Website pin promotion or deployment is part of this source release.
Documentation publication is asynchronous. Do not report a queued tag as a completed release.
