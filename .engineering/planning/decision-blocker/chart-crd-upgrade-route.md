---
format: aep.planning-md/3
id: decision-blocker:chart-crd-upgrade-route
kind: decision-blocker
status: cleared
title: How a loom-agents chart upgrade delivers its CRD; operator decides
relations:
- blocks: story:chart-upgrade-applies-crd
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-11-05T09:32:49Z", actor: "human:timo", revision: 3, executor: "agent:loom-wave-lead", correlation: "loom-wave-2026-11-05-w3"}
---
Decider: operator. A consumer-visible change to how the chart installs its CRD; no default on silence.

## Question

How does an upgrade of the `loom-agents` chart deliver the CRD of its release? Helm installs a
chart's `crds/` directory once and never upgrades it (`README.md` Kubernetes install), so today a
`helm upgrade` leaves the previous release's `LoomAgent` CRD in place.

| option | what | cost |
|---|---|---|
| A | the CRD moves from `charts/loom-agents/crds/` into `templates/crd.yaml`, annotated `helm.sh/resource-policy: keep`, so every upgrade applies it and an uninstall leaves it | an existing release's CRD was created without Helm ownership metadata, so the first `helm upgrade` of every existing release fails until a one-time `kubectl label` / `kubectl annotate` (`app.kubernetes.io/managed-by=Helm`, `meta.helm.sh/release-name`, `meta.helm.sh/release-namespace`); a `**Breaking:**` release with that migration command |
| B | a pre-install / pre-upgrade hook Job applies the CRD with server-side apply | a hook ServiceAccount with a ClusterRole and ClusterRoleBinding on `customresourcedefinitions` (new cluster-scoped privilege), an image that carries the apply (a new `b10x-loom-operator crd apply` or a pinned third-party image), hooks mapped to PreSync under GitOps tools |
| C | the chart stays as today; the upgrade guide says to run `kubectl apply --server-side -f deploy/crd/loomagents.loom.beyond10x.dev.yaml` of the new tag before `helm upgrade` | a manual step on every upgrade that changes the CRD; nothing breaks |

Option D, the operator applying its own CRD at start, is excluded by
`docs/design/loom-agent-crd-deployment.md:203` (no ClusterRole for the controller).

## Recommendation

C now, A later: C breaks no consumer and needs only documentation and a check that the guide names
the tag's CRD; A is the end state, taken in a release that announces the one-time migration ahead
of time. B adds cluster-scoped privilege for what one documented command does.

## What it blocks

`story:chart-upgrade-applies-crd`. Scoping on 2026-11-05 (tree `7e408a5e`): chart, `Taskfile.yml`
(`crd-generate`, `chart-check`), `crates/loom-operator/tests/chart.rs`, the CRD projection only
under option A; no workflow change under any option.

## Source

The orchestrator took the story out of wave 2026-11-05-w2 on 2026-11-05: "option A makes the first
`helm upgrade` of every existing release fail until a manual relabel, which breaks a consumer".

## Decided

The operator answered on 2026-11-05, relayed by the orchestrator: "yes" to the recommendation.

- C now: the chart stays as it is, and the README and AGENTS.md § chart say to run
  `kubectl apply --server-side -f` with the CRD of the new tag before `helm upgrade`.
  `story:chart-upgrade-guide-applies-crd-first` delivers it.
- A later: `story:chart-upgrade-applies-crd` moves the CRD into the chart templates in a release
  that announces the one-time migration ahead of time. It stays a draft until then.
