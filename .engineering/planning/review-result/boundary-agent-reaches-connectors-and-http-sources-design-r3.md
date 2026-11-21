---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-design-r3
kind: review-result
status: active
title: Plan critic design, round 3, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

story:connectors-relay-client — the body reuses `classified` "with no signature changes" to classify a relay write, but `classified` reads the CLI's projected answer (`ok`/`result`, `error.data`). The v1alpha2 `InvokeResponse` has no `ok`, puts `mutation`, `request_id` and `source_audit` at the top level, and allows `error` only `code`, `message` and `retry_after_seconds` (`deny_unknown_fields`). Passed in as it arrives, every relay write classifies `Unknown`, so the body must name the step that maps a decoded `InvokeResponse` into that projection, or classify the typed `MutationObservation` itself and drop `classified` from the reuse list — .engineering/planning/story/connectors-relay-client.md:73, :107; crates/loom-connectors/src/cli/write.rs:366-407; connectors `v0.62.0` crates/connectors-core/src/v1alpha2.rs:192-201, :226-236, :489-503

story:agent-selects-the-connectors-relay — the decision picks `relay` for bindings naming "at most one" pair, but with zero `connectors` bindings there is no pair to give `ConnectorsRelay::new(url, adapter, connection, sources)`. `src/transport.rs` has the same gap: it classifies the URL by calling that pair-taking constructor. The body must say what zero bindings builds (for example `Unset` with no client, or a refusal), and the URL rule must be reachable without a pair, as one relay function that both `new` and `transport.rs` call — .engineering/planning/story/agent-selects-the-connectors-relay.md:148, :160, :215; .engineering/planning/story/connectors-relay-client.md:57

**Round-2 finding (one instance per client): fixed in the stored bodies.**
- The relay story's Outcome (`connectors-relay-client.md:57-69`) has `new(url, adapter, connection, sources)`, a `relay-other-instance` refusal on every method before any request, and `over` keeping the pair.
- New test `a_call_naming_another_instance_sends_nothing` covers it.
- `SeveralInstances` reports the same rule at start: it names the bindings and `relay-other-instance`, and asserts the relay received no connection (`agent-selects-the-connectors-relay.md:125-129`, the test at :259-261).

**What I read:** 5 artifacts, each whole, with `aep plan artifact show`. I also read the ledger, the round-2 design verdict and the source.
- **Commands:** `relations`, `graph --format json`, `validate` ("valid"; it reports only review-outcome gaps elsewhere in the store), `waves`.
- **Edges walked:** 15 starting in the set (2 `serves` on the epic, `decomposes` + `serves` on each story, 5 `depends_on`), and outside the set the 35 inbound `reviews` edges.
- **Cycles and chain:** no cycle. The set is not a serialising chain: egress and relay have no edge between them.
- **Acceptance reads:** 4 lead to a producer in the set, and an edge records all 4.
  - shape reads relay's test file and write;
  - select reads relay's client, `relay-other-instance` and `new`;
  - select reads shape's `describe-write-body.json` and its shaped write;
  - select reads what egress leaves at `adversary_d243_u1.rs:282`, `render.rs:2586` and the new golden.
- **Source opened:**
  - `crates/loom-connectors/src/cli.rs:55-240, 600-725` and `cli/write.rs:315-420`
  - `crates/loom-connectors/src/lib.rs:200-235`
  - `crates/loom-plugin/src/lib.rs:150-210, 355-372`
  - `crates/loom-agent/src/serve.rs:140-200, 840-915`
  - `ess/domains/agent.yaml:300-316` and `datasource.yaml:59-74`
  - connectors `v0.62.0` `v1alpha2.rs` and `connectors-client/src/lib.rs:315-332`

