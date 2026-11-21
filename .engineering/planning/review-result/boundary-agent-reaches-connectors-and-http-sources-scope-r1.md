---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-scope-r1
kind: review-result
status: active
title: Plan critic scope, round 1, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

story:send-input-in-the-declared-shape — The body promises the declared shape for "every catalog write that has a request body", but its rule only handles writes whose input is the body alone. A catalog write that also has path or query parameters at the top level fits neither `flat` nor `body`, so it is refused `input-shape-unknown`, and neither the story's nor the epic's Out of scope records that this case was dropped. — .engineering/planning/story/send-input-in-the-declared-shape.md:27, :46-47, :97; request "sends the input in the shape the catalog operation declares" .engineering/drafts/e6-request.md:35-36; connectors `v0.73.0` `adapters/catalog/src/lib.rs:2872-2896` (parameters become top-level properties, then `body` is added beside them)
story:agent-selects-the-connectors-relay — The admission refusal `BoundaryOperationUndeclared` (a binding operation that `spec.boundary.connectors.operations` does not list) is not asked for by any sentence of the parent, and the epic's Outcome does not claim it. It is also the only reason this story edits `AdmissionRefusalReason` and `AdmitLoomAgent`, which is half of story 4's `depends_on` edge. Either drop it (the relay's 403, already in the epic's acceptance, covers the runtime case) or name it in the epic Outcome as an added refusal and give the reason. — .engineering/planning/story/agent-selects-the-connectors-relay.md:42-45, :73; .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md:15-28, :70-73; .engineering/drafts/e6-request.md:33-36

**Promises:** 11 extracted from the parent, 8 traced to an item, 3 named by the drafter as replies or process outside the set. One of the 8 is the narrowing above.
1. reply.send speaks the wire to `LOOM_BOUNDARY_CONNECTORS_URL` itself (request:33-34): stories 1 and 2.
2. source.read over a `connectors` binding (:33): story 2, `a_source_read_goes_through_the_relay`.
3. No local `connectors` program (:35): story 1 `the_write_path_starts_no_process`, story 2 `no_connectors_program_is_started_under_the_relay`.
4. No credential in the agent container (:35): story 1 `the_relay_client_sends_no_credential`, story 2 `the_upstream_sees_only_the_boundarys_token`, story 4 `the_boundary_container_and_the_agent_container_are_unchanged_by_an_http_egress`.
5. Input in the declared shape (:35-36): story 3, narrowed (finding 1).
6. A `writes.mode: auto` send is performed (:15): story 2, `an_auto_run_sends_its_reply_through_the_relay`.
7. Option (a): a namespace and pod-selector entry, no host, for credential-free `http` bindings (:48-49): story 4.
8. Loki, Prometheus and a health endpoint over GET bindings (:43-46): story 4. Requiring GET is not a narrowing, because `ess/domains/agent.yaml:498` already refuses any `http` method other than GET (`HttpMethod`). `bearer_env` is the only credential member of `HttpBinding` (`:273-292`).
9. Reply with the id, whether Loom takes it now, and the release (:11): named in e6-plan.md Open 2.
10. The bot-token instruction (:6-7): named in Open 1.
11. Loom's view on llm-gateway's audience and principal (:60-63): answered in `e6-llm-gateway-note.md`, and the epic's Not in scope names it.

**Reach checked and not a finding:**
- The Slack poll going through the relay follows from story 2's own one-transport-per-pod refusal.
- Applying the shape fix to the CLI transport too: request:27-31 states the shape defect without limiting it to the relay.
- The `loom_sdk` re-export follows AGENTS.md:172 and `sdk-stability.md:137`.
- `allowedBoundaryHttpEgress` restricts option (a); it does not add anything beyond it.
- Graph: no artifact outside the set claims either gap. `story:protocol-argument-schema` and `story:connector-effect-in-substrate` cover other ground.

**Story 4 and the invariant:** § Why does say what it lets through that the rule exists to stop. The rule is there to keep closed "a destination that admits by network position alone" (commit `85c92a0f`). The story says it lets through any process in the pod, any method and path, on the allowed pod port, with Loki push as its example. The changed rule text is in AGENTS.md § Operator (:1225-1227, :1463-1468) and in `admission.rs`, and both are in its Files list.

My reasons on the residual risk, for the operator's decision:
- **For (a):** the story's argument holds. A relay that adds no token cannot stop the agent container from going around it, because a NetworkPolicy selects pods, not containers. So (b) would cost more and protect no more.
- **Against, or the limits:** condition 1 only refuses the namespaces of upstreams this resource declares. An allowed entry could still name the namespace of a Connectors or git host the resource does not declare, and that is exactly what the adversary's original finding was about. Only the operator's list guards that case. The example also names only Loki push. Selected pods that act with their own credentials (a backend, Prometheus's admin API) are the larger exposure.

**What I read:** 1 parent request, 1 epic and 4 stories (`aep plan artifact show` on each), `aep plan artifact graph --format json`, a grep of the store for overlapping artifacts, e6-plan.md and e6-llm-gateway-note.md, AGENTS.md § Credential boundary and § Operator (egress text), `admission.rs:60-72, :1355-1362, :1462-1482, :1600-1618`, `ess/domains/agent.yaml:270-296`, and connectors `v0.73.0` `lib.rs:2870-2900` and Slack `operations.json:50-64`.

**What I could not establish:**
- Whether any operation the consumer uses has the mixed path/query-plus-body shape. Slack's `chat.postMessage` is body-only, so the consumer itself is covered.
- Out of my lane (design or acceptance): the epic's first acceptance bullet (`crates/loom-agent/tests/connectors_relay.rs` asserting `{body: {channel, text}}`) is owned by no story. Story 2 creates that file without the shape check, and story 3 tests only `crates/loom-connectors`.
- Out of my lane (acceptance): story 3 has no acceptance test for the no-credential invariant of its own.
- Out of my lane (acceptance): in story 4, the "a POST binding" case of `an_egress_no_get_binding_names_is_refused` cannot be built, because the document check (`HttpMethod`) refuses it before admission step 5.
- Out of my lane (design): in story 4, condition 3 accepts only `<svc>.<ns>.svc[.cluster.local]` hosts, and the `port` in `allowedBoundaryHttpEgress` is the pod port while `base_url` names the Service port.

```findings
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 27
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The body promises the declared shape for \"every catalog write that has a request body\", but its rule only handles writes whose input is the body alone. A catalog write that also has path or query parameters at the top level fits neither `flat` nor `body`, so it is refused `input-shape-unknown`, and neither the story's nor the epic's Out of scope records that this case was dropped."
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 42
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "The admission refusal `BoundaryOperationUndeclared` (a binding operation that `spec.boundary.connectors.operations` does not list) is not asked for by any sentence of the parent, and the epic's Outcome does not claim it. It is also the only reason this story edits `AdmissionRefusalReason` and `AdmitLoomAgent`, which is half of story 4's `depends_on` edge. Either drop it (the relay's 403, already in the epic's acceptance, covers the runtime case) or name it in the epic Outcome as an added refusal and give the reason."
```
