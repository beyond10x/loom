---
format: aep.planning-md/3
id: epic:loom-agent-command-variant
kind: epic
status: draft
title: A delivery image runs as a LoomAgent Command resource without hand patches
relations:
- serves: vision:governed-autonomy
revision: 1
---
## Outcome

A takt delivery (or any image with its own entrypoint) runs as a `LoomAgent` `Command` resource
without hand patches: it runs once to completion and reports its exit code, takes its plain
environment, credential files and opaque configuration from the resource, runs under the
confinement profiles it declares, reaches only the hosts it declares, and is installed and upgraded
by the chart alone.

## Why

Design `docs/design/loom-agent-crd-deployment.md` § 9a (slice C, built in
`story:loom-agent-command-variant` and `story:loom-agent-trial-fixes`). The takt-on-LoomAgent kind
trial of 2026-10-26 (S5/S6, takt 0.12.0 on loom 0.42.0; orchestrator's record of it, outside the
repository) ran eight takt steps to a draft pull request, and needed nine workarounds on Loom's
side: a `sleep infinity` wrapper, a `kubectl patch` for seccomp and AppArmor, a manual CRD apply
before the chart upgrade, and others. The orchestrator, 2026-10-26: "Add to your wave as store work
(file as stories with Scope and acceptance scenarios, implement none in this wave): the 9 loom gaps
the takt-on-LoomAgent trial found today".

## Stories

- `story:command-variant-runs-once`
- `story:command-run-exit-status`
- `story:command-confinement-profiles`
- `story:loom-agent-spec-env`
- `story:secret-files-several-secrets`
- `story:command-opaque-config-files`
- `story:egress-by-host`
- `story:writable-dirs-mode-0700`
- `story:chart-upgrade-applies-crd`
