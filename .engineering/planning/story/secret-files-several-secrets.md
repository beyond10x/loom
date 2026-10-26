---
format: aep.planning-md/3
id: story:secret-files-several-secrets
kind: story
status: draft
title: spec.secretFiles admits several Secrets and a secretRef key can be staged as a file
relations:
- decomposes: epic:loom-agent-command-variant
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: charts/loom-agents/crds
- confidence: cited
  path: crates/loom-cli/src/stage.rs
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: deploy/crd
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: generated/rust/loom
revision: 2
---
## Outcome

`spec.secretFiles` admits several Secrets, each with its own `mountPath`, and a key of the
`secretRef` Secret can also be staged as a file, so one credential need not live in two Secrets.
Every staged file keeps today's guarantees (owned by the pod user, directory 0700, file 0600, the
Secret as of pod start), and every name stays in `allowedSecretNames`.

## Why

Trial S5/S6, 2026-10-26: takt's endpoint token had to be copied into a second Secret
(`takt-s5-files`) to be staged as a file, while `secretRef opencode-go-api-key` stayed on the agent
"as required" and the wrapper unset its variable. Gap: "secretFiles admits several Secrets, and a
secretRef key can be staged as a file".

## Acceptance

- ESS first: `secretFiles` becomes a list (or gains a list form) and `secretRef` gains an optional
  list of keys staged as files; admission refusals for two mounts at one path and a key the Secret
  does not name; scenarios red first.
- `two_secret_files_entries_are_staged_at_their_paths`.
- `a_secret_ref_key_is_staged_as_a_file_and_kept_in_the_environment` (or removed from it, as the
  story decides).
- `two_secret_files_at_one_mount_path_are_refused`.
- `b10x-loom agent stage-files` keeps its modes and stale-key removal per destination.
- `deploy/crd/` regenerated; `task crd-drift` exits 0. A breaking change of `secretFiles`' shape
  is a `**Breaking:**` CHANGELOG bullet.

## Scope

Derived 2026-10-26 by the coordinator from `b6e35835`.

- `ess/domains/resource.yaml` (`SecretFiles`, `SecretRef`) — cited
- `crates/loom-operator/src/render.rs` (init container `stage-secret-files`), `admission.rs` — cited
- `crates/loom-cli/src/stage.rs` — cited
- `generated/rust/loom/`, `deploy/crd/`, `charts/loom-agents/crds/` — cited
- AGENTS.md § Resource, design § 9a, CHANGELOG.md — cited
