---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

- story:send-input-in-the-declared-shape — epic says stories 2 and 3 "share no file but `AGENTS.md` and `CHANGELOG.md` and may run in one wave", but they collide on several surfaces. The collision shows in two places. First, both ESS-first commits add scenario lines to `ess/ess-inputs.yaml` (cited: story 2 `agent-selects-the-connectors-relay.md:70` and `:122`; story 3 `send-input-in-the-declared-shape.md:63` and `:89`). Second, both add an arm to the one command `match` in `crates/loom-conformance/src/lib.rs:245-251` (`AdmitConnectorsTransport` and `ShapeWriteInput`; the file is inferred from `lib.rs:220-262`, both bodies cite only `crates/loom-conformance/src/`). Both also regenerate `generated/rust/loom/plan.json` and `PLAN.md`, whole-spec aggregates that both bodies list as `generated/rust/loom/`. Both edit `AGENTS.md § Connectors` itself, not just the file. Story 3 likely also adds `b10x-loom-connectors` to `crates/loom-conformance/Cargo.toml` and `Cargo.lock` (inferred; the crate depends only on `loom-agent`, `Cargo.toml:11`). Stories 3 and 4 also share `plan.json`, `PLAN.md`, `AGENTS.md` and `CHANGELOG.md`, and no edge or sentence names that pair. The epic body is the one to change, at `epic/boundary-agent-reaches-connectors-and-http-sources.md:73-74`. Two remedies, and I do not choose: (a) an ordering edge 3→2 (and 4→3) whose reason is the shared `ess/ess-inputs.yaml`, `lib.rs` match and generated plan files; (b) split the surface so they stop sharing it. Edges 2→1, 3→1 and 4→2 are already recorded.
- story:boundary-admits-declared-http-egress — Files omit `ess/ess-inputs.yaml`, though the four new `resource-admission` scenarios must be listed there. That file lists every scenario one by one (`ess/ess-inputs.yaml:77-98` holds the existing ones). Stories 2 and 3 both list it, so the shared file is hidden on this side (cited: `boundary-admits-declared-http-egress.md:128-137`, ESS first at about `:91-97`).
- story:agent-selects-the-connectors-relay — its acceptance reads state story 3 changes (`depends_on` missing): the keyed write over the relay (`an_auto_run_sends_its_reply_through_the_relay`, `:87`) goes through the write path story 3 changes. Its "unchanged" guards (`:109`: `crates/loom-agent/tests/serve.rs`, `crates/loom-cli/tests/agent_run.rs`) install the fixture `crates/loom-connectors/tests/fixtures/connectors/fake-connectors`. That fixture is read at `crates/loom-cli/tests/agent_run.rs:488`, `agent_serve.rs:596`, `plugin.rs:370` and `crates/loom-plugin/tests/support/mod.rs:84`, and story 3 edits it (`send-input-in-the-declared-shape.md:93`). Story 2's body never says which describe schema its fake service answers (the fixture `describe-write.json` is flat `channel`, `text`). Once story 3 lands, a fake whose schema declares neither refuses `input-shape-unknown`, so the verdict depends on landing order. Missing edge: story 2 `depends_on` story 3, or the two merge.
- epic:boundary-agent-reaches-connectors-and-http-sources — its first acceptance line (`:80`) asserts `{body: {channel, text}}` in `crates/loom-agent/tests/connectors_relay.rs`. Story 2 creates that file (`agent-selects-the-connectors-relay.md:~83`) and story 3 produces the shape. Story 3's Files hold only `crates/loom-connectors` tests (`send-input-in-the-declared-shape.md:~90-94`), and no story's body adds the assertion. With 2 and 3 declared concurrent, nobody owns it. The epic should name the story, which needs the edge above.

Named, so not findings:
- **4 against 2:** the `admission.rs` step 5, `AdmissionRefusalReason`, `resource.yaml` and `config.yaml` collision, with an edge and reason.
- **3 against 1:** the shared `relay.rs`, with an edge.
- **4 against `story:credential-free-workspace-isolation`:** the `admission.rs` overlap, stated as "no edge; the second to land rebases" (`boundary-admits-declared-http-egress.md:~138`). It is outside the set; I leave it to the operator.

