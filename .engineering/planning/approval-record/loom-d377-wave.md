---
format: aep.planning-md/3
id: approval-record:loom-d377-wave
kind: approval-record
status: approved
title: 'Wave 2026-11-02-w1 accepted under the orchestrator grant (D377): published images and chart, chart defaults'
tags:
- non-interactive
relations:
- decides: story:release-publishes-images
- decides: story:chart-defaults-to-published-images
revision: 5
transitions:
- {from: "draft", to: "approved", at: "2026-11-02T10:20:24Z", actor: "human:timo", revision: 2}
---
Wave 2026-11-02-w1 (item D377): published images and chart, chart defaults

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved the item under the grant in the workspace instructions (class C: a wave, its commits, one
bot pull request, its merge and one release), item D377, 2026-11-02. Its words:

- Requirement (the operator, relayed 2026-11-02): Loom must "provide Docker images which we can
  deploy from the GitHub registry"; a downstream platform adds only configuration on top.
- "release.yml: on every release tag, build and push public linux/amd64 images ... tagged with the
  release version. Follow the existing pattern in beyond10x/llm-gateway ... Add a Dockerfile per
  image if none exists (minimal runtime base, non-root)."
- "charts/loom-agents/values.yaml: image defaults point at those ghcr images, tag defaulting to the
  chart's appVersion; publish the chart as an OCI artifact to ghcr ... No comments in values.yaml."
- "Release-prepare commit in the wave (next minor), local full gate ..., bot PR, merge on green."
- "STOP before pushing the release tag: the first ghcr publish is registry publishing and waits
  for my explicit go."

Amendments from the orchestrator, same day:

- D378: "Image publish on the release tag is approved by [the operator's] own words (2026-11-02T10:13:26Z:
  \"they must provide Docker images which we can deploy from the GitHub registry\")".
- D382: the chart's OCI push is part of the release workflow.
- D383 reverses D379, D380 and the neutral-key part of D378 (the operator corrected his rule: "It's
  of course completely fine if it's called loom operator, if the image is originated from the loom
  repository"): "images ghcr.io/beyond10x/loom-agent (b10x-loom) and
  ghcr.io/beyond10x/loom-operator (b10x-loom-operator); chart
  oci://ghcr.io/beyond10x/charts/loom-agents (keep the existing chart name). Cancel the CRD/env
  rename ... Neutral values keys: drop them unless already finished and tested". None was
  recorded or implemented before D383, so no store record of the rename exists.
- Still: "report the PR merge and wait for my go before the tag."

## Units

| unit | story | scope |
|---|---|---|
| A | story:release-publishes-images | `deploy/images/`, `.dockerignore`, `.github/workflows/images.yml`, `.github/workflows/release.yml`, `crates/loom-xtask/tests/release_check.rs` |
| B | story:chart-defaults-to-published-images | `charts/loom-agents/` (`values.yaml`, templates), `crates/loom-operator/tests/chart.rs` |

Integration: `wave/2026-11-02-w1` in managed tree `loom-d377-int` (base origin/main abdb0c73,
Loom 0.53.0). Release 0.54.0, no breaking change.

## Decisions taken inside the item

1. Image names `loom-agent` and `loom-operator`, the names the chart already used (D383).
2. One tree, units in sequence by the lead: `/` stood at its 80G build floor at the start, so one
   build directory serves the wave. Both units are workflow, Dockerfile and chart text held by
   tests; no adversary pass (no hand-written runtime code).
3. Runtime base `debian:bookworm-slim` with `ca-certificates`, `libssl3` and `libcurl4`, pinned by
   digest: `b10x-loom` links OpenSSL and libcurl through its git and HTTP dependencies, which a
   distroless base lacks. The Dockerfile fails the build on any unresolved library.
4. The images build on pull requests that touch them (no push), so a broken Dockerfile shows on
   the PR, not first on the release tag.

## Commits this approval authorises

The opening store commit, the unit commits, the release-prepare commit (0.54.0), one bot PR to
main, its merge on green. The release tag waits for the orchestrator's go.

## D385 (orchestrator, 2026-11-02)

"(b) accepted: no local full gate; CI's full gate on the bot PR decides. ESS pin 0.77.0 -> 0.82.0:
include. (a) targeted tests: wait" for the disk floor host-monitor sets. Package visibility: b10x-atlas
sets the packages public after the first publish.

## Go for merge and tag (orchestrator, 2026-11-02)

The orchestrator gave the go for merge and tag on 2026-11-02, under the grant (D377, D383), after every check of pull request 154 passed at fcd9b4a2 (the macOS build re-run once: its first attempt got no hosted runner).

- Merged through the bot: https://github.com/beyond10x/loom/pull/154, merge commit 0dff39169b7d58f8fd951ef173b6eb28cf3a12cb, tree equal to fcd9b4a2.
- Tag 0.54.0, annotated by the bot on that commit, pushed; release run https://github.com/beyond10x/loom/actions/runs/41460610672.
- Image and chart visibility: the operator's own words, relayed 12:54Z: "Yes, tagged and the chart and loom can be public. Yeah, so option A." The packages are set public after the first publish.
