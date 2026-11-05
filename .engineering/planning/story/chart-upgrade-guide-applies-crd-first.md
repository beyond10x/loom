---
format: aep.planning-md/3
id: story:chart-upgrade-guide-applies-crd-first
kind: story
status: active
title: The chart upgrade guide applies the CRD of the new tag before helm upgrade
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-operator/tests/chart.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T09:33:25Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
- {from: "proposed", to: "active", at: "2026-11-05T09:33:25Z", actor: "human:timo", revision: 4, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
---
## Outcome

The upgrade guide of the `loom-agents` chart says to apply the `LoomAgent` CRD of the new tag with
`kubectl apply --server-side -f` before `helm upgrade`, because Helm installs `crds/` once and
never upgrades it. The guide names the CRD of the workspace version, so a release cannot ship a
guide that applies an older CRD. The chart itself does not change.

## Why

The operator chose route C of `decision-blocker:chart-crd-upgrade-route` on 2026-11-05 ("yes" to
"C now, A later"). C breaks no consumer. A, which has the chart apply its CRD, follows in
`story:chart-upgrade-applies-crd`.

## Acceptance

- README.md (Kubernetes install) gives the upgrade order as `kubectl apply --server-side -f` of
  `deploy/crd/loomagents.loom.beyond10x.dev.yaml` at the release tag, then `helm upgrade`. AGENTS.md
  § chart states the same rule.
- `the_upgrade_guide_applies_the_crd_of_the_workspace_version` in
  `crates/loom-operator/tests/chart.rs` fails when README's apply command names a version other than
  the workspace version, or when the command is missing.
- No file under `charts/` changes.
