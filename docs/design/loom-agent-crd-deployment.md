# Design: CRD-driven Loom agent deployment for a downstream platform

Date 2026-10-25. Status: draft, design only; nothing in this document is implemented or deployed.
Every `file:line` is at the head named in § 0; "I don't know" means no file or command said.

The downstream platform is a multi-tenant product that runs customer-owned AI agents; it is a
consumer of Loom, not a part of it. Its name, hosts and internal paths are left out of this document
on purpose; the platform-side copy of this design names them.

## 0. The ask and the heads read

Timo, 2026-10-25 (verbatim, the platform's name replaced by "[the platform]"):

> "reconcile and enumerate steps needed to get the operational agent build + deployed using only
> loom-sdk from beyond10x - create the steps for the most minimal full vertical slice: agent loop,
> some data fetching, some reasoning + action. defined via loom/ymal(?), deployed via deployment
> repo of [the platform]"

> "technically there oculd justr in deployment be the helm stuff also for [the platform] itself
> btw, we want to refactor that soon ... also there could just be a config map injecting some YAML
> into a container which holds and reads just k8s custom resource defintions ... of agents ... and
> for each of them starts one pod"

> "send me a design for this CRD driven loom agent deployment via [the platform] and tag me,
> design only"

> (after revision 1) the operational agent "should have 0 dependencies to actual [the platform],
> only beyond10x components"

The 2026-10-22 ask behind the operator agent ("ideally an agent would just be a YAML file", "Forget
about the two containers", "this agent is just a canon protocol") is the one the Loom Kubernetes
design draft of 2026-10-22 (`loom-kubernetes.md`, not yet in this repository) answers.

| Source | Head / state | Read |
|---|---|---|
| `beyond10x/loom` | main `858643a6`, release 0.40.0 (`README.md` "Status") | README, `crates/loom-sdk/{Cargo.toml,src/lib.rs}`, `examples/agents/operator.yaml`, `ess/domains/agent.yaml`, `.engineering/planning/story/agent-*.md`, `approval-record/loom-kubernetes-filing.md` |
| Loom Kubernetes design | `loom-kubernetes.md` (draft, 281 lines, 2026-10-22) | § 1 D1-D10, § 3 spec, § 4 build flow, § 6 security, § 7 milestones, § 9 Q1-Q9 |
| `beyond10x/connectors` | `e26481df0` (2026-10-20); installed CLI 0.56.0 | `README.md:199-203` (custody), adapters directory |
| the downstream platform | `main` on 2026-10-25 | its backend Dockerfile, chart, CI file, `Cargo.lock` |
| the platform's deployment repository | one commit of 2026-10-22, README template only | clone in session scratch |
| the platform's operator-agent repository | no commits | clone attempt |
| the environment configuration repository (ArgoCD applications and values of the dev environment) | default branch | the dev environment's application file and values file |
| the dev cluster, the platform's dev namespace | live, 2026-10-25 13:2xZ | `kubectl get crd`, `get applications -n argocd`, `get deploy`, `get svc -n monitoring`, `get ds -n kube-system` |

## 1. Decision summary

Constraint (Timo, 2026-10-25): the agent and its controller depend on beyond10x components only
(Loom, Connectors, llm). The platform's repositories hold deployment configuration (its deployment
repository), the end-to-end scenario (its e2e-tests repository) and nothing the agent or the
controller runs; the platform's workspace, images and specification gain no code, dependency or
domain for it.

| # | Decision | Why |
|---|---|---|
| D1 | An agent is one Kubernetes custom resource, kind `LoomAgent`, whose `spec.agent` is the `loom.agent/1` document verbatim (Loom's ESS domain `loom.agent`, `ess/domains/agent.yaml`, story `agent-domain` implemented). No second agent schema. | "an agent would just be a YAML file"; the typed form exists and is exported as `loom_sdk::agent` (`crates/loom-sdk/src/lib.rs:60`). |
| D2 | One controller, `loom-operator`, reads `LoomAgent` resources in its namespace and owns one Deployment (one pod, one container) per resource, plus the ConfigMap, ServiceAccount, NetworkPolicy and optional Service that pod needs. Its own configuration is one ConfigMap-mounted YAML file. | The ask: "a config map injecting some YAML into a container which holds and reads just k8s custom resource definitions of agents and for each of them starts one pod". |
| D3 | The pod runs Loom's own command line, not a platform binary: `b10x-loom agent serve --spec /etc/agent/agent.yaml --state /var/lib/agent` once Loom story `agent-serve` lands; until then there is no spec-driven host to run (§ 4). The platform contributes the deployment repository, nothing agent-side. | `loom-kubernetes.md` D3 (`run_plugin` is Loom's one long-running loop); `b10x-loom-sdk` 0.40.0 does not re-export the plugin host (`crates/loom-sdk/Cargo.toml:14-22` lists no `b10x-loom-plugin`), so "only loom-sdk" on the platform side cannot build the loop without duplicating Loom's M1. |
| D4 | The agent image is built in the deployment repository from the Loom and Connectors tags (`cargo install --locked --git … --tag …` in a builder stage), pushed to the dev registry as `loom-agent:<loom tag>`. | Loom publishes no release binary (`loom-kubernetes-filing.md` D4; `.github/workflows/release.yml` creates no artifact). |
| D5 | Deployment path: the platform's deployment repository holds the CRD, the controller chart, the image builds and the per-environment `LoomAgent` resources; ArgoCD applies them (a second Application beside the platform's). The platform's own chart is untouched by this design; moving it into the deployment repository is its own story. | The ask names "the deployment repo of [the platform]"; that repository exists and is empty; the cluster is ArgoCD-synced. |
| D6 | The first live slice reads over `http` bindings (Loki, Prometheus in `monitoring`) and answers over A2A; Slack reads and writes come second. | Connectors keeps credentials in the Secret Service keyring on Linux (`connectors/README.md:199-203`), which a container has not got; `loom-kubernetes.md` Q3 recommends a file-backed custody in Connectors and no story is filed. `http` bindings need no credential. |
| D7 | Model route: an llm `routes.yaml` in the ConfigMap; the credential, if any, from one Secret the resource names. Candidate endpoints in § 4. | `loom-kubernetes.md` D6; the platform's text cognition already runs on Bedrock through EKS Pod Identity. |
| D8 | Spec first: the CRD's `openAPIV3Schema` is generated from Loom's `loom.agent` domain, and Loom's `ess/domains/` gains a domain for the `LoomAgent` resource lifecycle (beside `loom.agent`) before the controller is written. The platform's specification repository holds nothing of it. | Timo 2026-10-24: "any architectural or model change is a spec change first". |
| D9 | The controller is a beyond10x component: a Rust crate in this repository (`crates/loom-operator`, binary `loom-operator`) with `kube` + `k8s-openapi` as its own dependencies, released as its own image from the Loom tag. The platform's workspace, backend image and `Cargo.lock` do not change. | Timo 2026-10-25: the operational agent has 0 dependencies on the platform, only beyond10x components. Loom's gate and release train apply; the dependency add is Loom's decision. |
| D10 | Nothing in this design is built until the four Loom stories in § 3 are implemented and released; the Loom-side work (D8, D9) and the deployment-side work (D5) can start now against the spec and a fake pod image. | Reconciliation in § 3: today 0 of the 4 Loom milestones the slice needs is implemented. |

