---
format: aep.planning-md/3
id: story:loom-agent-command-variant
kind: story
status: active
title: 'LoomAgent slice C: a Command variant runs its own image and entrypoint with a workspace, /config and Secret files'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: charts/loom-agents
- confidence: cited
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-operator
- confidence: cited
  path: crates/loom-resource
- confidence: cited
  path: deploy/crd
- confidence: cited
  path: docs/design/loom-agent-crd-deployment.md
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: generated/rust/loom
- confidence: cited
  path: website/data/ess
- confidence: cited
  path: website/docs/reference
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T09:58:54Z", actor: "human:timo", revision: 15}
- {from: "proposed", to: "active", at: "2026-10-26T09:58:54Z", actor: "human:timo", revision: 16}
---
## Outcome

A `LoomAgent` can run a program other than `b10x-loom agent serve`: an image of its own (a takt
image) with that image's entrypoint or a given one, a workspace volume for a repository checkout,
the `loom.agent/1` document mounted at `/config/agent.yaml`, and the Secret's keys as environment
or as files. The serve path renders as before. Design `docs/design/loom-agent-crd-deployment.md`
§ 9a, slice C.

- `loom.resource.LoomAgentSpec` gains `command` (`loom.resource.CommandSpec`: optional
  `entrypoint` and `args`, each a list of strings) and `workspace` (`loom.resource.Workspace`:
  `mountPath`, optional `claimName`); `loom.resource.SecretRef` gains an optional `mountPath`.
  `spec.image` (`repository`, `tag`) is already declared and keeps its shape.
- The variant is `loom.resource.AgentVariant` (`AgentServe`, `Command`): `Command` when
  `spec.command` is present, `AgentServe` otherwise. `RecordCheck` takes it and its `running` and
  `refused` outcomes record it as `status.variant`; `ApplyLoomAgent`'s `respecified` clears it.
- The controller, for `Command`: no init container; the container runs `spec.image` (else the
  default image) with `command` = `entrypoint` and `args` = `args`, each left to the image when
  absent; the ConfigMap `<name>-spec` read-only at `/config` (`agent.yaml`, `routes.yaml`,
  `connectors.toml`); an emptyDir at `/var/lib/agent` as `HOME` and one at `/tmp`. Admission for
  `Command` runs the credential walk and the `loom.agent/1` decoder over `spec.agent`, not the
  serve check against the plugin catalog, since `agent serve` does not read it.
- For both variants: `spec.workspace` mounts an emptyDir, or the PersistentVolumeClaim
  `claimName`, read-write at `mountPath`; `spec.secretRef.mountPath` mounts the Secret read-only
  as files there, beside the environment it already gives; the pod gets `fsGroup` 10001 when it
  mounts either. Admission refuses a mount path that is not absolute and plain or overlaps another
  mount (`MountPathInvalid`), a `claimName` outside the configuration's new `allowedClaimNames`
  (`ClaimNotAllowed`), and a credential in `spec.command` (`SecretLiteral`).
- Conditions: `Checked` is `True` with reason `SpecServeable` for `AgentServe` and `SpecAdmitted`
  for `Command`.
- `charts/loom-agents` gains `allowedClaimNames`; the CRD, the chart's copy, the generated Rust
  and the ESS reference pages are regenerated; the design's § 9a records what was built.

## Why

Timo, 2026-10-26: "loom deployable agents + takt working as auto developer". The orchestrator's
plan step S2 (approved under the grant, `approval-record:loom-agent-command-variant-filing`):
§ 9a slice C now, § 10 row 7 option A, confinement = the pod for the local trial; Substrate
confinement is a later slice.

## Design

- The `/config` contract: the `spec.agent` document is written to `/config/agent.yaml` as the
  YAML of the document the API server stored, the same bytes the serve variant gets at
  `/etc/agent/agent.yaml`; `routes.yaml` and `connectors.toml` beside it as for serve.
- An empty `spec.routes` with no model route configured is refused `NoModelRoute` for
  `AgentServe` only; a `Command` resource then gets an empty `routes.yaml`.
- A mount path is plain when it is absolute, UTF-8 with no control character, has no empty, `.`
  or `..` component and is not `/`; it overlaps another when one is the other or beneath it. The
  reserved mounts are `/etc/agent` (serve), `/config` (command), `/var/lib/agent` and `/tmp`
  (command).
- `allowedClaimNames` is a list in the controller's `config.yaml`, absent or empty admitting none,
  as `allowedSecretNames`.
- Not in ESS: the rendering itself is a function of the spec with no state, held by golden
  manifests in `crates/loom-operator/tests/render.rs`.

## ESS first

The first commit changes only `ess/domains/resource.yaml`. On it `task drift` and `task crd-drift`
fail, naming `generated/rust/loom/` and `deploy/crd/loomagents.loom.beyond10x.dev.yaml`. Later
commits make both pass without changing `ess/`.

## Acceptance

Named conformance scenarios of the `loom.resource` suite, each executed and passed by
`task conform`: `loom.resource.RecordCheck/outcome/running` and `/refused` (each records
`variant`), `loom.resource.ApplyLoomAgent/outcome/respecified` (clears it), and every other
`loom.resource` scenario of the base, none skipped.

And:

- `task check` exits 0; `ess specify validate --path ess --strict-requires` exits 0 on ESS 0.69.2.
- `crates/loom-resource/tests/crd.rs`: the CRD declares `spec.command.entrypoint`,
  `spec.command.args`, `spec.workspace.mountPath`, `spec.workspace.claimName`,
  `spec.secretRef.mountPath` and `status.variant`, none of them required but `mountPath` of
  `workspace`.
- `crates/loom-operator/tests/render.rs`: the serve golden file is unchanged; a takt-shaped
  `LoomAgent` (an image, `command` with `args` only, a workspace on a claim, a Secret with a
  mount path) equals a new golden file: no init container, no `command`, the given `args`,
  `/config`, the workspace, the Secret volume, `HOME`, `/tmp` and `fsGroup`.
- `crates/loom-operator` tests: a credential in `spec.command`, a relative or overlapping mount
  path and a claim outside `allowedClaimNames` are refused before any child; a `Command`
  resource whose `spec.agent` names a protocol outside the plugin catalog is admitted; its status
  is `variant: Command` with `Checked` reason `SpecAdmitted`.

## Not in scope

The takt image and entrypoint (takt, step S1); Substrate confinement (`loom-child-confinement`);
the kind cluster and Secrets (steps S3, S4); a generic environment or header passthrough; a
`deploy.kubernetes.image_repository` read.
