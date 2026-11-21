---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-design-r1
kind: review-result
status: active
title: Plan critic design, round 1, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

epic:boundary-agent-reaches-connectors-and-http-sources — the epic's first acceptance line needs story 2's agent test to show story 3's `{body: {channel, text}}` shape, but neither story's acceptance asserts it and no `depends_on` edge joins them, so the consumer's working Slack post is shown by no story and the two are planned for one wave; story 3 should own the assertion in `crates/loom-agent/tests/connectors_relay.rs` and gain `depends_on story:agent-selects-the-connectors-relay` — .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md:81, .engineering/planning/story/agent-selects-the-connectors-relay.md:87, .engineering/planning/story/send-input-in-the-declared-shape.md:74

story:connectors-relay-client — it adds the public `loom_connectors::Connectors` enum (re-exported in the SDK and listed in `sdk-stability.md`), but none of its acceptance lines uses the enum and its only users are story 2's `Host`, `DataSourceEffects` and Slack poll, so the enum should move to `story:agent-selects-the-connectors-relay`, or story 1 should gain a test that calls both transports through it — .engineering/planning/story/connectors-relay-client.md:18, .engineering/planning/story/agent-selects-the-connectors-relay.md:32,58

**What I read:** 5 artifacts (`aep plan artifact show` on each, `relations`, `graph --format json`, `validate`). I walked all 13 edges, including outside the set (the 6 `serves` edges to `vision:O1` and `vision:governed-autonomy`; nothing outside points into the set). There is no cycle: the `depends_on` edges are 2→1, 3→1 and 4→2. I traced 5 acceptance reads to a producer in the set:
- story 2 reads story 1's `ConnectorsRelay`.
- story 3 reads story 1's `connectors_relay.rs` and fake service.
- epic line 1 reads stories 2 and 3.
- epic line 3 reads story 4.

Edges record 2 of the 3 story-level reads. The 2/3 integration is the missing one. Source opened at `39f4da88`:
- `admission.rs`: lines 60-72, 1351-1365, 1465-1485 and 1598-1618.
- `route.rs:106-118` in `crates/loom-boundary/src`.
- `poll.rs:27,143` and `plugin/lib.rs:193-199`.
- the `describe-write.json` fixture and how `serve.rs` uses it.
- `AGENTS.md:1460-1470`.
- `connectors-core` `Descriptor` and `Operation` at `ea7ec60` (`v0.62.0`).
- connectors `compatibility.md:49`.

**What I could not establish:**
- **Edge 4→2 is a trade-off, not a finding.** The reason is written beside it: both edit `fn boundary` (step 5) in `crates/loom-operator/src/admission.rs`, `AdmissionRefusalReason` in `ess/domains/resource.yaml`, and `ess/scenarios/resource-admission/`. Option 1 is to keep the order; then gap 2 (story 4) waits behind stories 1 and 2. Option 2 is to move story 2's admission refusals (`BoundaryOperationUndeclared`, and `Malformed` for both transports together) into their own unit, or into story 4, so story 4 can start in wave 1. The drafter chooses.
- **Story 2 vs story 4 on `ess/scenarios/resource-admission/config.yaml`.** The epic says both edit it (epic `:73`), but story 2's ESS-first list does not include it. This belongs to parallel safety, not me.
- **One relay, many instances (scope or acceptance lane, not mine).** `GET /v1/describe` returns one `Descriptor` with one `instance` (`connectors-core` `lib.rs:103-110`), and one boundary has one Connectors upstream. Neither story 1 nor story 2 says what happens to an agent whose `connectors` sources name several instances behind the relay; the `Connectors` enum claims the same methods and signatures as the multi-instance `ConnectorsCli`.
- **Story 3 says "both transports", but there are three write paths (scope lane).** `ConnectorsInvoker`, Commission's wire path at `crates/loom-connectors/src/lib.rs`, is not shaped, while the epic outcome speaks of every governed write.
- **Story 3's acceptance does not state the no-credential invariant (acceptance lane).** Story 3 touches no pod or credential surface, so the invariant holds trivially. Stories 1, 2 and 4 each have a test that fails if it breaks: `the_relay_client_sends_no_credential`, `the_upstream_sees_only_the_boundarys_token`, `the_boundary_container_and_the_agent_container_are_unchanged_by_an_http_egress`.
- **Story 4's POST-binding case may be refused earlier (acceptance lane).** `check` already refuses a non-GET `http` binding (`HttpMethod`, `ess/domains/agent.yaml:498`), so the case may never reach step 5 as `BoundaryEgressUnbound`.
- **Story 4, option (a), for the operator's decision.**
  - **What the rule exists to stop.** Story 4 states it accurately: commit `85c92a0f` (upstream namespace on further ports) and "reaches only DNS and its boundary's upstreams" (`admission.rs:1476-1477`). Condition 1 keeps the first. Option (a) knowingly ends the second.
  - **For (a).** The argument against (b) holds: the policy selects pods, not containers (`AGENTS.md:1463-1467`). A relay that adds no credential therefore isolates nothing the agent container could not bypass.
  - **What (a) lets through, named:** Loki push.
  - **What (a) lets through, not named:**
    - A selected peer that admits by network position and forwards with its own credential (a datasource proxy, a gateway) gives the agent credentialed reach. That is exactly what D243 B1 closed.
    - Condition 3 ties only the namespace to a binding, not the selector or the port to the binding's Service. So conditions 2 and 3 add little; condition 4's allow-list is the only real gate.
  - **My advice.** Accept (a), and have the chart docs require that every allow-list entry names read-path pods and a read-only port, never a proxy that adds a credential.

```findings
- file: .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md
  line: 81
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the epic's first acceptance line needs story 2's agent test to show story 3's `{body: {channel, text}}` shape, but neither story's acceptance asserts it and no `depends_on` edge joins them, so the consumer's working Slack post is shown by no story and the two are planned for one wave; story 3 should own the assertion in `crates/loom-agent/tests/connectors_relay.rs` and gain `depends_on story:agent-selects-the-connectors-relay`"
- file: .engineering/planning/story/connectors-relay-client.md
  line: 18
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds the public `loom_connectors::Connectors` enum (re-exported in the SDK and listed in `sdk-stability.md`), but none of its acceptance lines uses the enum and its only users are story 2's `Host`, `DataSourceEffects` and Slack poll, so the enum should move to `story:agent-selects-the-connectors-relay`, or story 1 should gain a test that calls both transports through it"
```