## 2. What exists today

| Layer | Exists | Source |
|---|---|---|
| Loom agent spec as data | yes: domain `loom.agent`, `AgentSpec` in `loom_sdk::agent`, example `examples/agents/operator.yaml` with a `deploy.kubernetes` block | `ess/domains/agent.yaml:432` (`AgentSpec`), `crates/loom-sdk/src/lib.rs:60`, story `agent-domain` implemented |
| Loom long-running host | only `b10x-loom plugin run slack-handler --config <SlackConfig json> --state <dir>` (reads Slack through the `connectors` CLI, proposes replies, sends nothing) | `README.md` "Run the command line", last paragraph |
| `b10x-loom agent check / serve / project` | no: stories `agent-check`, `agent-serve`, `agent-project-kubernetes` are `draft` | `aep plan artifact list` in this repository |
| `http` read binding, A2A exposure | no: `agent-http-binding`, `agent-a2a` are `draft` | same |
| Loom release binaries or image | no | `loom-kubernetes-filing.md` D4 |
| Connectors headless custody | no: keyring Secret Service on Linux; no file or env custody | `connectors/README.md:199-203` |
| Connectors adapters the agent would use | yes: loki, prometheus, grafana, alertmanager, slack, kubernetes | `connectors/adapters` |
| Platform chart | one Deployment (backend + frontend), a ServiceAccount with Pod Identity, ConfigMap, smoke CronJob | the platform's chart templates |
| Platform deploy to the dev namespace | one ArgoCD Application syncs the environment configuration repository at `HEAD`; the platform is a subchart of the environment's umbrella chart; the platform's CI deploy job triggers a rollout restart only | the environment's application file and values file, the platform's CI file |
| Deployment repository | README template, no CI, no chart | clone |
| Operator-agent repository | empty | clone: "does not have any commits yet" |
| Agent CRDs in the dev cluster | none; CRDs present are `*.argoproj.io` and `prometheusagents.monitoring.coreos.com` | `kubectl get crd` filtered on agent, loom, argo, flux |
| In-cluster data sources | `loki.monitoring.svc:3100`, `monitoring-kube-prometheus-prometheus.monitoring.svc:9090`, `monitoring-kube-prometheus-alertmanager.monitoring.svc:9093` | `kubectl -n monitoring get svc` |
| In-cluster model access | a SigV4 signing proxy for Bedrock runs as a Deployment in the dev namespace; its wire protocol: I don't know | `kubectl get deploy` in the dev namespace |
| NetworkPolicy enforcement | CNI is `aws-node`; one NetworkPolicy exists in the dev namespace; whether the VPC CNI network-policy agent is enabled: I don't know | `kubectl -n kube-system get ds`, `kubectl get networkpolicy` |
| `kube` crates in the platform workspace | 0 | `grep -c '^name = "kube' Cargo.lock` there |