**What I read.**
- 5 artifacts: `aep plan artifact show` on the epic and four stories, and `graph --format json` for their edges. `aep plan artifact waves` lists all four stories as unassessed, because there are no scope entries yet.
- Source at `39f4da88`: `ess/ess-inputs.yaml`, `crates/loom-conformance/src/lib.rs`, `crates/loom-connectors/tests/fixtures/connectors/*` and their users, `crates/loom-operator/src/{admission,render}.rs`, `tests/render.rs`, `tests/golden/credential-free.yaml`, and `examples/agents/operator.yaml`.
- Surfaces: 4 stories placed cited, 0 inferred, 0 unplaced. The epic holds no surface of its own. Four sub-surfaces are inferred: the `lib.rs` match, `plan.json`/`PLAN.md`, the conformance `Cargo.toml` and `Cargo.lock`, and the fixture script.
- Acceptance reads traced to a producer in the set: 7. A `depends_on` edge records 4: 2→1, 3→1, 4→2, and 1's "unchanged `connectors_cli*.rs`" against 3's edit of `connectors_cli_write.rs`. Three have none: story 2's write tests against story 3's shaping, story 2's "unchanged" guards against story 3's fixture edit, and the epic's `{body}` assertion.

**What I could not establish.**
- Whether `generated/rust/loom/plan.json` and `PLAN.md` change when only ESS comments change (story 1 lists no `generated/`).
- Whether `ess/ess-inputs.yaml` entries could be added by only one story if the scenario lists were split. Only a person can say.
- Whether any other test than `render.rs`'s example-wrapped ones feeds a `runtime.connectors` fixture to admission. I did not trace the rest.

**Out of my lane, which does not set the verdict.**
- *Acceptance:* the existing scenario `a-boundary-beside-an-egress-namespace-is-refused` (`resources/boundary-egress-namespace.yaml`) has namespace `monitoring`, a selector, no `http` binding and no allowlist. Under story 4's conditions 1-4 it hits condition 3 (`BoundaryEgressUnbound`), not `BoundaryEgressNamespace`, so "keeps its reason" (`boundary-admits-declared-http-egress.md:~95`) needs checking.
- *Acceptance:* story 2's `TwoTransports` refusal meets `examples/agents/operator.yaml:40-42` (`runtime.connectors`). `render.rs` wraps that example in `boundary(true, true, true)` many times (`:1035`, `:1967`, `:2294`, `:2437`, `:2517` and others), and the golden `credential-free.yaml:22` carries `connectors.toml`. Story 4 says the goldens are unchanged, and story 2's Files omit the example and fixtures.
- *Acceptance:* story 1's first commit changes ESS comments, but its Files omit `generated/rust/loom/` (drift).
- *Design, the residual risk of option (a):*
  - **For:** the default list is empty. Each opening is one operator-listed entry (namespace, labels, port). Condition 1 keeps the D243 adversary fix (no upstream namespace). Option (b) would let through the same traffic, since the policy selects pods, not containers, as the story's own comparison says.
  - **Against:** the destination is admitted by network position alone, which is the property `Reason::BoundaryEgressNamespace` (`admission.rs:1473-1480`) exists to stop. The opening is pod-wide: any process in the pod gets any method and path on the port, and the boundary sidecar gets it too. Condition 3 ties an entry to the spec author's own GET binding. Beyond the operator's list, only `HttpGet` in process stops a write.
  - **Gap:** the acceptance tests assert the pod spec is unchanged, but none fails if a credential or env var appears on a resource with the entry. The invariant ("no credential, reaches only relays") is not restated as a failing test for the changed network rule.
  - The operator should decide with that list in front of them.

```findings
- file: .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md
  line: 73
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the body says stories 2 and 3 share no file but AGENTS.md and CHANGELOG.md and may run in one wave, yet both land on ess/ess-inputs.yaml, AGENTS.md section Connectors, generated/rust/loom/plan.json and PLAN.md (cited, both bodies' ESS first and Files) and on the command match in crates/loom-conformance/src/lib.rs plus conformance Cargo.toml and Cargo.lock (inferred from lib.rs:220-262); stories 3 and 4 share plan.json, PLAN.md, AGENTS.md and CHANGELOG.md; none is named, so record an ordering edge with the shared file as reason or split the surface"
- file: .engineering/planning/story/boundary-admits-declared-http-egress.md
  line: 128
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Files omit ess/ess-inputs.yaml although the four new resource-admission scenarios must be listed there (ess/ess-inputs.yaml:77-98 lists each scenario), the same file stories 2 and 3 edit, so the shared surface is hidden on this side"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 109
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its relay write tests and its unchanged guards on the shared fake-connectors fixture (read at crates/loom-cli/tests/agent_run.rs:488, agent_serve.rs:596, plugin.rs:370, crates/loom-plugin/tests/support/mod.rs:84) read the write path and fixture story:send-input-in-the-declared-shape changes (its Files line 93), and no depends_on edge runs from story 2 to story 3, so the two are scheduled at once and the outcome depends on landing order; add the edge or merge the two"
- file: .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md
  line: 80
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the first acceptance line asserts the {body: {channel, text}} input in crates/loom-agent/tests/connectors_relay.rs, a file story 2 creates while story 3 produces the shape and lists only loom-connectors tests, and with 2 and 3 declared concurrent no story's Files own the assertion"
```
