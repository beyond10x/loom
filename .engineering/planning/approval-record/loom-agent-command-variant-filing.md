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
revision: 3
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

## Follow-up: trial fixes

The orchestrator asked, after the first kind trial on 0.41.0, for a follow-up unit before the 0.42.0
tag instead of reopening PR 133. It was filed as `story:loom-agent-trial-fixes` and implemented
by the coordinator on `impl/loom-agent-trial-fixes`:

- ESS first `fdb86b1f` (red: `task drift` exit 201), implementation `9d8c420c`, adversary fixes
  `1f2ed835`, scenario listing `9aa720bb`; merged as `577aa2d2`, `8eb252bf`, `a9180c30`.
- Its three points: two Secrets (`secretRef` as environment, `secretFiles` as files); files the pod
  user owns (init container `stage-secret-files`, `b10x-loom agent stage-files`, dir 0700, files
  0600); `runtime.connectors` and `deploy.kubernetes.image_repository` optional. The unreleased
  `spec.secretRef.mountPath` is removed.
- Adversary pass 1 (`review-result:adversary-trial-fixes-pass-1`): 6 findings, 2 red. F1, F2 and
  F4 fixed, F3 and F5 documented, F6 accepted as infeasible. The coordinator verified the
  correction: both red cases green, no test edited. No second attack.
- The ESS-first scenario was missing from `ess/ess-inputs.yaml` until `9aa720bb`. The gate on
  `577aa2d2` therefore ran 30 scenarios; the gate on `a9180c30` ran 31.
- Gate on `a9180c30`: every step of `task check` exit 0, with 417 test-result lines, 2310 passed,
  0 failed, 9 ignored; `task conform` passed; its suite holds 120 scenarios (the run with the new scenario mutated reported 119 of 120).
- Not run on a cluster here. Whether `b10x-gates` accepts the staged files was reasoned against
  kubelet behaviour and held by `adv_staged_files_under_a_setgid_world_writable_root_pass_the_gates_reader`.