## 3. Reconciliation against Loom: what the slice needs from Loom

The minimal slice (agent loop, data fetching, reasoning, action) maps onto Loom like this:

| Slice part | Loom piece | State in Loom 0.40.0 | Story |
|---|---|---|---|
| Agent loop | `run_plugin` driven by the spec's `loops.outer` (poll) and `loops.inner` (turn on a Canon protocol) | host exists for the compiled Slack plugin only | `agent-serve` (draft, M1) |
| Spec refused before anything runs | `b10x-loom agent check --spec` | no | `agent-check` (draft, M0) |
| Data fetching | `http` binding, GET only, `allow_paths`, `max_bytes` | no | `agent-http-binding` (draft, M2) |
| Reasoning | the chooser path: classifier + agent model through llm routes | exists (`run --config`, `plugin run --catalog`) | none needed |
| Action | `reply.send` through Connectors (Slack), or the A2A answer | `reply.send`: no (`operator-reply@1` protocol in M0/M1); A2A: no | `agent-serve`, `agent-a2a` (draft, M5) |
| Packaging | `b10x-loom agent project --target kubernetes` writes manifests | no | `agent-project-kubernetes` (draft, M3); **not needed here**: the controller renders the manifests (D2), so M3 is superseded for this path |

So 4 Loom stories gate the slice: `agent-check`, `agent-serve`, `agent-http-binding`, `agent-a2a`.
They are filed under `epic:loom-sdk-for-products` in this repository's store, all `draft`. One
Connectors story is needed for the Slack variant and is not filed anywhere: headless credential
custody (`loom-kubernetes.md` Q3 option A).

## 4. The per-agent pod

```
Pod <agent id>                                   (Deployment, replicas 1, strategy Recreate)
  container loom-agent
    image  <registry>/loom-agent:<loom tag>          (built in the deployment repository, § 6)
    cmd    b10x-loom agent serve --spec /etc/agent/agent.yaml --state /var/lib/agent
    env    from Secret <spec.secretRef>  (model key if the route needs one; nothing else)
    mounts /etc/agent      ConfigMap <agent id>-spec  (agent.yaml, routes.yaml, connectors.toml), read-only
           /var/lib/agent  emptyDir                    (state directory; cursors rebuilt from lookback_minutes)
    ports  8080 a2a        (only with expose.a2a)
    probes GET /ready, GET /live on the a2a port       (loom story agent-a2a); without a2a: exec `b10x-loom agent check`
    securityContext runAsNonRoot, runAsUser 10001, readOnlyRootFilesystem, allowPrivilegeEscalation false, drop ALL
  serviceAccountName <agent id>, automountServiceAccountToken false, no Role
```

