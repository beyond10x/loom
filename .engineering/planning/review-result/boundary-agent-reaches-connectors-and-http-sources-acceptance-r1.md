---
format: aep.planning-md/3
id: review-result:boundary-agent-reaches-connectors-and-http-sources-acceptance-r1
kind: review-result
status: active
title: Plan critic acceptance, round 1, on epic:boundary-agent-reaches-connectors-and-http-sources
relations:
- reviews: epic:boundary-agent-reaches-connectors-and-http-sources
- reviews: story:agent-selects-the-connectors-relay
- reviews: story:boundary-admits-declared-http-egress
- reviews: story:connectors-relay-client
- reviews: story:send-input-in-the-declared-shape
revision: 1
---
needs-revision

- `story:connectors-relay-client` — the outcome says what an answer lost or unreadable becomes (`Unknown`) but not what a definite non-2xx relay answer (403 for an unlisted operation, 401, 5xx, relay down) returns on a read or a keyed write, and no acceptance test has one, though story 2's 403 test depends on it — `.engineering/planning/story/connectors-relay-client.md:38-42` (`Unknown` for a lost answer is the only transport failure named) and `.engineering/planning/story/agent-selects-the-connectors-relay.md:100` ("the relay answers 403 ... the turn records the send refused").
- `story:connectors-relay-client` — outcome rules have no test among the acceptance bullets: `undeclared-kind` with no request, a read whose answer carries a `mutation` or a non-`complete` audit being an error, `WriteApproval` accepted and not sent, the `relay-unreachable` code the ESS comment names, and `Connectors` dispatching to `ConnectorsCli` and `ConnectorsRelay`. The check can pass with any of them wrong — `.engineering/planning/story/connectors-relay-client.md:29,31-32,42,74` against the test list at `:84-109`.
- `story:agent-selects-the-connectors-relay` — the outcome names the Slack poll's `list`, `get` and `search` and `agent serve`, but every acceptance test is `agent run` or `agent resume`, so a poll or `serve` that still starts a `connectors` program passes — `.engineering/planning/story/agent-selects-the-connectors-relay.md:18,23-24` against `:87-108`. The poll's tests exist (`crates/loom-plugin-slack/tests/poll.rs`), and `crates/loom-agent/tests/serve.rs` is only "unchanged".
- `epic:boundary-agent-reaches-connectors-and-http-sources` — the first acceptance bullet, an auto `agent run` whose invoke input is `{body: {channel, text}}`, names `crates/loom-agent/tests/connectors_relay.rs`. That file is story 2's, whose tests never ask for a body-shaped input, and story 3 lists no `loom-agent` test, so no story's acceptance owns the end-to-end check — `.engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md:79-83`, `.engineering/planning/story/send-input-in-the-declared-shape.md:67-83`.
- `story:send-input-in-the-declared-shape` — the credential-free invariant is not stated for the write path. Story 3 changes both transports' writes, but no bullet says the relay's shaped write still sends only the placeholder bearer and starts no process (story 1's `the_relay_client_sends_no_credential` and `the_write_path_starts_no_process` cover story 1's requests only) — `.engineering/planning/story/send-input-in-the-declared-shape.md:74`.
- `story:send-input-in-the-declared-shape` — `the_shaped_write_keeps_its_idempotency_key` names no value to compare. The outcome says the key "does not change", so the bullet needs to say it equals `cli::idempotency_key` of the admitted request, flat or under `body` — `.engineering/planning/story/send-input-in-the-declared-shape.md:77`.
- `story:boundary-admits-declared-http-egress` — "a new golden ... is blessed and reviewed" has "reviewed" as its check, which two people can answer differently. It should name the observable: the golden exists and `task check` is green with it — `.engineering/planning/story/boundary-admits-declared-http-egress.md:116-117`. Warning only.

**Invariant, story by story** (as the brief asks)
- Story 1 states it and a test can fail it (`the_relay_client_sends_no_credential`, `the_write_path_starts_no_process`).
- Story 2 states it. `the_upstream_sees_only_the_boundarys_token` and `no_connectors_program_is_started_under_the_relay` can fail, but only on the `run` path (finding 3).
- Story 3 does not state it (finding 5).
- Story 4 states it. The agent and boundary containers must be byte-equal (`:114`) and the binding must have no `bearer_env` (condition 3), so a test can fail it.

