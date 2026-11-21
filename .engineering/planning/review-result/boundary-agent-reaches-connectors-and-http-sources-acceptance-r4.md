---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-acceptance-r4
kind: review-result
status: active
title: Plan critic acceptance, confirming pass (round 4), on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:connectors-relay-client
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

- story:connectors-relay-client — the acceptance tells the fake to write every invoke answer with `connectors-core`'s `encode`, but at `v0.62.0` `encode` applies `first_binding_rules`. Those rules refuse `replayed: true` ("this binding does not replay") and every non-base code except `outcome_unknown`, so the bodies of `a_replayed_write_is_applied_naming_the_original_attempt` (`:250`) and the 403 `approval_required` case (`:238`) cannot be built. Write answers need `encode_keyed`, and the replay body must also carry `attempt`, which § 5 requires beside `original_request_id` — .engineering/planning/story/connectors-relay-client.md:175; connectors `v0.62.0` crates/connectors-core/src/v1alpha2.rs:629-651, :605-610, :732-748
- story:connectors-relay-client — `a_relay_write_applied_carries_its_classification_audit_and_body` scripts an invoke answer with a `source_audit`, but on this binding `first_binding_rules` and `keyed_binding_rules` both refuse `source_audit`. So neither encoder can build that body, `decode_keyed` reads it as `upstream_protocol`, and the expected `Applied` with the response's `source_audit` cannot be observed. The test must drop `source_audit`, and the Outcome's "`source_audit` is the response's" must say it is always absent at `v0.62.0` — .engineering/planning/story/connectors-relay-client.md:180, :96; connectors `v0.62.0` crates/connectors-core/src/v1alpha2.rs:630-631, :691-692
- story:boundary-admits-declared-http-egress — every refusal case of condition 4 has exactly one allowed entry, and the only case with two allowed entries expects an admission. A gate that checks namespace, selector and port each against *any* allowed entry therefore passes all seven tests. Given allowed `{monitoring,[app=metrics],9090}` and `{observability,[app=logs],3100}`, such a gate admits `{monitoring,[app=logs],3100}`. One refusal case is missing: an entry whose three values are each found in a different allowed entry, with no single entry matching all three — .engineering/planning/story/boundary-admits-declared-http-egress.md:222-237, against :81-85
- story:agent-selects-the-connectors-relay — the Outcome says "Every Connectors call of the agent goes through `ConnectorsRelay`". But `agent serve`'s A2A listener builds its own plugin with `SpecPlugin::new(agent.spec, agent.state)`, which this body keeps building `Cli` and leaves unchanged. Under the relay, with no `runtime.connectors`, that build fails "`runtime` names no connectors command line" and stops the host. That covers every `expose.a2a` agent, and every zero-binding (A2A-only) serve, where `Relay(None)` was meant to give an `Unset` host. No acceptance test runs an A2A turn or an A2A-only serve under the relay, so this break passes every test — .engineering/planning/story/agent-selects-the-connectors-relay.md:113, :120, :150; crates/loom-cli/src/a2a.rs:136; crates/loom-agent/src/serve.rs:155-159

**Round-3 fixes:** all seven acceptance fixes hold in the stored bodies and can be tested as written.
- Egress gate: seven named tests (`boundary-admits…md:216-237`).
- Go-away fake: closes its listener and sends `connection: close` (`connectors-relay-client.md:211-215`). `tests/connectors_invoker.rs:276` holds.
- Shape case 7: no `argument`, message names `body`.
- Shape case 8: `type: array` with `properties`, asserts no `argument`.
- Shape case 9: asserts no `argument`.
- `Unset`: three tests.
- Binary scenarios: each run fails at the stage after the transport, with stated observables; the blank value runs as `""` and `"  "`.
- Epic `:103-104`: names story 4.

The design fixes also hold: `write_result`, `loopback_url` taking no pair, `Relay(None)`, and the `cli::` allow-list scan.

**Brief Q2 (egress gate):**
- None of the five named wrong gates survives:
  - subset or superset comparison: the strict subset and strict superset tests;
  - namespace ignored: the `observability` test;
  - port ignored: the 9090 test;
  - label keys only: the `app=metrics` test;
  - empty list read as allow-all: the empty-list test;
  - list order compared: the reordered exact match.