This is `loom-kubernetes.md` § 4's manifest set, rendered by the controller instead of by
`b10x-loom agent project`. The spec's `deploy.kubernetes` block (`examples/agents/operator.yaml`
lines `deploy:` to the end) supplies namespace, service account, image repository, resources,
egress and ingress; the controller reads only that block and `expose.a2a`, and passes the rest of
the document through untouched.

Model route candidates for `routes.yaml` (`llm.routes/1`), to be settled by the first manual run:

| Option | Credential | Known | Unknown |
|---|---|---|---|
| A. the in-cluster Bedrock signing proxy | none (SigV4 signed by the proxy under Pod Identity) | the Deployment exists in the dev namespace | its wire protocol and whether llm's `routes.rs` endpoint `protocol` values cover it |
| B. Anthropic API, `anthropic/claude-haiku-5-5` | `env:ANTHROPIC_API_KEY` from a SealedSecret in the environment configuration repository | `loom-kubernetes.md` D6; the platform's provider keys follow this SealedSecret pattern | the model id is unverified against the provider (D6, Q8) |
| C. OpenRouter | `env:OPENROUTER_API_KEY` | the platform's Secret already reserves that key; llm has an openrouter adapter in Connectors, llm routes: I don't know | |

Default A, falling back to B; the route alias (`haiku`) isolates the change.

## 5. The resource and the controller

### 5.1 `LoomAgent` (group `loom.beyond10x.dev`, version `v1alpha1`, namespaced)

```yaml
apiVersion: loom.beyond10x.dev/v1alpha1
kind: LoomAgent
metadata: {name: platform-operator, namespace: <dev namespace>}
spec:
  agent:            # the loom.agent/1 document, verbatim (schema generated from ess/domains/agent.yaml)
    format: loom.agent/1
    agent: {id: platform-operator, name: Platform operator, brief: "…"}
    model: {routes: routes.yaml, roles: [{role: agent, model: haiku}, {role: classifier, model: haiku}]}
    loops: {outer: {…}, inner: {protocol: operator-reply@1, step_budget: 8, model_budget: 16}}
    states: [...]
    bindings:
      - {name: logs, kind: http, base_url: http://loki.monitoring.svc:3100, allow_paths: [/loki/api/v1/query_range, /loki/api/v1/labels], method: GET, max_bytes: 262144}
      - {name: metrics, kind: http, base_url: http://monitoring-kube-prometheus-prometheus.monitoring.svc:9090, allow_paths: [/api/v1/query, /api/v1/alerts], method: GET, max_bytes: 262144}
    policy: {capabilities: [datasource.read, reply.propose]}
    expose: {a2a: {port: 8080, path: /a2a, card: {…}}}
    runtime: {state_dir: /var/lib/agent, confinement: none}
    deploy: {kubernetes: {namespace: <dev namespace>, service_account: platform-operator, image_repository: …, resources: {cpu: 250m, memory: 256Mi}, egress: [...], ingress: [...]}}
  routes:           # llm.routes/1, inline; secrets only as env:/file: references
    endpoints: {...}
    routes: {haiku: ...}
  connectorsConfig: ""   # optional; the connectors-local TOML, only for connectors bindings
  secretRef: {name: platform-operator-secrets}   # optional; env for the container
  image: {repository: <registry>/loom-agent, tag: "0.40.0"}   # optional override of the controller default
status:
  observedGeneration: 3
  state: Running        # Pending | Running | Refused | Degraded, the entity's lifecycle state
  check: {exitCode: 0, message: "checked platform-operator: 3 states, 3 bindings, protocol operator-reply@1"}
  deployment: {availability: Available}   # Available | Unavailable, last read of the child Deployment
  conditions:                             # Checked and Available, computed from state, check, deployment
    - {type: Checked, status: "True", reason: SpecServeable, message: "checked platform-operator: …"}
    - {type: Available, status: "True", reason: DeploymentAvailable, message: the Deployment reports Available}
```

`status.state` is the `loom.resource` lifecycle state this design first called `phase`.
`status.check` comes from the controller's own check (§ 5.2), not from the pod. `status.deployment`
and `status.conditions` were added with the controller (`story:loom-operator`): `deployment` is set
by every `ObserveDeployment` outcome that reads the Deployment and cleared when the spec changes;
`conditions` is declared in `ess/domains/resource.yaml` for the CRD's schema, but ESS 0.69.0 admits
no list as a `sets:` value, so the controller computes it from `state`, `check` and `deployment`
when it publishes the status (`crates/loom-operator/src/status.rs`).

