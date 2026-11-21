---
format: aep.planning-md/3
id: story:boundary-admits-declared-http-egress
kind: story
status: implemented
title: A credential-free LoomAgent may reach an in-cluster http source it declares, through a namespace egress peer the operator allows
summary: Namespace entry admitted beside spec.boundary only outside every allowed upstream's namespace, with a selector, tied by namespace to an http binding with no bearer and listed in allowedBoundaryHttpEgress; the operator's list is the gate; chart and operator page change
tags:
- ai-agent-platform
relations:
- decomposes: epic:boundary-agent-reaches-connectors-and-http-sources
- serves: vision:O1
- depends_on: story:ess-096-upgrade
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
  path: crates/loom-conformance/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/admission.rs
- confidence: cited
  path: crates/loom-operator/src/cli.rs
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
  path: crates/loom-operator/tests/security_w20261121w6_u2.rs
- confidence: cited
  path: ess/domains/resource.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-allowed-http-egress-is-admitted.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-in-the-model-route-namespace-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-namespace-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-namespace-the-configuration-does-not-give-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-namespace-without-a-selector-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-egress-that-names-no-namespace-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/a-boundary-beside-an-unbound-egress-namespace-is-refused.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/config.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-namespace-name.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-namespace.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-not-allowed.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-unbound.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-egress-unselected.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/boundary-http-egress-allowed.yaml
- confidence: cited
  path: ess/scenarios/resource-admission/resources/with-model-route
revision: 26
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T02:54:03Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-accepts-boundary-epic-2026-11-21"}
- {from: "proposed", to: "active", at: "2026-11-21T10:53:57Z", actor: "agent:sdc", revision: 13, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
- {from: "active", to: "implemented", at: "2026-11-21T12:56:32Z", actor: "agent:sdc", revision: 26, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}, executor: "agent:loom", correlation: "loom-wave-2026-11-21-w6"}
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

## As built, wave 2026-11-21-w6

Where the unit's commits (`9b4170fb..527a3147`) differ from the text above, the commits are what
holds:

- Condition 1 closes more than the text says. Security pass 1
  (`review-result:boundary-admits-declared-http-egress-security-1`) found that the namespace of
  the configuration's `modelRoute`, a proxy holding a credential, stayed open (F1), and that an
  upstream URL with a short service name or an IP host read as no namespace, so its namespace
  stayed open (F2). The planner decided on 2026-11-21: condition 1 also refuses the namespace of
  `modelRoute`, and the configuration read refuses an `allowedBoundaryUpstreams` or `modelRoute`
  URL whose host is no `<service>.<namespace>.svc[.cluster.local]`.
- Security pass 2 (`review-result:boundary-admits-declared-http-egress-security-2`) found that
  admission, given such a URL on a `Config` built without `Config::parse`, still admitted the
  entry (F2-b). Condition 1 now fails closed (`crates/loom-operator/src/admission.rs`,
  `fn boundary_http_egress`): one URL whose namespace cannot be read refuses every namespace
  entry beside a boundary with `BoundaryEgressNamespace` at the entry's `namespace`, the detail
  naming the setting and the URL. The detail quotes the controller's URL; the shipped binary
  does not reach that branch, because the configuration read refuses such a URL first.
- After the upstream and model-route check, condition 1 refuses a namespace that is no DNS-1123
  label, with the same reason and member and its own detail.
- Condition 3 reads only a binding whose method is `GET`.
- The CHANGELOG has two `**Breaking:**` bullets where the text says none:
  `AdmissionRefusalReason` gains `BoundaryEgressUnbound` and `BoundaryEgressNotAllowed` and
  operator `Config` gains `allowed_boundary_http_egress`; and the configuration read refuses a
  configuration it accepted before (a short service name or an IP address as the host of an
  `allowedBoundaryUpstreams` or `modelRoute` URL).
- Six scenarios under `ess/scenarios/resource-admission/` where the text names three: the three
  more state the model route's namespace, an entry without a selector and a namespace that is no
  DNS-1123 label (findings F1 and F4 of pass 1).
