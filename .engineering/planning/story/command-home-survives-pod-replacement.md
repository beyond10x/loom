---
format: aep.planning-md/3
id: story:command-home-survives-pod-replacement
kind: story
status: draft
title: A Command LoomAgent's HOME can survive a pod replacement
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
- depends_on: story:writable-dirs-mode-0700
revision: 1
---
## Outcome

A `Command` `LoomAgent` can keep its `HOME` (`/var/lib/agent`) across a pod replacement, as the
workspace already can: the resource names where `HOME` lives (for example a subdirectory of the
workspace claim, or a claim of its own), and a spec change that replaces the pod leaves the
program's state and transcripts in place.

## Why

The orchestrator, 2026-10-26, from the takt one-hour pod trial (S7, agent report 19:3xZ): "HOME
content is lost when the pod is replaced (only the workspace PVC survives)". The trial record:
"HOME is an emptyDir: a spec change replaces the pod and loses takt's state and transcripts
(new)".

## Acceptance

- `a_command_home_on_a_claim_survives_a_new_generation`: with the persistent `HOME` declared, the
  rendered pod mounts `HOME` from a claim (or claim subdirectory), and a new generation's pod
  mounts the same one.
- Without the declaration, `HOME` stays an emptyDir (today's behaviour), documented as lost on
  replacement.
- The mounted directory meets the same ownership and mode rule as story:writable-dirs-mode-0700.

## Scope

Derived 2026-10-26 by the coordinator, read in the tree at `5a052c44`.

- `ess/domains/resource.yaml` (a member declaring where `HOME` lives) — inferred
- `crates/loom-operator/src/render.rs` (the `Command` pod's `state` volume) — cited
- `crates/loom-operator/src/admission.rs` (a claim name or subdirectory refused when not plain) — inferred
- `deploy/crd/`, the chart's CRD copy — inferred
- AGENTS.md § Resource and § Operator, design § 9a, CHANGELOG.md — inferred
