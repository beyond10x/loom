---
format: aep.planning-md/3
id: review-result:adversary-agent-serve-pass-1
kind: review-result
status: active
title: Adversary pass 1 on story:agent-serve, unit commit 91fc7bc8 on base b40e1bb9
relations:
- reviews: story:agent-serve
revision: 1
---
Adversary pass 1 on `story:agent-serve` (wave 2026-10-25-agents, unit tree `loom-w2-agent-serve`, branch `impl/agent-serve`, unit commit `91fc7bc8` on base `b40e1bb9`), `aep:adversary`, 2026-10-25. The report, in the adversary's own words except where shortened.

```
unit: story:agent-serve, head 91fc7bc8 (base b40e1bb9)
verdict: NEEDS-CHANGE
cases: executed 288→291, red 3
origin: introduced 3 / pre-existing 0 / undecided 0
needs-coordinator: none
```

**Cases added** (`crates/loom-agent/tests/serve.rs` +144, `crates/loom-cli/tests/agent_serve.rs` +72), each run alone first:

| case | asserts | red output (verbatim) |
|---|---|---|
| `two_approved_sends_on_two_items_carry_two_idempotency_keys` | two held, approved and resumed items carry two `--idempotency-key` values | `assertion left != right failed ... left: "loom-action-request-00000000-0000-4000-8b00-000000000009" right: "loom-action-request-00000000-0000-4000-8b00-000000000009"` |
| `a_resume_refused_before_any_process_keeps_the_send_held` | a resume refused RateLimited leaves the held file and Suspension | `assertion left == right failed: a send refused before any process stays held for the next resume  left: []  right: ["79932101-2c84-4799-8b2d-2065b2a8c7a0"]` |
| `a_relative_state_directory_performs_the_granted_send` | `--state agent-state` (relative) serve/approve/resume exits 0 and writes once | `the send was not performed: refused: the send was refused (approval-not-absolute): the approval proof path agent-state/grant-3866...6634.json is not absolute  left: Some(5) right: Some(0)` |

Suite after the cases: `cargo test --locked --no-fail-fast -p b10x-loom-agent -p b10x-loom-cli -p b10x-loom-plugin -p b10x-loom-plugin-slack -p b10x-loom-conformance` → `EXIT=101`, 288 passed, 3 failed.

**Judgement.** Turn ids restart at 1 per turn (`turn.rs:948-961`, `write.rs:71`), so two items whose turns take the same steps suspend requests with the same id and the same idempotency key; `a_send_is_held_until_approved` compared the key to the same code's request id and could not see it. `serve.rs:449` removes the held file after any resume, so a pre-process refusal (rate limit, proof path, thread mismatch) drops the approved send. `serve.rs:302-309` keeps `--state` as given, so a relative path yields a proof path Connectors refuses. `agent resume` takes no state lock, so it and a serving host can both resume one grant. Two INFEASIBLE notes: conformance drives `admit_send` on declared booleans only, and no run against real Connectors was possible.

**Could not break:** `writes.mode: auto` refused at `SpecPlugin::new` before any process; refused spec → no state dir, no read; the grant path (capability check, grant tied to the suspended request, hand-edited held file refused on mismatch, replay → `NotHeld`); the allow-list walk; `sends.jsonl` window edges. SIGTERM mid-item not attacked (needs a stalling model).

**Fix round (implementor, same tree, uncommitted on 91fc7bc8 at the time of this record):** D10 `Ids::seeded(commission)` puts FNV-1a of the commission id in the id's first two groups, the counter stays in the last; D11 `SendPolicy::admission` runs before any process in `SpecPlugin::resume`, a refusal keeps file, Suspension and grant, only an Ok end removes; D12 `SpecPlugin::new` takes `std::path::absolute(state)`; D13 `agent_resume_command` holds `StateDir::open(--state)` around the resume (2 s `LOCK_WAIT`), a held directory exits 1; the two INFEASIBLE notes are lines under "Not verified here" in `docs/qualification/2026-10-25-agent-serve.md`. Gate after the round: 292 passed, 0 failed across the five crates; clippy clean; fmt clean; `loom-docs generate --check` current. All six findings: `review_outcome` fixed (the two INFEASIBLE ones by the qualification note).

```findings
[
{"file":"crates/loom-plugin/src/turn.rs","line":948,"category":"property","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"per-turn ids restart at one, so sends of two items carry the same idempotency key and Connectors never applies the second"},
{"file":"crates/loom-agent/src/serve.rs","line":449,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a resume refused before any process removes the held file and the approved send is lost with no recovery"},
{"file":"crates/loom-agent/src/serve.rs","line":302,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a relative --state yields a relative proof path that Connectors refuses, dropping the granted send"},
{"file":"crates/loom-cli/src/main.rs","line":165,"category":"concurrency","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"agent resume takes no state lock, so it and a serving host can both resume the same grant and double-count sends.jsonl"},
{"file":"ess/scenarios/agent-serve","line":1,"category":"mutant","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"conformance exercises admit_send on declared inputs only, so a mis-wired input in effects.rs send passes every scenario"},
{"file":"docs/qualification/2026-10-25-agent-serve.md","line":1,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"no run against real Connectors; acceptance of a Loom grant file as the write's proof is unverified"}
]
```
