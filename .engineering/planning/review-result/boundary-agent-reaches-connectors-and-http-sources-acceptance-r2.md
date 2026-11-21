---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-acceptance-r2
kind: review-result
status: active
title: Plan critic acceptance, round 2, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:connectors-relay-client
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

- `story:connectors-relay-client` — the same input has two outcomes, and no bullet says at which request a write's scripted answer applies: the unreachable-relay test expects `WriteError::Refused` `relay-unavailable` for the write ("no listener on the port"), but the first request a write sends is the describe, and the closing paragraph makes a describe's `relay-unavailable` a `WriteError::Failed` — `.engineering/planning/story/connectors-relay-client.md:125-126` against `:73-74` and the table at `:65`, `:67`. The refusal and unavailable tests (`:128-131`, "for a read and for a write") do not say whether the fake answers the describe or the invoke. The fix is to give one code per stage for a write, or to say which request each test scripts.
- `story:send-input-in-the-declared-shape` — the rule "a body the schema requires with no argument under it is refused `input-shape-unknown`" has no test. The cli and relay bullets list six cases (body, split, open, flat, ambiguous, and a schema declaring neither), and none of them gives a required body no argument. `ShapeWriteInput` takes `body_required`, but "one per outcome and per refusal" is one scenario per code, not per case — `.engineering/planning/story/send-input-in-the-declared-shape.md:32-33` against `:87`. The check passes with `body_required` ignored.
- `story:boundary-admits-declared-http-egress` — "the new golden `tests/golden/credential-free-http-egress.yaml` exists and `cargo test -p b10x-loom-operator --test render` passes against it" names no test that reads the golden. The golden tests are one function each (`render.rs:144`, `:488`, `:3140`), and the bullet that renders the egress resource (`:250-252`) asserts rules, not a file. So the run passes with the file present and unread — `.engineering/planning/story/boundary-admits-declared-http-egress.md:153-155`. The fix is to name the test, as `a_credential_free_command_resource_renders_as_its_golden_file` is for `credential-free.yaml`.
- `story:agent-selects-the-connectors-relay` — "every test of `render.rs` and `adversary_d243_u1.rs` passes with the helpers below changed and none returns early on a refusal it did not before" is a check nobody can run. It needs a diff of control flow in two files and no test observes it. The story's own reason is that two tests return on `RenderError::Refused` and so pass testing nothing — `.engineering/planning/story/agent-selects-the-connectors-relay.md:154-158`. The fix is a bullet with an observable: the helper asserts admission, or each named test asserts it reached `Ok(rendered)`.

**Round-1 findings: what held and what did not**
- Held: relay answers (the table at `connectors-relay-client.md:63-73` with tests `:111-146`; the reservation above is its describe/invoke stage); every outcome rule now has a test; the `Connectors` enum test is at `agent-selects-the-connectors-relay.md:765`.
- Held: poll, serve, resume and `source.read` tests; the end-to-end check moved to story 4 and the epic bullet names it. Story 3 now has the no-credential test and the key compared to `cli::idempotency_key`. The golden no longer says "reviewed", though it now has the unread-file hole above.
- I checked the boundary's code constants (`crates/loom-boundary/src/lib.rs:385-414`, `:646-656`) and the story's fixed codes and body shape. They match.
- `NoTransport` on "neither" does not break agents with no Connectors bindings: `SpecPlugin::build` already refuses a spec without `runtime.connectors` (`crates/loom-agent/src/serve.rs:155-159`), so the outcome keeps today's behaviour.
- `config.yaml:14` is the `llm` model upstream, so the moved scenario keeps `BoundaryEgressNamespace`.

**Invariant, story by story**
- Story 1 states it and a test fails it (`an_http_egress_adds_no_credential_variable_or_volume`, both containers' env, volumes and mounts).
- Story 2 states it (`the_relay_client_sends_no_credential`, `the_relay_client_starts_no_process`).
- Story 3 states it (`the_shaped_relay_write_sends_no_credential_and_starts_no_process`).
- Story 4 states it (`the_upstream_sees_only_the_boundarys_token`, `no_connectors_program_is_started_under_the_relay`, and the poll and serve tests).

**The egress story's residual risk, with my reasons for the operator**
- It now says what it lets through, per entry, in `## Why`: an unauthenticated write on the allowed port, a peer that forwards with its own credential, and the sidecar's reach. It keeps shut what the rule was written for. Condition 1 now covers every upstream the operator allows, not only the declared ones, which closes the gap I flagged as out of lane in round 1. The test at `:253-256` can fail it.
- For: the story is honest that (b) gives the agent the same reach, and the entry is opt-in and empty by default. It is bound to a `GET` binding with no `bearer_env` and a non-empty selector.
- Against: it is still the first path where a pod that holds no credential reaches a destination by network position alone. The only limit is the operator's list and one documentation sentence. Condition 3 ties an entry to a binding by namespace only, so a listed entry can name pods that have nothing to do with the binding. This is the operator's decision, and I raise no finding on it.

**What I read**
- Five ids, all read in full with `aep plan artifact show` (epic, four stories), plus the round-1 acceptance verdict.
- In the tree: `serve.rs:140-175`, `admission.rs:1595-1620`, `loom-boundary/src/lib.rs:380-420` and `:640-660`, `resource-admission/config.yaml`, `tests/render.rs` (golden tests), and `adversary_d243_u1.rs:270-300`.

**What I could not establish**
- I did not open Connectors, so I did not check the `adapters/catalog` line citations, the `v0.62.0` or `v0.73.0` claims, or `connectors-client`'s `read_v1alpha2` at `:319-330`.
- I did not confirm that a 403 `operation_refused` can only come at the invoke. I read the boundary's code constants, not `route.rs`.
- Out of my lane, and not in the verdict:
  - Design: `TransportSetting::Loopback` in the ESS input has to repeat `ConnectorsRelay::new`'s URL rules (no path, user information, query or fragment), or the scenarios and the Rust refusal will diverge.
  - Parallel-safety: the story-1 `Waits on` claim that the egress story "depends on no story" while story 3 edits `ess/ess-inputs.yaml` and the plan files. The coordinator decided the order; I mention only that the edge direction is as decided.

```findings
- file: .engineering/planning/story/connectors-relay-client.md
  line: 125
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the unreachable-relay test expects WriteError::Refused relay-unavailable for a write with no listener, but a write's first request is the describe and the paragraph at :73-74 makes a describe's relay-unavailable WriteError::Failed; the refusal and unavailable tests (:128-131) do not say whether the fake answers the describe or the invoke, so one input has two outcomes"
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 32
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the rule that a body the schema requires with no argument under it is refused input-shape-unknown has no acceptance test among the six cli and relay cases and no named scenario, so the check passes with body_required ignored"
- file: .engineering/planning/story/boundary-admits-declared-http-egress.md
  line: 153
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the new golden tests/golden/credential-free-http-egress.yaml 'exists and cargo test --test render passes against it', but no named test reads that file, so the run passes with the golden unread; name the golden test as the other three goldens have"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 156
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'none returns early on a refusal it did not before' is not observable, since no test or command shows a test returning early on RenderError::Refused; state the check as an assertion that the helper's resource is admitted or that each named test reaches Ok(rendered)"
```