**What I could not establish, and what is out of my lane:**
- **Can the source scans fail for their defect? Partly (acceptance lane).**
  - `the_relay_module_starts_no_process` and `the_shape_module_starts_no_process` fail for a process started directly in `src/relay.rs` or `src/shape.rs`.
  - They cannot fail for one reached through a call into `crate::cli`. This revision widens that module to `pub(crate)` for exactly that reuse, and the scan reads only the scanned file.
  - What does catch it is behavioural: the select story's marker tests (`no_connectors_program_is_started_under_the_relay`, `the_poll_under_the_relay_starts_no_process`, `the_enum_dispatches_to_the_client_it_holds`). Every relay test also checks answers from the fake service.
  - A tighter scan would also refuse any `cli::` path but the four reused items.
- **Wrong story in the epic, introduced this round (acceptance/scope lane).** The epic's "(same story)" bullet (`epic/…:107`) now follows the new relay-client bullet. So it credits the poll and `agent serve` no-process checks to `story:connectors-relay-client`. Those tests are in `story:agent-selects-the-connectors-relay` (`poll_relay.rs`, `agent_serve.rs`).
- **Egress file names disagree (scope / parallel-safety lane).** The egress story's typed scope entries name `boundary-unbound-egress-namespace.yaml`, `boundary-egress-namespace-not-given.yaml` and `boundary-allowed-http-egress.yaml`. Its ESS-first section and Scope prose name `boundary-egress-unbound.yaml`, `boundary-egress-not-allowed.yaml` and `boundary-http-egress-allowed.yaml`.
- **Stale Scope prose in the relay story (scope lane).** It still calls `cli.rs` and `cli/write.rs` inferred, and points `loom-sdk` at "Story defects". The typed entries now say cited.
- **Shared runtime not stated.** `ConnectorsRelay` must be `Clone` (`relay.clone()` in `SpecPlugin::host`; `over` shares the runtime) while it owns a Tokio runtime. `ConnectorsInvoker`'s runtime is an `Option<Runtime>` shut down in `Drop` (`lib.rs:228-235`), so the relay needs a shared, reference-counted runtime. The body does not say so; this is implementor detail, not a finding.
- **Security invariants, story by story.** Each story has a bullet a test can fail:
  - **No credential:** egress `an_http_egress_adds_no_credential_variable_or_volume`; relay `the_relay_client_sends_no_credential`; shape `the_shaped_relay_write_sends_no_credential`, with no boundary in front.
  - **Only the declared relay or destination:** `a_relay_url_that_is_not_loopback_is_refused_before_any_request`, `relay-other-instance` / `SeveralInstances`, and the NetworkPolicy's "exactly one more rule".
  - **Same write policy:** `a_relay_write_of_other_input_or_action_sends_nothing`, the key tests, `a_send_over_max_per_hour_or_outside_the_allow_list_reaches_no_relay`, `a_shaped_write_still_refuses_input_other_than_the_admitted_arguments`.
- **File-sharing edges.** shape → egress and select → egress each have their shared files written beside them. That is a trade-off, not a finding: neither edge costs a wave.

```findings
- file: .engineering/planning/story/connectors-relay-client.md
  line: 73
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body reuses `classified` with no signature change to classify a relay write, but `classified` reads the command line's projected answer (`ok`/`result`, `error.data`) while the v1alpha2 `InvokeResponse` carries `mutation`, `request_id` and `source_audit` at the top and an `error` of only `code`, `message`, `retry_after_seconds`, so every relay write passed as is classifies `Unknown`; the body must name the mapping of a decoded `InvokeResponse` into that projection, or classify the typed `MutationObservation` itself and drop `classified` from the reuse list"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 148
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the decision selects `relay` for bindings naming at most one pair, but with zero `connectors` bindings there is no pair to build `ConnectorsRelay::new(url, adapter, connection, sources)`, and `src/transport.rs` classifies the URL by calling that pair-taking constructor; the body must say what zero bindings builds (for example `Unset` with no client, or a refusal), and the URL rule must be callable without a pair, as one relay function that both `new` and `transport.rs` call"
```
