---
format: aep.planning-md/3
id: story:boundary-admits-declared-http-egress
kind: story
status: proposed
title: A credential-free LoomAgent may reach an in-cluster http source it declares, through a namespace egress peer the operator allows
summary: Namespace entry admitted beside spec.boundary only outside every allowed upstream's namespace, with a selector, tied by namespace to an http binding with no bearer and listed in allowedBoundaryHttpEgress; the operator's list is the gate; chart and operator page change
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: charts/loom-agents/templates/configmap.yaml
- confidence: cited
  path: charts/loom-agents/values.yaml
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/config.rs
- confidence: cited
  path: crates/loom-operator/src/render.rs
- confidence: cited
  path: crates/loom-operator/tests/admission_reasons.rs
- confidence: cited
  path: crates/loom-operator/tests/adversary_d243_u1.rs
- confidence: cited
  path: crates/loom-operator/tests/chart.rs
- confidence: cited
  path: crates/loom-operator/tests/golden/credential-free-http-egress.yaml
- confidence: cited
  path: crates/loom-operator/tests/render.rs
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-allowed-http-egress-is-admitted.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-namespace-the-configuration-does-not-give-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-unbound-egress-namespace-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/config.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-namespace.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-not-allowed.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-unbound.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-http-egress-allowed.yaml
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:03Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
---
## Outcome

A `LoomAgent` with `spec.boundary` may reach an in-cluster `http` source it declares. Admission
step 5 (`fn boundary`, `crates/loom-operator/src/admission.rs:1361`) admits a
`deploy.kubernetes.egress` entry of `spec.agent` that names a namespace beside `spec.boundary`
only when every one of these holds, checked in this order, and otherwise refuses it at the entry:

