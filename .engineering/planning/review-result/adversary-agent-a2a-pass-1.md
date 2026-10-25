---
format: aep.planning-md/3
id: review-result:adversary-agent-a2a-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:agent-a2a, unit commit de057abe on base 62ea2a89
relations:
- reviews: story:agent-a2a
revision: 1
---
Adversary pass 1 on `story:agent-a2a` (wave 2026-10-25-agents, unit tree `loom-w2-agent-a2a`, branch `impl/agent-a2a`, unit commit `de057abe` on base `62ea2a89`), `aep:adversary` (opus), 2026-10-25. The report, in the adversary's own words except where shortened.

```
unit: story:agent-a2a, tree loom-w2-agent-a2a at de057abe (base 62ea2a89) plus two test-file edits
verdict: NEEDS-CHANGE
cases: executed 289→294, red 4
origin: introduced 7 / pre-existing 0 / undecided 0
needs-coordinator: whether the no-channel gap is fixed in this unit or gets its own story (coordinator: in this unit)
```

**Cases** appended to `crates/loom-a2a/tests/a2a.rs` (+103) and `crates/loom-agent/tests/a2a.rs` (+75), each run alone before the suite:

| case | now | output when first run |
|---|---|---|
| `adversary_the_liveness_probe_is_answered_while_unauthenticated_peers_hold_every_slot` | red | `GET /live while 32 idle unauthenticated connections are open: {"error":"too many connections"}` left: 503, right: 200 |
| `adversary_a_head_trickled_one_byte_at_a_time_is_cut_off` | red | `a head trickled one byte per 400 ms was still being read after 25.296207328s (cut off: None)` |
| `adversary_an_unauthenticated_request_is_refused_before_its_announced_body_arrives` | green | guards D4, which no existing case tested |
| `adversary_a_bearer_with_a_secret_files_trailing_newline_still_admits_its_token` | red | `the listener started with a bearer no request can present` left: `WrongBearer {..}`, right: `Admitted` |
| `adversary_a_spec_exposing_a2a_without_a_channel_answers_its_messages` | red | `a spec with expose.a2a and no channel cannot be served: ... channel is undeclared; nothing would poll` |

Suite after the cases: the five-crate gate exited 101, 290 passed, 4 failed, all four the adversary's.

**Judgement.** `SpecPlugin::new` refuses a spec without `channel` (`serve.rs:124-127`), so the design's slice A spec cannot be served. A connection takes one of the 32 slots before admission (`server.rs:202-204`), so peers without the bearer fill them and `/live` answers 503; a failing liveness probe restarts the pod. The 10 s timeout is per read with nothing bounding the whole head (`http.rs:223`). The stored token is not trimmed while the presented one is (`admit.rs:100-103`), so a Secret file ending in a newline makes every request 401. The caller's `messageId` becomes the item revision unchanged and is printed through `inert()`, which escapes only `<>&` (`server.rs:339-343`). ESS conformance checks only the three-boolean decision; a server that skips `admit()` passes (`crates/loom-conformance/src/a2a.rs:28`). The "sends nothing" case scripts `reply_send` after the turn ends, so it stays green with `retain(SEND)` removed (`crates/loom-agent/tests/a2a.rs:153`).

**Held up:** constant-time bearer comparison, only the length leaks; no bearer in Debug output, errors, stderr, the card or the 401 body; the connectors child runs with `env_clear`; the body is not read before the 401; odd id types and an unknown method get a JSON-RPC error; a message continuing a task is refused; the card says no streaming and no push and leaks neither spec nor model route; a panicking turn ends its task failed; only ended tasks are evicted; with the default 0.0.0.0 the host still will not start without a bearer.

**Fix round (opus implementor, same tree, uncommitted on de057abe at the time of this record):** ESS first, `loom.agent.AdmitServe` with 3 scenarios (21 valid). D9 the command; D10 a channel-less A2A spec runs the listener as the host's main loop until SIGTERM, holding the state lock, `--once` refused; D11 unadmitted connections hold one of 32 unadmitted slots (the oldest cut off by a 33rd), admitted requests one of 32 admitted slots, probes and refusals take no admitted slot; D12 deadlines per request part (head 10 s, body 30 s, answer 10 s, drain 1 s), a late head or body gets 408; D13 the bearer is trimmed and a control character refuses start; D14 a plain-token `messageId` is kept, any other becomes `m-` plus a 128-bit FNV-1a hash; D15 the projection filter moved into `answer_projection` and `an_a2a_turn_is_never_offered_reply_send` goes red without `retain(SEND)` (verified once, restored). Gate after the round: all 5 adversary cases pass, 641 passed in the six-crate run with the 43 base ESS-gate failures under ess 0.69.2; clippy, fmt, `task drift`, `task docs-check` clean. All seven findings: `review_outcome` fixed (the conformance note by a line in the qualification doc).

```findings
[
{"file":"crates/loom-agent/src/serve.rs","line":124,"category":"acceptance","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"SpecPlugin::new refuses a spec without channel, so the slice A agent (expose.a2a, http reads, no chat) cannot be served; red: adversary_a_spec_exposing_a2a_without_a_channel_answers_its_messages"},
{"file":"crates/loom-a2a/src/server.rs","line":202,"category":"concurrency","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"connections take a slot before admission, so 32 idle unauthenticated peers make GET /live answer 503; red: adversary_the_liveness_probe_is_answered_while_unauthenticated_peers_hold_every_slot"},
{"file":"crates/loom-a2a/src/http.rs","line":223,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"READ_TIMEOUT is per read with no deadline on the head, so a trickling peer holds a slot indefinitely; red: adversary_a_head_trickled_one_byte_at_a_time_is_cut_off"},
{"file":"crates/loom-a2a/src/admit.rs","line":100,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"Bearer::new keeps a trailing newline that the trimmed presented token never has, so every request is 401; red: adversary_a_bearer_with_a_secret_files_trailing_newline_still_admits_its_token"},
{"file":"crates/loom-a2a/src/server.rs","line":339,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"caller messageId becomes item.revision, printed in the turn context through inert() which leaves newlines and backticks, so caller text lands outside the quoted item text"},
{"file":"crates/loom-conformance/src/a2a.rs","line":28,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the ESS scenarios check only the boolean decision table; a server that skips admit() passes conformance and is caught only by the integration tests"},
{"file":"crates/loom-agent/tests/a2a.rs","line":153,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the sends-nothing case scripts reply_send after reply.propose ends the turn, so it stays green with retain(SEND) removed"}
]
```
