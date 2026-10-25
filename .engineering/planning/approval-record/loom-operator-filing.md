---
format: aep.planning-md/3
id: approval-record:loom-operator-filing
kind: approval-record
status: draft
title: Filing story:loom-operator and wave 2026-10-25-operator accepted under the orchestrator's grant
tags:
- non-interactive
relations:
- decides: story:loom-operator
revision: 1
---
# Filing and approving story:loom-operator, wave 2026-10-25-operator

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved the item under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-25): "loom
step 6 of docs/design/loom-agent-crd-deployment.md, the crates/loom-operator controller and the
charts/loom-agents chart, as a one-unit aep:implementing wave that you coordinate, through the bot
route to main. Approved under the grant in ~/beyond10x/CLAUDE.md (Timo, 2026-10-25: 'you will now
implement the design about deployable loom agents'; 'use /aep:implementing for implementing
stuff')." The item brief fixes the operator's answers to the design's § 10: "Timo decided § 10 row
3 = B (the CRD + controller chart live in beyond10x/loom, versioned with the loom tag; the
deployment repository only references it by version) and row 5 = A (model route = an in-cluster
signing proxy, configured by URL, no credential in the chart). Rows 1, 2, 4, 6 are their
defaults." And: "No release: the orchestrator batches the loom release after the agents wave."

Skill `aep:implementing` 0.24.1, wave mode, N = 1: the orchestrator's item names one story, which
did not exist and was filed first.

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:loom-operator` | `vision:governed-autonomy` | `ess/domains/resource.yaml`, `generated/rust/loom/`, `deploy/crd/`, `crates/loom-operator/` (new), `crates/loom-conformance/`, `crates/loom-executor/tests/crate_names.rs`, `crates/loom-docs/`, `charts/loom-agents/` (new), `Taskfile.yml`, `.github/workflows/check.yml`, `Cargo.lock`, website reference pages, `docs/design/loom-agent-crd-deployment.md`, AGENTS.md, README.md, CHANGELOG.md | adversary |

`aep plan artifact waves --kind story --status draft` was not run for a selection: the item names
one story, and the concurrent wave `2026-10-25-agents` (stories agent-serve, agent-http-binding,
agent-a2a, branch `wave/2026-10-25-agents`) holds `crates/loom-agent`, `crates/loom-cli`,
`crates/loom-plugin*` and `ess/domains/agent.yaml`, which this unit does not touch; its CHANGELOG,
README and AGENTS.md edits are confined to their own sections.

The wave's commits, under the grant: the opening store commit, the unit's ESS-first commit and
its implementation commits on `impl/loom-operator`, the merge into `wave/2026-10-25-operator`, the
closing store commit, and the merge into `main` through a bot pull request on green. No tag, no
release.

Pre-flight: `df -h /` 158G free; one unit, one build directory inside the unit tree
(`~/.local/state/worktree/trees/b10x/loom/loom-operator-u1/target`); integration tree
`loom-operator-w`; `git worktree list` holds other sessions' trees (`loom-agent-domain`,
`loom-agent-domain-u1`, `loom-agents-w2`, `loom-crd-design`, `loom-w18-u2-breaks`,
`loom-w18-u3-clicontract`, `loom-w2-agent-serve`, `loom-wave-20261018-w1`, `wt-8082f40b5998`), left
as they are. Dispatch: `aep:implementor` (opus, xhigh: the unit crosses crates and a security
boundary), then `aep:adversary` (opus, high).

## Decisions taken in planning

- D1: the chart is `charts/loom-agents/` in this repository, `Chart.yaml` `version` and
  `appVersion` the workspace version (row 3 = B); its `crds/` file is a byte-identical copy of
  `deploy/crd/`, held by a test.
- D2: the model route is a chart value `modelRoute: {url, protocol, model}` reaching the
  controller's `config.yaml`; a resource whose `spec.routes` is `{}` gets the default routes
  document from it (one anonymous endpoint, one route per declared role), so no credential is in
  the chart or the resource (row 5 = A). The proxy's wire protocol and model id are values with
  defaults (`messages`, empty), to be settled by the first manual run.
- D3: the spec check runs in the controller through `b10x_loom_agent::check_text`, before any
  child, and is recorded as `status.check`; the pod keeps the init container but its exit code is
  not read back. The design's § 5.1 and § 8 are amended in this wave.
- D4: `status.deployment` is set by every observing outcome: `still-running` and `still-degraded`
  move over self-transitions and emit `LoomAgentObserved`, since an `updates:` beside `moves:`
  makes the command an obligation on ESS 0.69.0 (probed in session scratch).
- D5: `status.conditions` is declared `Optional<List<Condition>>` and set by no command: ESS 0.69.0
  `sets:` admits no list value (ess-gap). The controller computes it from the row.
- D6: `kube` 4.2.0 and `k8s-openapi` 0.28.0 (`v1_34`), pinned in `Cargo.lock`.
- D7: the chart is checked by Helm (`helm lint`, `helm template`) inside `task check`, CI installing
  Helm 3.19.0 by checksum; `kubeconform` is not used. The rendered output is read by a Rust test.
- D8: the credential walk over `spec.routes` reuses `check_text` (`SecretLiteral` is the refusal,
  `Malformed` the pass); `crates/loom-agent` is not changed, the concurrent wave owning it.
