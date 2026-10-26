---
format: aep.planning-md/3
id: story:loom-agent-trial-fixes
kind: story
status: implemented
title: 'LoomAgent trial fixes: a files Secret staged for its reader, optional connectors and image repository'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T11:19:28Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-26T11:19:28Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-26T12:00:42Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

What the local kind trial of takt as a `LoomAgent` (plan steps S3 and S4, loom 0.41.0 chart) found
missing, closed before 0.42.0 ships:

- Two Secrets, each projected one way: `spec.secretRef` stays the environment (`envFrom`), and a
  new `spec.secretFiles {name, mountPath}` (`loom.resource.SecretFiles`) mounts a Secret as files.
  Both names must be in `allowedSecretNames`. The unreleased `spec.secretRef.mountPath` is removed.
- The files are owned by the pod's user, so a reader that refuses a file another user owns or
  that group or others may read (`b10x-gates` reading its policy and key) accepts them: an init
  container `stage-secret-files` running the controller's default image copies the Secret's keys
  with `b10x-loom agent stage-files --from <dir> --to <dir>` into a memory-backed emptyDir, the
  directory mode 0700 and each file 0600, both owned by user 10001; the container mounts that
  directory read-only at `mountPath`.
- `loom.agent.RuntimeSpec.connectors` and `loom.agent.KubernetesDeploy.image_repository` become
  optional, so a spec that uses neither (a `Command` resource such as takt's) need not declare
  them; `agent serve` still refuses to start without `runtime.connectors`, as it refuses without
  `runtime`.

## Why

The orchestrator, 2026-10-26, from S3/S4 on the kind cluster: "`spec.secretRef` takes one Secret
and renders envFrom only; takt needs two (model key as env, forge key as files)", "`b10x-gates`
refuses a policy or key file not owned by the caller or group/world-readable … Secret volume files
are root-owned: the file projection needs an init step that copies into an emptyDir owned by the
pod user (dir 700, files 600)", and "The operator's render refused specs without
`runtime.connectors` and `deploy.kubernetes.image_repository` even when unused; takt's variant
should not need them." Land as a follow-up unit before the release.

## Acceptance

- A resource with `secretRef` and `secretFiles` naming two allowed Secrets renders `envFrom` from
  the first and, from the second, the init container and the read-only mount; a `secretFiles`
  name not allowed is `SecretNotAllowed`; its `mountPath` follows the `MountPathInvalid` rules.
- `b10x-loom agent stage-files` writes each key of a Secret volume (following its symlinks,
  skipping `..` entries) as a 0600 file in a 0700 directory and refuses a destination that is not
  a directory.
- A `loom.agent/1` document without `runtime.connectors` or `deploy.kubernetes.image_repository`
  decodes and passes `agent check`; the CRD no longer requires them.
