---
format: aep.planning-md/3
id: story:loom-operator
kind: story
status: implemented
title: 'The Loom operator: crates/loom-operator reconciles LoomAgent into a pod, and charts/loom-agents installs it'
relations:
- decomposes: epic:loom-sdk-for-products
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: README.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: charts/loom-agents
- confidence: cited
  path: crates/loom-conformance
- confidence: cited
  path: crates/loom-docs
- confidence: cited
  path: crates/loom-executor/tests/crate_names.rs
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
  path: website/data/status.json
- confidence: cited
  path: website/docs/reference
revision: 24
transitions:
- {from: "draft", to: "proposed", at: "2026-10-25T18:23:57Z", actor: "human:timo", revision: 21}
- {from: "proposed", to: "active", at: "2026-10-25T18:23:57Z", actor: "human:timo", revision: 22}
- {from: "active", to: "implemented", at: "2026-10-25T20:44:00Z", actor: "human:timo", revision: 24, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A `LoomAgent` resource applied to a cluster runs as one pod, reconciled by Loom's own controller,
and the chart that installs the CRD and the controller lives in this repository, versioned with the
Loom tag:

- The `loom.resource` domain (`ess/domains/resource.yaml`) gains what the controller owns of the
  resource's status: `deployment` (what it last read of the child Deployment) and `conditions`
  (the Kubernetes conditions `Checked` and `Available`, the controller's projection of `state`,
  `check` and `deployment`), and `ObserveDeployment` records every observation, so a Running
  resource whose Deployment is available shows it. The CRD's `status` schema carries both,
  projected by `loom-xtask crd-generate` as before.
- `b10x-loom-operator` (`crates/loom-operator`, package `b10x-loom-operator`) is a kube-rs
  controller: it watches `LoomAgent` in one namespace and reconciles each resource into a
  ServiceAccount, a ConfigMap (`agent.yaml`, `routes.yaml`, `connectors.toml`), a Deployment
  (replicas 1, `Recreate`, an init container `b10x-loom agent check --spec /etc/agent/agent.yaml`,
  the container `b10x-loom agent serve --spec /etc/agent/agent.yaml --state /var/lib/agent` from
  `spec.image` or the configured default), a NetworkPolicy and, with `expose.a2a`, a Service, every
  child owned by the resource so deletion takes them. Before any child is rendered the controller
  runs the spec check in-process (`b10x_loom_agent::check_text`, the library behind
  `b10x-loom agent check`) over `spec.agent` and the credential walk over `spec.routes`: a refusal
  is recorded as `status.check` and `status.state: Refused`, and no child exists for that
  resource. The lifecycle is the generated behaviour of `loom.resource` over
  `b10x_loom_resource::LoomAgentRecord`; the controller publishes the record's row as the
  resource's status (`observedGeneration`, `state`, `check`, `deployment`, `conditions`).
- The command line `b10x-loom-operator` (clap derive): `run --config <file>` (the controller, in
  cluster or from a kubeconfig), `render --resource <file> [--config <file>]` (the children of one
  `LoomAgent` document as one multi-document YAML, nothing applied) and `crd` (the CRD, the same
  bytes as `deploy/crd/loomagents.loom.beyond10x.dev.yaml`).
- `charts/loom-agents/` is the Helm chart that installs the CRD (`crds/`, a byte-identical copy of
  `deploy/crd/`, drift-checked), the controller Deployment, its ServiceAccount, Role and
  RoleBinding (namespaced, the child kinds and `loomagents` + `loomagents/status`, no secrets
  verb), and its ConfigMap-mounted `config.yaml` from the values: controller image, default agent
  image, the model route (an in-cluster signing proxy by URL and protocol, no credential), allowed
  Secret names and egress hosts. `Chart.yaml` `version` and `appVersion` are the workspace
  version.
- `task check` gains `task chart-check` (`helm lint` and `helm template` over the chart, with
  CI installing Helm as it installs `ess` and Task) and the controller's tests: golden manifests
  per example spec, the reconcile loop against a fake API server, and "a secret literal is refused
  before any child is created".
- The site gains the generated `b10x-loom-operator` CLI reference; README.md, AGENTS.md and
  `CHANGELOG.md` describe the controller and the chart; the design document records where the
  implementation differs from § 5 and § 8.

## Why

Design `docs/design/loom-agent-crd-deployment.md` § 9 step 6 (`crates/loom-operator`, `kube` +
`k8s-openapi` in Loom's lock, design D2, D9) and the operator's decisions of 2026-10-25 on its
§ 10: row 3 = B (the CRD and the controller chart live in beyond10x/loom, versioned with the Loom
tag; the deployment repository only references it by version) and row 5 = A (the model route is an
in-cluster signing proxy, configured by URL, no credential in the chart). Step 5
(`story:loom-agent-resource-domain`, PR 129) left `status.deployment` and `status.conditions` to
this step (its D7). Approval: `approval-record:loom-operator-filing`.

## Design

- ESS first. `status.deployment` is `Optional<loom.resource.DeploymentStatus>` (`availability`),
  set by every `ObserveDeployment` outcome that reads a Deployment and cleared on `respecify`.
  `still-running` and `still-degraded` become moves over the self-transitions `observe-running`
  (Running to Running) and `observe-degraded` (Degraded to Degraded), emitting
  `loom.resource.LoomAgentObserved`, because an `updates:` outcome beside `moves:` under one
  subject predicate makes the whole command a hand-written obligation on ESS 0.69.0 (probed), and
  a `preserves:` outcome sets nothing. The view `LoomAgents` projects `deployment` and
  `conditions`, which the preserving outcomes of `ApplyLoomAgent` need.
- `status.conditions` is `Optional<List<loom.resource.Condition>>` (`type` `Checked` or
  `Available`, `status` `'True'`, `'False'` or `Unknown`, quoted so YAML does not read them as
  booleans, `reason`, `message`). No command sets it: a `sets:` value on ESS 0.69.0 admits no list
  (a list is read only as the `via:` of a related read; `[]` needs `ess/31`). The controller
  computes the list from the row's `state`, `check` and `deployment` when it publishes the status,
  pinned by a unit test per state, and the declaration gives the CRD its schema. Reported as an
  ESS gap; the workaround is marked in `resource.yaml` with the ESS version.
- The spec check runs in the controller, in-process, through `b10x_loom_agent::check_text` over
  the YAML of `spec.agent`, before any child is rendered; its result is `RecordCheck`. The design
  (§ 5.1) read `status.check` from the pod's init container; the init container stays in the
  Deployment, so the pod's own binary refuses a spec too, but its exit code is not read back: the
  controller's check is the same library at the same version, needs no pod read permission, and
  answers before a child exists, which § 8's test requires. `spec.routes` is walked for a
  credential by the same library: `check_text` over the routes YAML answers `SecretLiteral` when it
  finds one and `Malformed` otherwise, since the credential walk runs before the decoder and the
  walker is not public; nothing in `crates/loom-agent` changes in this story.
- The controller's `config.yaml`: `namespace`, `defaultImage: {repository, tag}`,
  `defaultResources: {cpu, memory}`, `modelRoute: {url, protocol, model}`, `allowedSecretNames`
  and `allowedEgressHosts`, both lists, an empty list admitting none. A resource whose
  `spec.routes` is an empty object gets the default routes document (`format: llm.routes/1`, one
  anonymous endpoint at the model route's URL and protocol, one route per declared role to its
  model); an empty object with no model route configured, a `secretRef` outside
  `allowedSecretNames`, an egress host outside `allowedEgressHosts` or a `deploy.kubernetes.namespace`
  other than the resource's are refusals recorded through `RecordCheck` with exit code 3 and a
  message naming the member, before any child. A `spec.routes` that is not empty is written as
  given.
- Children carry the resource as owner (`controller: true`, `blockOwnerDeletion: true`) and are
  applied by server-side apply with the field manager `b10x-loom-operator`; a Refused resource has
  no children (existing ones of an earlier generation are deleted). The NetworkPolicy denies by
  default and admits egress to DNS, to each `deploy.kubernetes.egress` entry (a namespace
  selector entry as a peer; a host entry by port only, since a NetworkPolicy names no host) and
  to the model route's host port, and ingress from each `deploy.kubernetes.ingress` entry.
- The record lives in the controller's memory: after a restart every resource is applied again as
  `created` at its cluster generation, checked again, and its status rewritten; the cluster holds
  the published view.
- `kube` 4.2 (`runtime`, `derive`, `client`, `rustls-tls`) and `k8s-openapi` 0.28 (`v1_34`) are
  the controller's dependencies, pinned in `Cargo.lock`; the `LoomAgent` kube type is declared
  with `schema = "disabled"`, since the schema is the one projected from `ess/`, and a test holds
  its group, version, kind and plural to `b10x_loom_resource::crd`'s constants.
- The chart is rendered by Helm, not reimplemented in Rust: `task chart-check` runs `helm lint`
  and `helm template` (with the default values and with overrides), CI installs Helm 3.19.0 by
  checksum as it installs `ess`, and `crates/loom-operator/tests/chart.rs` reads the rendered
  output and holds the CRD copy to `deploy/crd/` byte for byte, `Chart.yaml`'s versions to the
  workspace version, the Role to the design's verbs and the controller's image and config to the
  values. `kubeconform` is not run: it is not installed here and fetches schemas from the network.
- The values file carries no comments (the operator's rule for every values and manifest file).

## ESS first

The first commit changes only `ess/domains/resource.yaml`. On it `task drift` fails, naming
`generated/rust/loom/src/resource.rs` and the files a fresh synthesis changes, and `task crd-drift`
fails, naming `deploy/crd/loomagents.loom.beyond10x.dev.yaml`; the runs are recorded in the wave
record. Later commits make both pass without changing `ess/`.

## Acceptance

Named conformance scenarios of the suite `ess verify conform synthesize --path ess --suite-format
5 --scenarios ess` writes, each executed and passed by `task conform` against `b10x-loom-resource`:

- `loom.resource.ObserveDeployment/outcome/still-running` and `/still-degraded` as moves that set
  `deployment` and emit `LoomAgentObserved`; `/degraded`, `/recovered`, `/no-deployment`,
  `/unknown` as before;
- `loom.resource.ApplyLoomAgent/outcome/respecified` with `deployment` cleared; every other
  `loom.resource` scenario of the base, none skipped (`ess/SKIPPED.md` names none).

And:

- `ess specify validate --path ess --strict-requires` exits 0 with ESS 0.69.0; `ess verify conform
  synthesize` reports 0 refusals; `task ess-gate`, `task drift`, `task crd-drift`,
  `task no-hand-model`, `task conform`, `task chart-check` and `task check` exit 0.
- `crates/loom-executor/tests/crate_names.rs` expects 26 packages, `b10x-loom-operator` among them.
- `crates/loom-operator/tests/render.rs`: `render` over `examples/agents/operator.yaml` wrapped as
  a `LoomAgent` equals the committed golden file `crates/loom-operator/tests/golden/
  platform-operator.yaml` (a ServiceAccount, a ConfigMap with the three files, a Deployment with
  the init container, the serve command, the mounts, the a2a port, the probes and the security
  context of design § 4, a NetworkPolicy, a Service), and a resource without `expose.a2a` renders
  no Service and no probes.
- `crates/loom-operator/tests/reconcile.rs`: against a fake API server (kube's mock client over
  `tower-test`), a created resource is applied, checked and its children applied in one reconcile,
  its status patched to `Running` with `observedGeneration` and `conditions`; a Deployment
  observed unavailable moves it to `Degraded` and available back to `Running`; a resource holding
  a secret literal in `spec.agent` or `spec.routes` is patched to `Refused` with `check` naming
  the member and no child is applied (no apply call reaches the fake server); a new generation
  returns it to `Pending` and runs the check again.
- `crates/loom-operator/tests/cli.rs`: `b10x-loom-operator crd` prints
  `deploy/crd/loomagents.loom.beyond10x.dev.yaml` byte for byte; `render` with a document that is
  not a `LoomAgent` exits 2 naming the file.
- `crates/loom-operator/tests/chart.rs`: as the Design section says.
- `website/docs/reference/operator-cli.md` is generated by `task docs-generate` and
  `task docs-check` exits 0; `task website` builds.

## Not in scope

The deployment repository's image builds, environment resources and CI (design steps 7-8); the
SRE items (step 9); a live cluster run (step 10); the Slack slice (step 11); a public credential
walker in `crates/loom-agent` (the concurrent wave 2026-10-25-agents owns that crate);
`kubeconform`; leader election or more than one namespace per controller instance; a release.