Validation layers, in order: the CRD schema (structural, from the ESS domain); the controller's
admission, before any child exists, which runs `b10x_loom_agent::check_text` (the library behind
`b10x-loom agent check`) over `spec.agent`, refuses a credential in `spec.routes` or in
`spec.connectorsConfig` (its decoded TOML, the words of each decoded string, and the words of the
raw text the ConfigMap receives, comments included), reads `spec.routes` with llm's strict
`llm.routes/1` parser (so an `auth.secret` that is a value and not a reference is refused), holds
`secretRef`, `deploy.kubernetes.namespace` and the egress hosts to the controller's configuration
(an egress entry naming neither a host nor a namespace is refused), and refuses a child name that
an object it does not control already holds, its answer landing in
`status.check`; then `b10x-loom agent check` again in the pod's init container, whose exit code is
not read back.

### 5.2 `loom-operator`

| Aspect | Design |
|---|---|
| Code | `crates/loom-operator` (package and binary `b10x-loom-operator`) in this repository; `kube` 4.2 (runtime, derive, client, rustls-tls) and `k8s-openapi` 0.28 (`v1_34`) as its dependencies (D9) |
| Image | its own image `<registry>/loom-operator:<loom tag>`, built in the deployment repository from the Loom tag beside the agent image (§ 6); the platform's backend image is untouched |
| Config | `/etc/loom-operator/config.yaml` from a ConfigMap: `namespace`, `defaultImage`, `defaultResources`, `modelRoute` (`url`, `protocol`, `model`; § 10 row 5 A), `allowedSecretNames`, `allowedEgressHosts`, `allowedClaimNames` (slice C) |
| Loop | watch `LoomAgent` in one namespace; reconcile = check the spec in-process, then render the children (ConfigMap, ServiceAccount, Deployment, NetworkPolicy, Service) as server-side apply with the resource as owner; status from the controller's check and the child Deployment's `Available` condition; delete = garbage collection through owner references |
| RBAC | namespaced Role: `loomagents` + `loomagents/status` (get, list, watch, update, patch), `loomagents/finalizers` (update), `configmaps`, `serviceaccounts`, `deployments`, `services`, `networkpolicies` (create, update, patch, get, list, watch, delete). No ClusterRole; the CRD itself is installed from the chart's `crds/` (§ 10 row 3 B), not by the controller |
| Replicas | 1, `Recreate`; no leader election |
| Platform coupling | none: no platform code, crate or image; the controller never calls the platform's management API and the platform never reads `LoomAgent`. The two meet only in the dev cluster namespace and in the deployment repository |
| Tests | render tests against golden manifests per spec example; a kube-apiserver fake (`kube`'s mock client) for the reconcile loop; `clippy -D warnings`; an ESS conformance suite for the resource lifecycle domain (D8) |

Where the implementation (`story:loom-operator`) differs from this table and § 4, and why:

- The spec check runs in the controller, through `b10x_loom_agent::check_text`, before any child
  is rendered, and its answer is `status.check`. Reading the init container's exit code would need
  a read permission on Pods and would answer only after a Deployment exists, which § 8's "refused
  before any child is created" excludes. The init container stays, so the pod refuses a spec on
  its own too.
- `events` is not in the Role and the controller records no Event. `loomagents/finalizers`
  (update) is in it, because an owner reference with `blockOwnerDeletion` needs it.
- Without `expose.a2a` the Deployment has no probe; § 4's exec probe running `agent check` is not
  rendered, since the check holds for the spec's whole life and adds nothing as a liveness signal.
- `deploy.kubernetes.image_repository` is not read: the image is `spec.image` or the configured
  `defaultImage`.
- The NetworkPolicy admits egress to DNS in `kube-system`, to each egress entry (a host entry by
  port only, since a NetworkPolicy names no host) and to the model route's port only when the
  controller wrote the default routes, and ingress from each ingress entry.
- The chart `charts/loom-agents` lives in this repository, versioned with the Loom tag (§ 10
  row 3 B): it installs the CRD, the controller's Deployment, ServiceAccount, Role, RoleBinding and
  ConfigMap, and `task chart-check` lints and renders it.
- "Server-side apply with the resource as owner" applies only over a child the resource already
  controls or that does not exist. Before applying, the controller reads each child: one held by
  any other controller, or by none (the controller's own ServiceAccount, `default`), refuses the
  resource `ChildNotOwned` instead of being taken over and later garbage-collected with it. That
  refusal depends on the cluster, not the spec, so the children are read again on each later
  reconcile at that generation, and once none is someone else's the generation is applied and
  checked as new. A child of a deleted `LoomAgent` of the same name is deleted first, and the
  children are applied on the next reconcile.
- The record is keyed by name, so the controller also holds each name's uid: a resource deleted
  and created again under its name is applied as new, not `unchanged` or `stale`. A Refused
  resource's children are deleted on every reconcile, so a failed delete is retried.
- "Each child it controls" is found by the resource's uid, not by the names the current spec
  renders: each child kind is listed by `app.kubernetes.io/instance`, and each child the
  controller applied for that uid is read by name, either counting only with a controller owner
  reference to the uid. A Refused resource loses every such child; a Running one loses each such
  child this generation does not render (the ServiceAccount an earlier generation named, a Service
  once `expose.a2a` is gone) after its children are applied.
- An egress entry naming neither `host` nor `namespace` is refused `EgressHostNotAllowed`: its
  rule would have no peer and admit every destination on its port.

## 6. Deployment through the platform's deployment repository

```
deployment
├── images/loom-agent/Dockerfile          builder: rust:1.96; cargo install --locked --git https://github.com/beyond10x/loom --tag <loom> b10x-loom-cli
│                                                 cargo install --locked --git https://github.com/beyond10x/connectors --tag <connectors> (only for the Slack variant)
│                                         runtime: debian:bookworm-slim, user 10001, the two binaries
├── images/loom-operator/Dockerfile       builder: cargo install --locked --git https://github.com/beyond10x/loom --tag <loom> loom-operator
├── charts/loom-agents/                   CRD, loom-operator Deployment + ServiceAccount + Role + RoleBinding + ConfigMap
├── envs/dev/
│   ├── kustomization.yaml                the chart (helm or rendered) + the agents below
│   └── agents/platform-operator.yaml     one LoomAgent
├── CI file                               verify: helm template + kubeconform + `kubectl apply --dry-run=server`
│                                         build: images to the dev registry on main and tags (the organization's CI templates)
└── README.md, AGENTS.md
```

Applying to the cluster: an ArgoCD `Application` for the agents with `repoURL` = the deployment
repository and `path: envs/dev`, declared beside the platform's application in the environment
configuration repository. That file, the registry repositories `loom-agent` and `loom-operator`
(infrastructure as code) and the ArgoCD repository credential for the deployment repository are
three SRE items. The alternative, a `kubectl apply` job inside the deployment repository's CI, needs
cluster credentials there, which the platform's own CI rules out (its cluster auth lives in a
separate pipeline project).

What does not change: the platform's chart, its deploy job, the umbrella chart's platform block.
Moving the platform's chart into the deployment repository ("we want to refactor that soon") is a
separate story with its own SRE change; this design neither depends on it nor blocks it.

## 7. Security

| Concern | Rule | Enforced by |
|---|---|---|
| Secrets | never in the resource, the ConfigMap or the image; only `env:`/`file:` references; the Secret is named by `spec.secretRef` and created outside the repository (SealedSecret in the environment configuration repository) | CRD schema, controller admission check, `agent check` |
| Blast radius of a pod | no service-account token, no Role, read-only root, non-root, no capabilities, resource limits | rendered manifests |
| Blast radius of the controller | one namespace, one Role listing the five child kinds, no secrets verb | Role |
| Egress | NetworkPolicy from `deploy.kubernetes.egress` plus DNS; default deny; `allowedEgressHosts` in the controller config refuses a host outside the list | controller; enforcement depends on the CNI (unknown, § 2) |
| Reads | `http` bindings GET only, path-prefix allow-list, `max_bytes` | Loom (`agent-http-binding`) |
| Writes | none in slice A; slice B (Slack) in `writes.mode: approval` first (`b10x-loom approve`), `auto` after Connectors' standing scoped approval (Loom `agent-serve` story note) | Loom |
| Prompt injection | every value read is quoted as data in the forced requests | Loom plugin host (`loom-kubernetes.md` § 6) |
| A2A | cluster-internal Service; bearer from the Secret checked by `loom-a2a`; no Ingress in the slice | Loom (`agent-a2a`), rendered Service only |

## 8. Tests (nothing lands without one)

| Layer | Test | Where |
|---|---|---|
| Spec | ESS conformance scenarios for the `LoomAgent` lifecycle domain (create → Pending → Running; refused spec → Refused; delete → children gone) | this repository, `ess/` |
| Controller | golden-manifest render tests per example spec; reconcile loop against a fake API server; "a secret literal is refused before any child is created" | this repository, `crates/loom-operator/tests` (`render.rs`, `reconcile.rs`, `cli.rs`) |
| Chart | `helm lint` and `helm template` with the default values and with overrides (`task chart-check`); the rendered Role, image and `config.yaml` and the CRD copy held by `crates/loom-operator/tests/chart.rs`. `kubeconform` is not run here: it fetches its schemas from the network | this repository, `task check` |
| Image | `docker run <image> b10x-loom agent check --spec examples/operator.yaml` exits 0 in the deployment repository's CI | deployment repository CI |
| Manifests | `helm template` + `kubeconform` + `kubectl apply --dry-run=server` on every change | deployment repository CI |
| End to end | one YAML scenario in the platform's e2e-tests repository: apply the resource, wait for `status.state: Running`, ask over A2A through a port-forward, expect a sourced answer, delete, expect 0 pods | the platform's e2e-tests repository |
| Smoke | the platform's smoke suite unchanged; a sixth check `operator` added only when the agent is part of the release contract | the platform |

## 9. The steps, enumerated (minimal vertical slice, slice A: http reads + A2A answer)

| # | Step | Repository | Owner | Exists | Test |
|---|---|---|---|---|---|
| 1 | Implement and release Loom `agent-check` (M0: `b10x-loom agent check`, protocol `operator-reply@1`) | beyond10x/loom | Loom sessions | draft | Loom conformance (story acceptance) |
| 2 | Implement and release Loom `agent-serve` (M1: spec-driven `run_plugin`, `SpecAuthority`) | beyond10x/loom | Loom sessions | draft | same |
| 3 | Implement and release Loom `agent-http-binding` (M2) | beyond10x/loom | Loom sessions | draft | same |
| 4 | Implement and release Loom `agent-a2a` (M5: card, `message/send`, `/ready`, `/live`) | beyond10x/loom | Loom sessions | draft | same |
| 5 | Loom: domain `LoomAgent` lifecycle in `ess/domains/` + CRD schema projection from `loom.agent` | beyond10x/loom | Loom sessions | no | Loom `task check`, conformance |
| 6 | Loom: `crates/loom-operator` (kube + k8s-openapi in Loom's lock) and its image build in the deployment repository | beyond10x/loom, the platform's deployment repository | Loom sessions + the platform track | no | § 8 controller tests |
| 7 | Deployment repository: `images/loom-agent/Dockerfile` + CI build to the dev registry | the platform's deployment repository | the platform track | README only | image check job |
| 8 | Deployment repository: `charts/loom-agents` (CRD, controller) + `envs/dev/agents/platform-operator.yaml` + verify job | the platform's deployment repository | the platform track | no | render + dry-run jobs |
| 9 | SRE: registry repositories, ArgoCD repo credential, the agents Application, Secret for the model route if B/C | infrastructure as code, environment configuration repository | SRE, change needs Timo's approval | no | ArgoCD sync Healthy |
| 10 | First deployment to the dev namespace; manual run: port-forward to the a2a port, ask "what is the error rate of the platform in the last hour", read the record | cluster | the platform track | – | e2e scenario (§ 8) recorded as evidence |
| 11 | Slice B: Connectors headless custody story (not filed), then the Slack binding + `writes.mode: approval` | beyond10x/connectors, loom | Connectors, Loom | not filed | Loom `agent-serve` scenarios |

Steps 5-8 can start now against the spec and a stub image (`b10x-loom --version` entrypoint);
step 10 waits on 1-4 and 9. Days: I don't know; the Loom stories carry no estimates.

## 9a. A takt delivery as a second agent kind (later slice)

takt (story delivery: branches, pull requests, releases, trials) is an agent of the same stack. It is not part of slice A or B. A later slice C runs a takt delivery as a second `LoomAgent` variant: `spec.image` names a takt image (built in the deployment repository from the takt tag beside `loom-agent`), the pod runs takt's own entrypoint instead of `b10x-loom agent serve`, and `spec.agent` carries takt's run configuration as its `loom.agent/1` document. The controller renders the same children (§ 5.2); what differs is the image, the command and the egress list (a forge over the bot route, a harness session under Substrate). Blocked until takt's `dependency-blocker:loom-child-confinement` clears; until then takt is out of scope here (decision 7).

Built (2026-10-26, `story:loom-agent-command-variant`, decision 7 option A): the variant is `Command`, chosen by the presence of `spec.command` (`entrypoint`, `args`, each left to the image when absent). For the local trial the confinement is the pod itself (user 10001, read-only root, no capability, the NetworkPolicy); Substrate confinement comes later. The contract with the image is that the `spec.agent` document is written verbatim to `/config/agent.yaml` (the ConfigMap mounted at `/config`; keys in the order the API server keeps them, as for `/etc/agent/agent.yaml`). `spec.workspace` adds a read-write volume (a claim in `allowedClaimNames`, else an emptyDir). The controller admits a `Command` spec by the credential walk and the `loom.agent/1` decoder, not the plugin catalog, and `status.variant` with the `Checked` reason `SpecAdmitted` reports it.

Built after the first kind trial (2026-10-26, `story:loom-agent-trial-fixes`): takt needs two Secrets, a model key as environment and a forge key as files, and `b10x-gates` refuses a key file another user owns or that group or others may read. So `spec.secretRef` (`name`) stays the environment and `spec.secretFiles` (`name`, `mountPath`) adds a second Secret as files: an init container from the configured default image runs `b10x-loom agent stage-files`, which copies the keys into a memory emptyDir subdirectory owned by user 10001 (directory 0700, files 0600), mounted read-only at `mountPath`. `runtime.connectors` and `deploy.kubernetes.image_repository` are optional in `loom.agent/1`, so a `Command` spec need not name them; `agent serve` still refuses a spec without `runtime.connectors`.

## 10. Decisions for Timo

| # | Question | Options | Default on silence |
|---|---|---|---|
| 1 | What runs in the agent pod? | A. Loom's CLI image built from tags (D3). B. A platform binary on `b10x-loom-sdk` that re-implements the host loop (duplicates Loom M1, needs `b10x-loom-plugin` as a second git dependency). | A |
| 2 | CRD group | A. `loom.beyond10x.dev`. B. a group under the platform owner's domain. | A |
| 3 | Where the controller chart and CRD live | A. the deployment repository's `charts/loom-agents` (D5). B. a chart in `beyond10x/loom` that the deployment repository only references by version. (The platform's own chart is out: it would be a platform dependency.) | A |
| 4 | New dependencies `kube` + `k8s-openapi` in the platform workspace | resolved 2026-10-25 (Timo: 0 dependencies on the platform): the controller is `beyond10x/loom` `crates/loom-operator` with its own gate; the platform workspace gains nothing. | B, taken |
| 5 | Model route for the first run | A. the in-cluster Bedrock signing proxy. B. Anthropic key Secret. C. OpenRouter key. | A, fall back to B |
| 6 | First slice | A. http reads + A2A answer, no credentials (D6). B. Slack read + write first, blocked on Connectors custody. | A |
| 7 | takt deliveries on this path | A. later slice C: a second `LoomAgent` variant with a takt image and command, after `loom-child-confinement` clears (§ 9a). B. out of scope: takt keeps its own deployment; the reason is that its confinement needs Substrate, which this design leaves out (§ 11). | A |

## 11. Not in this design

- Moving the platform's chart into the deployment repository (Timo: "we want to refactor that soon"): separate story.
- The platform's own first-party and per-account stored agents: untouched; a `LoomAgent` is a
  cluster-operator-owned resource, not a customer-owned agent.
- MCP read proxy (`loom-kubernetes.md` M6), Agency registration (M7), Substrate confinement (Q6).
- Multi-tenancy of `LoomAgent` resources across namespaces: one namespace per controller instance.
- takt deliveries: slice C, built as the `Command` variant (§ 9a, decision 7); nothing in slices A or B runs takt.