**Story 4's residual risk, and my reasons**
- **What it says.** The Why names what option (a) lets through. From any process in the pod, any method and path on the selected pods' port, because a NetworkPolicy is layer 4 and pod-wide. It also says only the in-process `HttpGet` holds GET-only.
- **What it keeps closed.** It names the adversary finding the boundary rule was written for, an upstream's namespace opened on further ports. Condition 1 and the upstream-namespace test keep that closed.
- **For (a).** Its claim that (b) lets through the same is correct. A pod-level policy cannot tell the boundary container from the agent container, and a tokenless relay adds nothing the upstream can check.
- **Against.** (a) is the first path where a destination that admits by network position alone is reachable from a pod whose point is holding no credential, and the story says so only in `## Why`. A prompt-injected agent process can reach an unauthenticated write API on the allowed port, such as Loki push or a Prometheus admin API. The mitigations are all operator-side: the list is empty by default, each entry names a selector and a port, and condition 3 binds each entry to a GET binding. Open point 6 rightly goes to Timo.
- **Not a finding.** I do not count this as a defect in the acceptance.

**What I read**
- Five artifacts, all given: the epic and its four stories, read in full with `aep plan artifact show <id>`.
- I also read `e6-plan.md` `## Open`, `admission.rs:1355-1380,1465-1485,1595-1620`, `serve.rs:145-165`, `loom-boundary/src/lib.rs:88-98`, `turn.rs:800-812`, and the test directories of `loom-connectors`, `loom-agent`, `loom-operator` and `loom-plugin-slack`.

**What I could not establish**
- I did not open Connectors, so I did not check its `adapters/catalog` line citations or the `v0.62.0` and `v0.73.0` claims. `RELAY_PLACEHOLDER` and `loom-boundary-relay` grep to nothing in `crates/`, so story 1 creates that constant.
- Out of my lane, not counted in the verdict:
  - Design: story 4's condition 1 compares only this resource's declared upstreams, so a namespace of an upstream the resource does not declare can be listed in `allowedBoundaryHttpEgress`. Nothing ties the entry's `port` to the binding's `base_url` port.
  - Scope or parallel-safety: "ESS first" in story 1 is comment-only, so a failing test on that commit says nothing about it.

```findings
- file: .engineering/planning/story/connectors-relay-client.md
  line: 38
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the outcome and acceptance name no result for a definite non-2xx relay answer (403 for an unlisted operation, 401, 5xx, relay down) on a read or a keyed write, yet story 2's test expects 'the turn records the send refused' from a relay 403"
- file: .engineering/planning/story/connectors-relay-client.md
  line: 84
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "outcome rules with no acceptance test: undeclared-kind with no request, a read answer carrying a mutation or a non-complete audit being an error, WriteApproval accepted and not sent, the relay-unreachable code, and the Connectors enum dispatching to both clients"
- file: .engineering/planning/story/agent-selects-the-connectors-relay.md
  line: 87
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the outcome names the Slack poll's list, get and search and agent serve, but every acceptance test is agent run or agent resume, so a poll or serve that still starts a connectors program passes"
- file: .engineering/planning/epic/boundary-agent-reaches-connectors-and-http-sources.md
  line: 79
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the first acceptance bullet (an auto run whose invoke input is {body: {channel, text}}) names crates/loom-agent/tests/connectors_relay.rs, a story 2 file whose tests never ask for a body-shaped input, and story 3 lists no loom-agent test, so no story's acceptance owns it"
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 74
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance does not state that the relay's shaped write still sends only the placeholder bearer and starts no process, so the credential-free invariant is not tested for the path this story changes"
- file: .engineering/planning/story/send-input-in-the-declared-shape.md
  line: 77
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the_shaped_write_keeps_its_idempotency_key names no value to compare; it should say the key equals cli::idempotency_key of the admitted request, flat or under body"
- file: .engineering/planning/story/boundary-admits-declared-http-egress.md
  line: 116
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the new golden is 'blessed and reviewed', and 'reviewed' is not observable; the acceptance should name the golden's existence and a green task check"
```