- The cross-entry gate above survives.

**Brief Q3 (relay answer table):**
- Every row has a named test. The rows are driven by real v1alpha2 bodies except where finding 1 makes the body impossible to build.
- A replay has a stated result: `Applied`, with `request_id` set to `original_request_id`.
- Not tested, all minor: a boundary 400; `upstream_failed` and "cannot connect" for a read at its invoke; a timeout after the request left.

**Brief Q4 (zero bindings, `Unset`):**
- Both refusals the Outcome states are tested, on `Host::new` with no relay: `run_plugin_on` and `run_item`.
- `SpecPlugin::host()` under `Relay(None)` is not tested directly.
- The A2A entry is finding 4.
- `resume` states no refusal; it would meet the `no-connectors-transport:` answers, which `unset_answers_…` tests.

**What I read:**
- All 5 artifacts in full with `aep plan artifact show`: the epic and the four stories. `aep plan artifact validate` reports "valid".
- `git diff HEAD --stat`, the round-3 acceptance and design verdicts, and `e6r3-changes.md`.
- Tree:
  - `crates/loom-cli/src/main.rs:255-325,440-500`
  - `crates/loom-cli/src/a2a.rs:130-155`
  - `crates/loom-agent/src/serve/a2a.rs:310-345`, `crates/loom-agent/src/serve.rs:150-162`
  - `crates/loom-connectors/src/cli.rs` (public items, `:160-225`), `crates/loom-connectors/src/cli/write.rs` (public items)
  - `crates/loom-operator/tests/render.rs:2586`, `adversary_d243_u1.rs:280-295`, `chart.rs:288-332`
  - `crates/loom-connectors/tests/connectors_invoker.rs:270-280,1100-1135`, `connectors_cli.rs:463-467`
  - `ess/domains/datasource.yaml:170-200`
- Connectors `v0.62.0` `v1alpha2.rs` (`:36-240`, `:485-754`).

**What I could not establish:**
- Whether the A2A listener also needs a turn test, or only the plugin build fixed. The root cause of finding 4 is design, which is not my lane.
- I did not re-check the catalog's `v0.73.0` line citations.
- The relay scan allows no `cli::SourceEntities`, although `sources` returns it (`cli.rs:165`). It can be met by writing `Vec<(ReadKind, SourceEntity)>`; this is friction, not a defect.

```findings
- file: .engineering/planning/story/connectors-relay-client.md
  line: 175
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance tells the fake to write every invoke answer with connectors-core's encode, but at v0.62.0 encode applies first_binding_rules, which refuse replayed: true and every non-base code but outcome_unknown, so the bodies of a_replayed_write_is_applied_naming_the_original_attempt and the 403 approval_required case cannot be built; write answers need encode_keyed, and the replay body must also carry attempt, which section 5 requires beside original_request_id"
- file: .engineering/planning/story/connectors-relay-client.md
  line: 180
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "a_relay_write_applied_carries_its_classification_audit_and_body scripts an invoke answer with a source_audit, but on this binding first_binding_rules and keyed_binding_rules both refuse source_audit, so neither encoder can build that body, decode_keyed reads it as upstream_protocol, and the expected Applied with the response's source_audit cannot be observed; the test must drop source_audit, and the Outcome must say it is always absent at v0.62.0"
- file: .engineering/planning/story/boundary-admits-declared-http-egress.md
  line: 222
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "every refusal case of condition 4 has exactly one allowed entry, and the only case with two allowed entries expects an admission, so a gate that checks namespace, selector and port each against any allowed entry passes all seven tests; one refusal case is missing: an entry whose three values are each found in a different allowed entry, with no single entry matching all three"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 120
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says every Connectors call of the agent goes through ConnectorsRelay, but agent serve's A2A listener builds its plugin with SpecPlugin::new (crates/loom-cli/src/a2a.rs:136), which this body keeps building Cli and leaves unchanged, so under the relay with no runtime.connectors it fails 'runtime names no connectors command line', and no acceptance test runs an A2A turn or an A2A-only serve under the relay to catch it"
```