- Files the list below does not name: `crates/loom-conformance/src/admission.rs` (the
  conformance target reads a `config.yaml` beside a resource when its directory holds one; one
  directory has one, `resources/with-model-route/`), `crates/loom-operator/src/cli.rs`
  (`RUN_HELP` and the `--config` text), `crates/loom-operator/tests/security_w20261121w6_u2.rs`
  (the reviewer's cases, committed as written in `ffcd7c0a` and `3a6ef6a3`).
- A test of the base that asserted nothing is fixed by the unit
  (`crates/loom-operator/tests/adversary_d243_u1.rs`, finding F3 of pass 1: its resource was
  refused `NoModelRoute` before the egress check).
- No third security pass ran: the coordinator read the diff `0c9d56e3..527a3147` after
  correction 2 (evidence of kind `verification` on this story).

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

Rewritten 2026-11-21 at the close of wave 2026-11-21-w6 from the unit's commits (`git diff --stat 9b4170fb 527a3147`: 61 files, 2735 insertions, 122 deletions). Every line is cited from that diff.

- **Primary surface:** `crates/loom-operator` — cited, 10 of the 61 changed files are under it (four in `src/`, six in `tests/`, one of them the new golden file); `ess/` holds 18 more (296 insertions, 7 deletions) and `generated/` 23
- **Files (ESS sources):** `ess/domains/resource.yaml` (22 insertions, 4 deletions: `AdmissionRefusalReason` gains `BoundaryEgressUnbound` and `BoundaryEgressNotAllowed` after `BoundaryEgressNamespace`, `AdmitLoomAgent` an `external:` outcome for each, the `boundary-egress-namespace` outcome text widens, the comment above the enum gives the six checks in order), `ess/ess-inputs.yaml` (6 lines: six new scenarios listed), `ess/scenarios/resource-admission/config.yaml` (2 lines: `allowedBoundaryHttpEgress`), six new scenarios (`a-boundary-beside-an-allowed-http-egress-is-admitted`, `...-an-egress-namespace-the-configuration-does-not-give-is-refused`, `...-an-unbound-egress-namespace-is-refused`, `...-an-egress-in-the-model-route-namespace-is-refused`, `...-an-egress-namespace-without-a-selector-is-refused`, `...-an-egress-that-names-no-namespace-is-refused`), seven new resource files under `resources/` (`boundary-egress-not-allowed`, `boundary-egress-unbound`, `boundary-http-egress-allowed`, `boundary-egress-namespace-name`, `boundary-egress-unselected`, and in the new directory `resources/with-model-route/` `boundary-egress-model-route` and `config`) — cited; the existing scenario `a-boundary-beside-an-egress-namespace-is-refused.yaml` (comment only, 2 insertions, 1 deletion) and its resource `resources/boundary-egress-namespace.yaml` (the entry moves from namespace `monitoring` to `llm`, 3 insertions, 2 deletions)
- **Files (generated output):** `generated/rust/loom/` — cited, 23 files (42 insertions, 28 deletions) rewritten by `ess synthesize --layout crate` (`task generate`): the contract digest line in each of 20 `src/*.rs` and in `Cargo.toml`, `PLAN.md` and `plan.json`, plus the two new reasons and two new outcomes in `src/resource.rs` (17 insertions, 3 deletions)
- **Files (source):**
  - `crates/loom-operator/src/admission.rs` (149 insertions, 12 deletions): new `fn boundary_http_egress` holds conditions 2 to 4, condition 1 now also closes the `modelRoute` URL's namespace and refuses an unreadable one, new `fn upstream_namespace`, `reason_spelling` gains two arms
  - `crates/loom-operator/src/config.rs` (302 insertions, 2 deletions): `Config` gains `allowed_boundary_http_egress`, new `AllowedHttpEgress` and `SelectorLabel`, `Config::allowed_http_egress`, `Config::parse` reads each entry and refuses an upstream or `modelRoute` URL whose host names no namespace (`names_namespace`), `Config::builtin` sets the empty list
  - `crates/loom-operator/src/render.rs` (24 insertions, 7 deletions): beside a boundary the NetworkPolicy keeps an egress entry that has no host, a namespace, a selector and an equal allowed entry, as a peer after DNS
  - `crates/loom-operator/src/cli.rs` (13 insertions, 4 deletions): `RUN_HELP` and the `--config` doc name `allowedBoundaryHttpEgress` and the closed namespaces
  - `crates/loom-conformance/src/admission.rs` (11 insertions, 2 deletions): the target reads the `config.yaml` beside a resource when its directory holds one
- **Files (chart and tasks):** `charts/loom-agents/values.yaml` (1 line, `allowedBoundaryHttpEgress: []`), `charts/loom-agents/templates/configmap.yaml` (1 line), `Taskfile.yml` (6 lines, the `chart-check` overrides) — cited
- **Files (new tests):**
  - `crates/loom-operator/tests/security_w20261121w6_u2.rs` (585 lines, 12 `#[test]` cases: the two security passes' cases)
  - `crates/loom-operator/tests/golden/credential-free-http-egress.yaml` (458 lines, the golden render of the example agent with one entry)
  - the 18 new cases in `tests/render.rs` and the 4 in `src/config.rs` are counted below
- **Files (existing tests):**
  - `crates/loom-operator/tests/render.rs` (634 insertions, 14 deletions): 65 `#[test]` before, 83 after, so 18 new cases, among them `a_declared_http_egress_beside_a_boundary_is_admitted_and_rendered`, `a_credential_free_agent_with_an_http_egress_renders_as_its_golden_file`, eight `allowedBoundaryHttpEgress` match cases, `an_egress_in_the_model_route_namespace_is_refused`, `an_http_egress_adds_no_credential_variable_or_volume`; `a_namespace_egress_entry_beside_a_boundary_is_refused` now expects `BoundaryEgressNotAllowed` at the entry and `egress[0].selector` for the entry without a selector; the `allowing` helper sets the upstreams on a parsed `Config` instead of reading them
  - `crates/loom-operator/tests/chart.rs` (54 insertions, 2 deletions): 9 `#[test]` before, 10 after; `OVERRIDES` grows from 18 to 24, the `Config` literals gain `allowed_boundary_http_egress`, one new case `the_values_reference_says_what_an_http_egress_entry_may_name`
  - `crates/loom-operator/tests/admission_reasons.rs` (46 insertions, 9 deletions): `REFUSED` grows from 35 to 40 rows, `boundary-http-egress-allowed` joins the admitted fixtures, `answer` reads a `config.yaml` beside the fixture (4 `#[test]` before and after)
  - `crates/loom-operator/tests/adversary_d243_u1.rs` (42 insertions, 7 deletions): `an_egress_namespace_entry_does_not_open_an_upstream_namespace_on_another_port` now carries a selector, a binding and an allow-list entry and asserts `BoundaryEgressNamespace` at the namespace instead of returning on any refusal; new helper `config_with` (5 `#[test]` before and after)
  - `crates/loom-operator/src/config.rs` unit tests: 5 `#[test]` before, 9 after, 4 new
- **Files (documents):** `AGENTS.md` (33 insertions, 14 deletions: § Operator and § Resource), `README.md` (13 insertions, 4 deletions), `CHANGELOG.md` (3 insertions: one `### Changed` bullet and two `**Breaking:**` bullets), `website/docs/reference/operator-cli.md` (12 insertions, 4 deletions), `website/docs/reference/ess/loom-resource.md` (9 insertions, 5 deletions, generated), `website/data/status.json` (1 line) — cited
- **Symbols:**
  - added (public, `b10x_loom_operator::config`): `AllowedHttpEgress` (`namespace`, `selector`, `port`), `SelectorLabel` (`name`, `value`), field `Config::allowed_boundary_http_egress`, method `Config::allowed_http_egress`
  - added (generated, `b10x_loom_resource::model`): `AdmissionRefusalReason::BoundaryEgressUnbound` and `BoundaryEgressNotAllowed`, `AdmitLoomAgentOutcome::BoundaryEgressUnbound` and `BoundaryEgressNotAllowed`
  - changed behaviour: `Config::parse` refuses an `allowedBoundaryUpstreams` or `modelRoute` URL whose host is no `<service>.<namespace>.svc[.cluster.local]`; `Config::builtin` gains the empty list; `admission::reason_spelling` gains two arms; the `BoundaryEgressNamespace` outcome text widens
  - removed: none
- **Not predicted and touched:** `crates/loom-operator/src/cli.rs`, `crates/loom-conformance/src/admission.rs` (the stored Scope says "inferred" it needs no edit, and the story's Files says "only if"), `crates/loom-operator/tests/security_w20261121w6_u2.rs`, four scenarios and three resource files (`a-boundary-beside-an-egress-in-the-model-route-namespace-is-refused`, `a-boundary-beside-an-egress-namespace-without-a-selector-is-refused`, `a-boundary-beside-an-egress-that-names-no-namespace-is-refused`, `a-boundary-beside-an-egress-namespace-is-refused` (comment), `resources/boundary-egress-namespace-name`, `resources/boundary-egress-unselected`, and the two files of `resources/with-model-route/`); documents `README.md` and `website/data/status.json` (the stored Scope names AGENTS.md, CHANGELOG.md and `operator-cli.md`), and the generated `website/docs/reference/ess/loom-resource.md` — cited
- **Predicted and not touched:** none — cited; all 22 paths of the typed scope are in the diff
- **Review:** `ls .engineering/planning/review-result | grep boundary-admits-declared-http-egress` shows two results, `-security-1.md` and `-security-2.md`, both red. `review-result:boundary-admits-declared-http-egress-security-1` (head `984ce66e`): F1 condition 1 ignored the `modelRoute` namespace (warning, confirmed), F2 an upstream URL with a short name read as no namespace (warning, infeasible), F3 the d243 adversary case tested nothing at the base (note, pre-existing), F4 no scenario for the selector refusal or an invalid namespace name (note), F5 AGENTS.md put the checks in the code's reverse order (note); 12 cases written, 2 red. `review-result:boundary-admits-declared-http-egress-security-2` (head `0c9d56e3`): F1, F4 and F5 closed, F2 closed at the configuration read; one finding left, F2-b, admission treats a URL with no readable namespace as no upstream there (warning, infeasible in the binary, introduced); fixed by `527a3147` ("condition 1 fails closed on an unreadable upstream namespace"), which neither review re-ran
