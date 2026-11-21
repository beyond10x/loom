---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-design-r2
kind: review-result
status: active
title: Plan critic design, round 2, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:connectors-relay-client
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

story:connectors-relay-client — `ConnectorsRelay` keeps `ConnectorsCli`'s signatures, so every read and write still takes an `adapter` and a `connection`, but it can reach only the one instance behind the relay and its body says nothing it does with a second pair. The single-instance rule lives only in `story:agent-selects-the-connectors-relay`'s `SeveralInstances`, so an SDK host using the re-exported client sends a second binding's write to the wrong instance with no refusal. Fix: the client refuses, before any request, a source or `WriteOperation` naming a second `adapter`/`connection` pair, and `SeveralInstances` surfaces that refusal — .engineering/planning/story/connectors-relay-client.md:18, :75, :165, :172; .engineering/planning/story/agent-selects-the-connectors-relay.md:44; crates/loom-connectors/src/cli/write.rs:150-162

**Round-1 findings: both fixed in the stored bodies.**
- **The 2/3 integration (blocker).** `story:agent-selects-the-connectors-relay` now owns `an_auto_run_sends_its_reply_through_the_relay_in_the_declared_shape` and has `depends_on story:send-input-in-the-declared-shape`. It runs in the coordinator's direction, not the one I proposed. That is decided and closes the seam.
- **The `Connectors` enum.** It moved to the agent-selection story, which has its dispatch test (`connectors_enum.rs`). The relay client's Out of scope (`connectors-relay-client.md:171-172`) points there.

**What I read:**
- **Artifacts:** 5, each read in full with `aep plan artifact show`. Also ran `relations`, `graph --format json` and `validate` ("valid"; it reports only unrelated review-outcome gaps).
- **Edges:** I walked all 15 edges that start in the set: 2 `serves` on the epic, `decomposes` + `serves` on each story, and 5 `depends_on`. I also walked outside the set: the targets `vision:O1` and `vision:governed-autonomy`, and the 20 inbound `reviews` edges from the round-1 review-results. Nothing else points in.
- **Cycles:** none. The `depends_on` edges are shape→relay, shape→egress, select→relay, select→shape and select→egress. Waves: {egress, relay}, then shape, then select.
- **Acceptance reads:** 4 story-level reads lead to a producer in the set, and an edge records all 4:
  - shape reads relay's `connectors_relay.rs` and its write;
  - select reads relay's client;
  - select reads shape's Slack fixture and shaped write;
  - select reads egress's `render.rs`/`adversary_d243_u1.rs` state.
  The epic's acceptance lines read select (lines 1-3) and egress (line 4).
- **Source opened** (tree `61fd500b`; only plan commits since `39f4da88`, no `crates/` change):
  - `crates/loom-connectors/src/cli.rs:125-260`, `cli/write.rs:140-175`
  - `crates/loom-boundary/src/lib.rs:540-572`
  - `crates/loom-plugin/src/effects.rs:380-400`
  - `crates/loom-connectors/Cargo.toml`
  - `crates/loom-operator/tests/golden/`
  - connectors `v0.62.0`: `connectors-core` `v1alpha2.rs:270,317,368,717`, `lib.rs:103`. The types the relay story names are in `connectors-core`, which `Cargo.toml:13` already pins.

**What I could not establish:**
- **File-sharing edges, a trade-off and not a finding.** `story:send-input-in-the-declared-shape` → `story:boundary-admits-declared-http-egress` (shared files: `ess/ess-inputs.yaml`, `generated/rust/loom/`) and select → egress (`admission.rs`, `tests/render.rs`, `ess/scenarios/resource-admission/`) both have their reason written beside them.
  - Neither costs a wave: shape and select already wait on stories from wave 1 or later.
  - The alternative is to split the shared files, and it buys nothing here.
- **The egress story's new golden is missing from the select story's breakage list (scope/acceptance lane).** The select story's "What the refusal makes fail" (`agent-selects-the-connectors-relay.md:171`) lists the goldens and test lines as they are at `39f4da88`. It omits `tests/golden/credential-free-http-egress.yaml` and the new `render.rs` tests that egress adds first (`boundary-admits-declared-http-egress.md:154`). If that golden's resource declares `spec.boundary.connectors`, the select story's helper change re-renders it. The edge is there, so the order is right; the list is incomplete.
- **A test that cannot fail (acceptance lane).** `the_upstream_sees_only_the_boundarys_token` (`agent-selects-the-connectors-relay.md:120`) cannot fail if the agent sends a credential itself. The boundary drops `authorization`, `proxy-authorization`, `x-api-key` and `cookie` from the agent and inserts its own token (`crates/loom-boundary/src/lib.rs:551-567`). Only `the_relay_client_sends_no_credential` in the relay story can catch a client that sends one.
- **The invariant, story by story.** Each story states the no-credential invariant as a test:
  - egress: `an_http_egress_adds_no_credential_variable_or_volume`;
  - relay: `the_relay_client_sends_no_credential`;
  - shape: `the_shaped_relay_write_sends_no_credential_and_starts_no_process`;
  - select: the marker tests and the token test (with the caveat above).
  The select story changes no rendered pod.
- **Option (a), for the operator's decision.**
  - **What it now names.** The egress story's § Why names everything the rule (`Reason::BoundaryEgressNamespace`, commit `85c92a0f`) exists to stop that (a) lets through: the write API on the read port, a peer that forwards with its own credential, and the sidecar's reach. Its Limits paragraph says admission ties an entry to a binding by namespace only.
  - **What it adopted from round 1.** The operator-page sentence, with a `chart.rs` test. Condition 1 now covers every allowed upstream.
  - **One gap left unnamed.** `allowedBoundaryHttpEgress` is cluster-wide and has no field for the resource's own namespace. An entry allowed for one team's agents opens to any credential-free resource, in any namespace, that declares a matching binding. `allowedBoundaryUpstreams` works the same way, so this follows today's model, but the operator should know it.
  - **My advice is unchanged:** accept (a), with the list empty by default.

```findings
- file: .engineering/planning/story/connectors-relay-client.md
  line: 18
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "`ConnectorsRelay` keeps `ConnectorsCli`'s signatures, so every read and write still takes an `adapter` and a `connection`, but it can reach only the one instance behind the relay and its body says nothing it does with a second pair. The single-instance rule lives only in `story:agent-selects-the-connectors-relay`'s `SeveralInstances`, so an SDK host using the re-exported client sends a second binding's write to the wrong instance with no refusal. Fix: the client refuses, before any request, a source or `WriteOperation` naming a second `adapter`/`connection` pair, and `SeveralInstances` surfaces that refusal"
```
