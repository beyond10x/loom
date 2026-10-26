---
format: aep.planning-md/3
id: approval-record:loom-agent-command-variant-filing
kind: approval-record
status: draft
title: Filing story:loom-agent-command-variant and wave 2026-10-26-slicec accepted under the orchestrator grant
tags:
- non-interactive
relations:
- decides: story:loom-agent-command-variant
revision: 2
---
# Filing and approving story:loom-agent-command-variant, wave 2026-10-26-slicec

The run is a sub-agent of session `b10x-chef-orchestrator`: no operator turn. The orchestrator
approved the item under the grant in `~/beyond10x/CLAUDE.md`, in these words (2026-10-26): "Goal
(Timo, 2026-10-26): takt runs as a `LoomAgent` on a Kubernetes cluster. The design is
`docs/design/loom-agent-crd-deployment.md` § 9a (slice C: a second `LoomAgent` variant whose pod
runs a takt image with takt's own entrypoint, `spec.agent` carrying takt's run configuration) and
§ 10 row 7 (option A, default taken by the orchestrator: slice C now, confinement = the pod for the
local trial; Substrate confinement stays a later slice)." And: "Define the /config contract on
loom's side as 'the `spec.agent` document is written verbatim to `/config/agent.yaml`' so both
sides meet without coordination." The item asks for a release per AGENTS.md § Releases after the
wave.

Skill `aep:implementing` 0.24.1, wave mode, N = 1: the item names one change, filed as one story
because its parts (the domain, the CRD, the controller, the chart) are one dependency chain.

| unit | serves | surface | review |
|---|---|---|---|
| u1 `story:loom-agent-command-variant` | `vision:governed-autonomy` | `ess/domains/resource.yaml`, `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/`, `crates/loom-resource/`, `crates/loom-operator/`, `crates/loom-conformance/`, website reference pages, `docs/design/loom-agent-crd-deployment.md`, AGENTS.md, README.md, CHANGELOG.md | adversary |

Decisions taken inside the item (the coordinator's, recorded so they can be disagreed with):

- D1. The variant is selected by the presence of `spec.command`, and an absent `entrypoint` or
  `args` is left to the image, so "takt's own entrypoint" is `command: {}`.
- D2. For the `Command` variant the controller checks `spec.agent` for a credential and decodes it
  as `loom.agent/1`, but does not run the serve check against the plugin catalog: the document is
  read by the image's program, not by `agent serve`, and the catalog check would refuse a protocol
  only `agent serve` needs to know.
- D3. Credentials as files: `spec.secretRef.mountPath`, the same allow-listed Secret, read-only,
  beside its environment. No other environment or header member.
- D4. A workspace claim is allow-listed (`allowedClaimNames`) as Secrets are.
- D5. The `Command` pod keeps the serve pod's security context (non-root, read-only root, no
  capability) and gets writable emptyDirs at `/var/lib/agent` (`HOME`) and `/tmp`.

The wave's commits, under the grant: the opening store commit, the unit's ESS-first commit and
its implementation commits, the adversary's test commits, the merge into
`wave/2026-10-26-slicec`, the closing store commit, the bot pull request and the fast-forward of
`main`, then the release commit, tag and GitHub Release per AGENTS.md § Releases.

## Wave result

- One unit, implemented by the coordinator itself rather than a dispatched implementor (the domain,
  CRD, controller and chart are one dependency chain): ESS first `84e0aadd` (red: `task drift`
  and `task crd-drift` exit 201 before regeneration), implementation `6a522d88`.
- Ledger between the passes: carried 0, new 3, resolved 2. Pass 1: 2 findings (credential after
  `:` in `spec.command`; `TooLarge` for a huge Command `spec.agent`), fixed in `988c6ac9`. Pass 2:
  3 findings (`Authorization:Bearer` join, shell punctuation such as `<<<ghp_`, `TooLarge` for
  `spec.command` undocumented), fixed in `a6369f70`; the coordinator read that correction (no test
  edited, operator suite 91 of 91). No third attack. The credential walk over a free-form
  `spec.command` script is a deny-list and stays a heuristic; the Secret belongs in `secretRef`.
- Gate on the integration branch `a8c9b572`, every step of `task check` exit 0 with ESS 0.69.2:
  415 test-result lines, 2298 passed, 0 failed, 9 ignored; `task conform` 119 scenarios, 110
  passed, 0 failed, 0 skipped, 9 unsupported.
- The `/config` contract: the `spec.agent` document is written verbatim to `/config/agent.yaml`
  (keys in the order the API server keeps them, as `/etc/agent/agent.yaml` for `agent serve`).
