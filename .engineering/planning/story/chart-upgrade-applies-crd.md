---
format: aep.planning-md/3
id: story:chart-upgrade-applies-crd
kind: story
status: draft
title: Upgrading the loom-agents chart applies the CRD of its release
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
  path: Taskfile.yml
- confidence: cited
  path: charts/loom-agents/Chart.yaml
- confidence: cited
  path: charts/loom-agents/crds
- confidence: cited
  path: charts/loom-agents/templates
- confidence: inferred
  path: charts/loom-agents/templates/role.yaml
revision: 4
---
## Outcome

`helm upgrade` of the `loom-agents` chart applies the `LoomAgent` CRD of its release, so an upgrade
never leaves a controller of version N reading resources through the CRD of version N-1. Helm does
not upgrade files under `crds/`; the chart applies the CRD itself (a template, or a pre-upgrade hook
running server-side apply), and never deletes the CRD on uninstall.

## Why

Trial S5/S6, 2026-10-26: the 0.42.0 CRD had to be applied by hand
(`kubectl apply --server-side --force-conflicts`) before the chart upgrade, "helm skips CRDs". Gap:
"Chart upgrade applies the CRD of its release".

## Acceptance

- `the_chart_renders_the_crd_of_its_version` (or a hook that applies it): `task chart-check`
  checks the rendered CRD equals `deploy/crd/loomagents.loom.beyond10x.dev.yaml`.
- `helm uninstall` leaves the CRD (`helm.sh/resource-policy: keep` or hook semantics).
- An upgrade from the previous chart on kind applies the new CRD (manual check, recorded).

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `charts/loom-agents/templates/`, `charts/loom-agents/crds/`, `charts/loom-agents/Chart.yaml` — cited
- `Taskfile.yml` (`crd-generate` copies the CRD into the chart; `chart-check`) — cited
- `charts/loom-agents/templates/role.yaml` (if a hook needs CRD rights) — inferred
- README.md (Kubernetes install), AGENTS.md § Resource, CHANGELOG.md — cited

## Route

Route A of `decision-blocker:chart-crd-upgrade-route`, chosen by the operator on 2026-11-05 as the
later end state ("yes" to "C now, A later"). The CRD moves into `templates/` with
`helm.sh/resource-policy: keep`. An existing release then needs a one-time relabel before its first
`helm upgrade`, so this story ships in a minor release whose notes announce that migration ahead
of time, under a `**Breaking:**` line. Until then `story:chart-upgrade-guide-applies-crd-first`
documents the manual apply.
