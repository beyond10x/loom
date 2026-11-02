---
format: aep.planning-md/3
id: story:chart-defaults-to-published-images
kind: story
status: implemented
title: The loom-agents chart runs the published images at its appVersion without an override
relations:
- decomposes: epic:loom-foundation-for-consumers
- serves: vision:O1
- depends_on: story:release-publishes-images
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T10:20:24Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T10:20:25Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-02T12:57:25Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`charts/loom-agents` installs the published images without an override: `image.repository` is
`ghcr.io/beyond10x/loom-operator` and `agentImage.repository` is `ghcr.io/beyond10x/loom-agent`,
and both tags default to the chart's `appVersion`, so an install of chart `<version>` runs images
`<version>`. A value set by the installer still wins.

## Why

Item D377 (2026-11-02): a downstream platform deploys from the GitHub registry and adds only
configuration. The chart named `registry.example/...` and carried both tags by hand, which every
release had to move.

## Acceptance

- `values.yaml` names the two ghcr repositories and leaves both tags empty; it carries no comment.
- The controller Deployment runs `<image.repository>:<image.tag or appVersion>`, and the
  controller's `config.yaml` `defaultImage` is `<agentImage.repository>` with tag
  `<agentImage.tag or appVersion>`.
- `crates/loom-operator/tests/chart.rs` holds both defaults against the workspace version and both
  overrides; a release moves only `Chart.yaml` `version` and `appVersion` (AGENTS.md § Releases).

## ESS first

Exempt (AGENTS.md § ESS): no behaviour of Loom changes. The chart's default values and two
template expressions change; the controller and its configuration format do not, and nothing enters
`ess/`. Its red test is `the_chart_and_its_images_carry_the_workspace_version` and
`the_controller_runs_the_configured_image_with_the_values_as_its_config` in
`crates/loom-operator/tests/chart.rs`, which fail on the earlier `registry.example` values.
