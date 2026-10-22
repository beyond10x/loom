---
format: aep.planning-md/3
id: story:agent-project-kubernetes
kind: story
status: draft
title: b10x-loom agent project writes an ess-build/1 graph and plain Kubernetes manifests, and deploys nothing
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
- depends_on: story:agent-serve
revision: 1
---
## Outcome

`b10x-loom agent project --spec <FILE> --target kubernetes --out <DIR>` writes an `ess-build/1`
graph, the build context and plain Kubernetes manifests (one Deployment, one container), and
nothing else: ESS builds and pushes the image, the downstream CI applies the manifests, and Loom's
command line still never builds, pushes or deploys. Milestone M3 of the Kubernetes design.

## Why

Orchestrator decisions on the design's questions: Q1 A ("Loom only projects; `ess` builds and
pushes; CI applies, as your AGENTS.md:1130 requires"; the rule is AGENTS.md § Never, "Never let the
slice or the command line merge, push or deploy"), Q4 C (plain manifests now, an ESS chart later),
Q5 A (`emptyDir` state, cursors rebuilt from `lookback_minutes`).

## Acceptance

- `project_writes_a_build_graph_ess_compiles`: `ess generate build compile` exits 0 on the output.
- `manifests_have_one_deployment_one_container`, `replicas: 1`, `strategy: Recreate`.
- `manifests_reference_no_secret_value`: the Secret by name only.
- `networkpolicy_lists_only_declared_egress`: `deploy.kubernetes.egress` and DNS, default deny.
- Pod: `runAsNonRoot`, `readOnlyRootFilesystem`, `allowPrivilegeEscalation: false`, capabilities
  dropped, `automountServiceAccountToken: false`, `emptyDir` at the state directory.
- `project.json` names the input digests and the Loom and ESS tags.
- The output passes `kubeconform` (test skips naming it when absent).
- (manual) `docker run <image> agent serve --once` exits 0 against the fake `connectors`.

## ESS first

The first commit adds the projection scenarios to `ess/domains/agent.yaml` only; they are red until
`project` lands.

## Open before it can be ready

- Binaries in the image: Loom publishes no release binary (`release.yml` verifies and creates no
  artifact, AGENTS.md § Releases step 4). Without one the `bins` node builds from source at the tag
  inside the image build. A Loom release-binaries story is not filed.

## Not in scope

A Helm chart from `ess-runtime/1` (Q4 C, later); a PVC (M7); applying anything.

## Scope

`crates/loom-agent` (`project`), `crates/loom-cli/src/agent.rs`, `ess/domains/agent.yaml`,
`website/docs/reference/cli.md` (generated), README.md, AGENTS.md, CHANGELOG.md.

## Ready

After `story:agent-serve`.