1. its `namespace` is the namespace of no upstream the controller configuration's
   `allowedBoundaryUpstreams` lists, of any kind, whether or not this resource declares it
   (`BoundaryEgressNamespace` at its `namespace`, with today's detail);
2. its `selector` is not empty (`BoundaryEgressNamespace` at its `selector`: an empty selector
   opens every pod of the namespace);
3. an `http` binding of `spec.agent` with no `bearer_env` has a `base_url` whose host is a cluster
   service in that namespace (`<service>.<namespace>.svc` or `<service>.<namespace>.svc.cluster.local`,
   read with `admission::cluster_service`) (`BoundaryEgressUnbound` at the entry). The binding's
   method needs no check here: the document check refuses any method but `GET` first
   (`HttpMethod`, `ess/domains/agent.yaml:498`);
4. an entry of the new configuration list `allowedBoundaryHttpEgress`, `{namespace, selector,
   port}` (`config::AllowedHttpEgress`, unknown members refused; empty, the default, admits none),
   matches the entry (`BoundaryEgressNotAllowed` at the entry otherwise). The match rule: one
   allowed entry equals it in all three, its namespace, selector and port (values found in
   different allowed entries are no match), the selector compared as a whole set of label pairs (order ignored; a pair the entry lacks, or a pair the allowed entry lacks,
   is no match). So a selector that is a strict subset of the allowed one (which picks more pods)
   and one that is a strict superset (which picks fewer) are both refused: the operator allowed
   one selector, not a family of them.

An entry naming a host stays refused (`BoundaryEgressHost`). The NetworkPolicy of an admitted
resource opens DNS, each upstream as today, and each admitted entry as a peer of its namespace and
pod selector on its TCP port, as it renders an entry without a boundary
(`crates/loom-operator/src/render.rs:1179-1206`; today every entry is skipped beside a boundary,
`:1180-1184`). Nothing is added to the boundary container, the agent container or the relays. A
resource without `spec.boundary`, and one with a boundary and no namespace entry, renders byte for
byte as before. The two new reasons go after `BoundaryEgressNamespace` in `AdmissionRefusalReason`
(`ess/domains/resource.yaml:428-429`), outside the first fourteen, which are
`loom.agent.RefusalReason`'s (`:402-413`).

Limits, stated: admission ties an entry to a binding by namespace only. A NetworkPolicy names pods
by labels and a pod port; a `base_url` names a Service and its port, which admission cannot map to
pods or pod ports without reading the cluster. So condition 3 does not check that the selector
picks the binding's Service's pods or that the port is the one the Service targets; the operator's
list (condition 4) is the gate. `website/docs/reference/operator-cli.md`, where the controller's
configuration is documented (`charts/loom-agents/values.yaml` carries no comments, AGENTS.md
§ Operator), says that every `allowedBoundaryHttpEgress` entry must name read-path pods and a port
that serves only reads, and never a pod that forwards requests with a credential of its own.

## Why

Gap 2 of `.engineering/drafts/e6-request.md`: admission refuses every namespace egress entry beside
a boundary (`admission.rs:1473-1480`, rule text `:65-69` and `:1359-1360`), and the boundary relays
`model`, `connectors` and `git` only (`render.rs:245-263`), so an agent that reads Loki,
Prometheus and a health endpoint over `http` bindings loses them once it gets a boundary.

What the refusal stops: commit `85c92a0f` (2026-10-30, wave D243-L) added it to fix "the adversary
finding that a namespace entry opened an upstream's namespace on further ports"; its detail is "a
resource with spec.boundary reaches only DNS and its boundary's upstreams". The upstreams admit the
pod by its projected token (design D243 decision B1, as `epic:credential-free-agent-pod` and
`approval-record:wave-d243-l` quote it); a destination that admits by network position alone is
what the rule keeps closed. Design D243 itself is not in this repository or the workspace (searched
2026-11-21); its § 2 is read only as the stories quote it: "DNS plus the credential namespace on
the declared ports, no host entries" (`story:credential-free-network-policy`).

The two options asked, compared:

- (a), this story. Cost: admission, render, one configuration list, the chart and one page; no
  relay, no CRD member. What it lets through, all of which the rule exists to stop and the operator
  decides on per entry:
  - an unauthenticated write API on the allowed port: from any process in the pod, any method and
    path reaches the selected pods on that port (a NetworkPolicy is layer 4 and pod-wide), so a
    source whose writes share its read port (Loki's push, Prometheus's admin API when enabled) is
    writable; GET-only and the allowed paths are held in process by `HttpGet`
    (`loom.agent.AdmitHttpRead`) only;
  - a peer that forwards with its own credential: a selected pod that admits callers by network
    position and calls on with a credential of its own (a datasource proxy, a gateway) gives the
    agent that credential's reach, which is what B1 closed;
  - the sidecar's reach: the boundary container gets the same opening, since the policy selects
    the pod, not a container;
  - not per resource: `allowedBoundaryHttpEgress` is the controller's, with no field naming the
    resources or agents an entry is for, as `allowedBoundaryUpstreams` has none. One controller
    instance watches one namespace (AGENTS.md § Operator), so an entry allowed for one agent opens
    to every credential-free resource of that namespace that declares a matching binding.
  Conditions 1 and 4 keep the adversary fix and put each opening under the cluster operator, as
  `allowedBoundaryUpstreams` does for upstreams; condition 1 covers every upstream the operator
  allows, so an entry cannot open a Connectors or git host's namespace this resource does not
  declare.
- (b), a fourth relay. Cost: `spec.boundary.http` in `ess/domains/resource.yaml` and the CRD, flags
  and a listener in `crates/loom-boundary`, render, admission, the chart, and the agent rewriting an
  `http` binding's `base_url` to the relay. Lets through the same: the relay adds no token, so the
  upstream cannot tell the relay from the agent, and the NetworkPolicy that lets the boundary reach
  the source lets the agent container reach it on the same port. Its method and path filter is one
  the agent container can go around, and duplicates `HttpGet`'s.

The documents point to (a)'s shape: every D243 relay exists to add a credential the agent lacks
(A1, B1), and the network rule is written in namespaces and ports. The residual exposure above is
the operator's decision (`e6r2-changes.md` § Open).

## Waits on

Nothing in this epic. It is the first story on `crates/loom-operator/src/admission.rs`,
`crates/loom-operator/tests/render.rs`, `ess/domains/resource.yaml` (`AdmissionRefusalReason`,
`AdmitLoomAgent`), `ess/scenarios/resource-admission/` and `ess/ess-inputs.yaml`;
`story:send-input-in-the-declared-shape` (`ess/ess-inputs.yaml`, the generated plan files) and
`story:agent-selects-the-connectors-relay` (`admission.rs`, `tests/render.rs`,
`tests/adversary_d243_u1.rs`, `ess/ess-inputs.yaml`) depend on it.

The second to land rebases, with no edge, against four draft stories of other epics that edit the
same admission files: `story:credential-free-workspace-isolation` (`admission.rs`,
`tests/render.rs`), `story:egress-by-host` (`render.rs`, `ess/domains/resource.yaml`, the chart),
`story:secret-files-several-secrets` and `story:command-home-survives-pod-replacement`
(`admission.rs`, `render.rs`, `ess/domains/resource.yaml`).

## ESS first

The first commit changes only `ess/`: `ess/domains/resource.yaml` — `AdmissionRefusalReason` gains
`BoundaryEgressUnbound` and `BoundaryEgressNotAllowed` after `BoundaryEgressNamespace`,
`AdmitLoomAgent` an `external:` outcome for each, and the comment above the enum (`:412-413`) the
new rule; `ess/scenarios/resource-admission/config.yaml` gains `allowedBoundaryHttpEgress`; the
resource file `resources/boundary-egress-namespace.yaml` of the existing scenario
`a-boundary-beside-an-egress-namespace-is-refused` moves its entry to namespace `llm`, the
namespace of the configuration's allowed model upstream (`config.yaml:14`), so the scenario keeps
its reason `BoundaryEgressNamespace`, member and detail under condition 1 (with `monitoring`, as
today, it would reach condition 3 and answer `BoundaryEgressUnbound`); new authored scenarios
`a-boundary-beside-an-unbound-egress-namespace-is-refused.yaml`,
`a-boundary-beside-an-egress-namespace-the-configuration-does-not-give-is-refused.yaml` and
`a-boundary-beside-an-allowed-http-egress-is-admitted.yaml`, with the resource files
`resources/boundary-egress-unbound.yaml`, `resources/boundary-egress-not-allowed.yaml` and
`resources/boundary-http-egress-allowed.yaml`, each scenario listed in `ess/ess-inputs.yaml`.
`generated/rust/loom/` changes with the enum (`task drift` red on that commit), and `task conform`
is red on the new scenarios.

## Acceptance

`crates/loom-operator/tests/render.rs`, `admission_reasons.rs`, `adversary_d243_u1.rs` and
`chart.rs`, no cluster:

- `a_declared_http_egress_beside_a_boundary_is_admitted_and_rendered`: the NetworkPolicy holds DNS,
  the upstream rules of today and exactly one more rule, the entry's namespace and pod selector on
  its port.
- `a_credential_free_agent_with_an_http_egress_renders_as_its_golden_file`: the example agent
  (`credential_free_agent()`) with the entry `{name: logs, namespace: monitoring, selector: [{name:
  app, value: logs}], port: 3100}`, its `logs` binding (`http://logs.monitoring.svc:3100`, no
  `bearer_env`), a model-only boundary (`boundary(true, false, false)`, so the example keeps its
  `runtime.connectors` and `story:agent-selects-the-connectors-relay`'s helper change does not
  re-render it) and a configuration allowing that entry, renders byte for byte as
  `tests/golden/credential-free-http-egress.yaml`, as
  `a_credential_free_command_resource_renders_as_its_golden_file` reads `credential-free.yaml`
  (`LOOM_OPERATOR_BLESS=1` rewrites it).
- `an_egress_in_an_allowed_upstream_namespace_is_refused`: an entry naming the namespace of an
  `allowedBoundaryUpstreams` entry the resource does not declare (a `connectors` upstream beside a
  model-only boundary), with a selector, a binding and an allow-list entry, is
  `BoundaryEgressNamespace`; so is one naming a declared upstream's namespace.
- `an_egress_with_an_empty_selector_beside_a_boundary_is_refused`: `BoundaryEgressNamespace` at
  `selector`.
- `an_egress_no_credential_free_binding_names_is_refused`: no `http` binding in that namespace, and
  a GET binding with `bearer_env`, each `BoundaryEgressUnbound`.
- `a_post_binding_beside_an_http_egress_is_refused_by_the_document_check`: `HttpMethod` at the
  binding, before step 5.
- The operator's gate, condition 4, against the match rule. Every case below has the entry
  `{name: logs, namespace: monitoring, port: 3100}` with the selector named, the example's `logs`
  binding (`http://logs.monitoring.svc:3100`, no `bearer_env`) and a model-only boundary, so
  conditions 1 to 3 hold and only the allow list decides. Each is its own test:
  - `an_egress_whose_namespace_is_not_the_allowed_one_is_refused`: entry selector `[app=logs]`,
    allowed `{namespace: observability, selector: [app=logs], port: 3100}`:
    `BoundaryEgressNotAllowed` at the entry. A gate comparing only selector and port admits it.
  - `an_egress_whose_selector_is_a_strict_subset_of_the_allowed_one_is_refused`: entry selector
    `[app=logs]`, allowed `{monitoring, [app=logs, tier=read], 3100}`: `BoundaryEgressNotAllowed`.
  - `an_egress_whose_selector_is_a_strict_superset_of_the_allowed_one_is_refused`: entry selector
    `[app=logs, tier=read]`, allowed `{monitoring, [app=logs], 3100}`: `BoundaryEgressNotAllowed`.
  - `an_egress_whose_selector_has_another_value_is_refused`: entry `[app=logs]`, allowed
    `{monitoring, [app=metrics], 3100}`: `BoundaryEgressNotAllowed`.
  - `an_egress_on_another_port_than_the_allowed_one_is_refused`: entry port 3100, allowed
    `{monitoring, [app=logs], 9090}`: `BoundaryEgressNotAllowed`.
  - `an_egress_beside_an_empty_allow_list_is_refused`: `allowedBoundaryHttpEgress: []`:
    `BoundaryEgressNotAllowed`.
  - `an_egress_whose_values_are_spread_over_two_allowed_entries_is_refused`: entry selector
    `[app=logs]`, allowed `{monitoring, [app=metrics], 9090}` and
    `{observability, [app=logs], 3100}`: its namespace is the first entry's, its selector and port
    the second's, and no single entry equals it in all three: `BoundaryEgressNotAllowed`. A gate
    matching each value against any allowed entry admits it.
  - `an_egress_matching_an_allowed_entry_exactly_is_admitted`: entry `[app=logs, tier=read]`,
    allowed `{monitoring, [tier=read, app=logs], 3100}` (the same set in another order) beside a
    second allowed entry `{observability, [app=logs], 3100}`: rendered, and the NetworkPolicy's
    one added rule is namespace `monitoring`, selector `app=logs, tier=read`, port 3100.
- `a_host_egress_beside_a_boundary_is_still_refused`.
- `an_http_egress_adds_no_credential_variable_or_volume`: for a resource admitted with the entry,
  compared with the same resource without it, the pod's `volumes` and, in the agent and boundary
  containers, `env`, `envFrom` and `volumeMounts` are equal; no container gains a Secret reference
  or a projected token. The test fails if either container gains any of them.
- `render.rs:2586`, `a_namespace_egress_entry_beside_a_boundary_is_refused`, changes and asserts
  afterwards: its first resource (`logs`/`monitoring` with a selector; the example's `logs`
  binding names that namespace with no bearer; `boundary_config()` allows no http egress) is
  `refused: BoundaryEgressNotAllowed at spec.agent.deploy.kubernetes.egress[0]: …`; in its loop
  the `connectors` and `llm` entries stay `BoundaryEgressNamespace` at `egress[0].namespace`
  (condition 1), the `monitoring` entry without a selector before a host entry becomes
  `BoundaryEgressNamespace` at `egress[0].selector` (condition 2), and the host entry first stays
  `BoundaryEgressHost` at `egress[0].host`; no refusal repeats `monitoring`, `connectors`, `llm` or
  `chat.example`, and the entries are still admitted without a boundary.
- `adversary_d243_u1.rs:282`,
  `an_egress_namespace_entry_does_not_open_an_upstream_namespace_on_another_port`, today returns on
  any `RenderError::Refused` (`:292`) and so tests nothing once its entry is refused. Afterwards its
  entry carries a selector (`selector: [{name: app, value: store}]`) and the configuration allows
  it in `allowedBoundaryHttpEgress`, and the test asserts the refusal is `BoundaryEgressNamespace`
  at that entry's `namespace`, so condition 1 alone refuses it.
- `chart.rs`: the two `Config` literals without `..` (`:290`, `:330`) gain
  `allowed_boundary_http_egress`; the chart renders `allowedBoundaryHttpEgress` from values into
  `config.yaml`, `[]` by default; `the_values_reference_says_what_an_http_egress_entry_may_name`
  finds in `website/docs/reference/operator-cli.md` the sentence that an entry names read-path pods
  and a port that serves only reads, never a pod that forwards with its own credential.
- The golden files `platform-operator.yaml`, `takt-delivery.yaml` and `credential-free.yaml` are
  unchanged.
- `task conform`, `task crd-drift` (the CRD is expected unchanged), `task chart-check` and
  `task check` exit 0.

CHANGELOG: `### Changed`, a namespace egress entry beside `spec.boundary` is admitted under
`allowedBoundaryHttpEgress`. No `**Breaking:**` bullet: every resource admitted today is admitted
and renders the same, and the default list admits nothing new. The chart changes (`values.yaml`,
`templates/configmap.yaml`); the CRD does not.

## Files

- `ess/domains/resource.yaml`, `ess/scenarios/resource-admission/` (`config.yaml`,
  `resources/boundary-egress-namespace.yaml`, three scenarios and their three resource files),
  `ess/ess-inputs.yaml`, `generated/rust/loom/`
- `crates/loom-operator/src/{admission.rs,render.rs,config.rs}`, `tests/render.rs`,
  `tests/admission_reasons.rs`, `tests/adversary_d243_u1.rs`, `tests/chart.rs`,
  `tests/golden/credential-free-http-egress.yaml`
- `crates/loom-conformance/src/admission.rs` (only if its configuration reader needs the member)
- `charts/loom-agents/values.yaml`, `charts/loom-agents/templates/configmap.yaml`, `Taskfile.yml`
  (`chart-check` overrides)
- `website/docs/reference/operator-cli.md`, AGENTS.md § Operator and § Resource, CHANGELOG.md

## Out of scope

An `http` relay in the boundary (option (b), above). A credential on an `http` binding beside a
boundary. An egress entry naming a host. Layer-7 policy (method or path in the NetworkPolicy).
Mapping a Service to its pods or ports at admission. A per-resource field on an
`allowedBoundaryHttpEgress` entry.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-operator` (admission step 5, NetworkPolicy render, controller config) — cited
- **Files (code):** `crates/loom-operator/src/admission.rs:1361` (`fn boundary`; egress loop `:1462-1481`), `src/render.rs:1178-1206` (egress entries skipped beside a boundary at `:1180-1184`), `src/config.rs:51-91,118-133` (`Config`, `AllowedUpstream`; new `AllowedHttpEgress`), `tests/render.rs`, `tests/admission_reasons.rs`, `tests/adversary_d243_u1.rs`, `tests/chart.rs`, new `tests/golden/credential-free-http-egress.yaml` — cited
- **Files (ESS):** `ess/domains/resource.yaml:403-430,1038-1147` (`AdmissionRefusalReason`, `AdmitLoomAgent`), `ess/scenarios/resource-admission/{config.yaml,resources/boundary-egress-namespace.yaml}`, three new scenarios, `ess/ess-inputs.yaml:94-98` — cited
- **Files (chart):** `charts/loom-agents/values.yaml:17`, `charts/loom-agents/templates/configmap.yaml:36`, `Taskfile.yml:96` (`chart-check` overrides) — cited
- **Files (resource files of the three new scenarios):** `ess/scenarios/resource-admission/resources/{boundary-egress-unbound,boundary-egress-not-allowed,boundary-http-egress-allowed}.yaml` — cited (named by the story)
- **Symbols:** `Reason::BoundaryEgressNamespace`/`BoundaryEgressHost` (`admission.rs:1467,1475`), `admission::cluster_service` (`:1203`; `ClusterService.namespace` at `:1167`), `Config.allowed_boundary_upstreams`, `AllowedUpstream` (`config.rs:118`), `RELAYS` (`render.rs:245`), `HttpMethod` (`ess/domains/agent.yaml:498`), `Binding.bearer_env` (`generated/rust/loom/src/agent.rs:245`) — cited
- **Symbols (new):** `BoundaryEgressUnbound`, `BoundaryEgressNotAllowed`, `config::AllowedHttpEgress`, `allowedBoundaryHttpEgress` — cited (the story names them; none exists yet)
- **Also likely:** `crates/loom-conformance/src/admission.rs:116` reads the configuration through `Config::parse`, so it needs no edit once `config.rs` has the member — inferred
- **Documents:** `website/docs/reference/operator-cli.md` (`--config` row at `:36`, new read-path sentence), `AGENTS.md` § Operator (`:1225-1228` holds the egress rule) and § Resource (boundary paragraph at about `:1027`), `CHANGELOG.md` — cited
- **Confidence:** high — the story names files, lines and symbols, and I checked each cite against the tree (the defects below are the exceptions).
- **Would collide with** (`depends_on` read from `aep plan artifact graph --format json`):
  - `story:send-input-in-the-declared-shape` on `ess/ess-inputs.yaml`. It `depends_on` this story, so the edge orders them. — cited
  - `story:agent-selects-the-connectors-relay` on `admission.rs`, `tests/render.rs`, `ess/ess-inputs.yaml`. It `depends_on` this story, so the edge orders them. — cited
  - `story:credential-free-workspace-isolation` (draft) on `admission.rs`, `tests/render.rs`, AGENTS.md § Operator. **No edge.** The story's "Out of scope" says so ("the second to land rebases"). — cited
  - `story:egress-by-host` (draft) on `render.rs` (NetworkPolicy), `ess/domains/resource.yaml` and `charts/loom-agents`. **No edge** (it decomposes `epic:loom-agent-command-variant`), and this story does not mention it. — cited from its `path:` scope
  - `story:secret-files-several-secrets` (draft) on `admission.rs`, `render.rs`, `ess/domains/resource.yaml`. **No edge**, not mentioned. — cited
  - `story:command-home-survives-pod-replacement` (draft) on `render.rs`, `admission.rs`, `ess/domains/resource.yaml`; its scope is inferred. **No edge**, not mentioned. — cited
  - `ess/ess-inputs.yaml` is also a list append for `story:engine-configuration-document`, `story:autonomy-grant-table` and `story:decision-request-entity` (all draft, no edge). A trivial merge. — inferred
  - `story:connectors-relay-client` shares no file with this one except `CHANGELOG.md`. — cited
- **Safety fact:** admitted resources render unchanged because the new code runs only for an entry with a namespace beside a boundary. Today that is refused at `admission.rs:1473-1480`, so nothing rendered today reaches it. Render's `entries` is `&[]` whenever `upstreams` is non-empty (`render.rs:1180-1184`), and the story changes only that branch. Level 2, unproven.
