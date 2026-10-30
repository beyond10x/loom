---
format: aep.planning-md/3
id: story:connectors-invoker-keyed-writes
kind: story
status: implemented
title: ConnectorsInvoker sends writes keyed with the admitted request's idempotency key (invoke_v1alpha2_keyed)
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: crates/loom-connectors/Cargo.toml
- confidence: cited
  path: crates/loom-connectors/src/lib.rs
- confidence: inferred
  path: crates/loom-connectors/tests/adversary.rs
- confidence: cited
  path: crates/loom-connectors/tests/connectors_invoker.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-30T17:37:27Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

`ConnectorsInvoker` sends every write it invokes on `POST /v1alpha2/invoke` with an idempotency
key, through `connectors_client::Client::invoke_v1alpha2_keyed`: the key is
`cli::idempotency_key` of the admitted request, the one the local `ConnectorsCli::invoke_write`
already sends, so one admitted request has one key on either transport. Reads stay unkeyed
(`invoke_v1alpha2`), since a key on a read is `unsupported`. A replayed write
(`mutation.replayed: true`) is `Performed` naming the original attempt; `approval_required` and
`idempotency_conflict` are `Err` naming the code, never `Refused` and never retried. The invoker
still invokes once and never resends. `connectors-client` and `connectors-core` move together from
`v0.35.0` to `v0.62.0`, the first release with the keyed client.

A host whose configuration sets no `standing_grants` refuses the key `unsupported` before dispatch
(connectors `contracts/service/compatibility.md` § 2.1, "Refused §4 members"); the invoker reports
that as `Err` that reaches the run, naming the missing standing grant and how to add one: set the
host's `service` configuration `standing_grants` to a grant file (`connectors.standing-grants/1`)
that covers the instance and operation, with a `state` directory. It never resends unkeyed
(orchestrator's amendment, D285). The kind cluster's `connectors-github` instance has standing
grants (D275), so its runs are unaffected.

**Breaking:** a Loom host that invokes writes through `ConnectorsInvoker` against a Connectors
service without `standing_grants` gets that `Err` where the write was dispatched unkeyed.

## Why

Decision D269 (orchestrator, 2026-10-30 09:40Z): Connectors 0.62.0 keeps keyed writes under a
standing grant opt-in, and "default flips only after loom moves to invoke_v1alpha2_keyed (loom
story, next loom wave), so no consumer breaks". D283 puts it in this wave. A key also makes a
host-side retry of one admitted request a replay of its attempt rather than a second write.
Connectors v0.62.0 is released (tag `v0.62.0`, `crates/connectors-client/src/lib.rs:181`).

## Acceptance

`crates/loom-connectors/tests/connectors_invoker.rs` (fake service on a loopback port, extended):

- `a_write_is_sent_with_the_admitted_requests_idempotency_key`: the fake records the request body;
  it carries `idempotency_key` = `cli::idempotency_key(request)`; the outcome is `Performed` naming
  the attempt.
- `a_read_is_sent_without_a_key`.
- `a_replayed_write_is_performed_naming_the_original_attempt`.
- `approval_required_and_idempotency_conflict_are_errors_and_not_retried`: one request each.
- `a_host_without_standing_grants_refusing_the_key_is_an_error_naming_it`: one request, no resend;
  the error text names `standing_grants` and the grant file a host needs.
- Every existing invoker, adversary and read fan-out test passes on `v0.62.0`.

## ESS first

None expected: `ConnectorEndpoint` (`ess/commission/domains/responsibility.yaml:1161`) gains no
member. If the implementor finds the keyed outcome needs a new `ConnectorEffects` variant in the
generated model, that is an ESS change first and the story returns to the coordinator.

## Scope

Confirmed by the wave 2026-10-30-w3 diff (`git diff a5f30124 impl/connectors-invoker-keyed-writes`, head 585a32fd):

- `crates/loom-connectors/Cargo.toml`: connectors-client and connectors-core `v0.35.0` -> `v0.62.0`, `sha2 0.11.0` (already in the lock); `Cargo.lock` adds `connectors-operation-types`
- `crates/loom-connectors/src/lib.rs`: the keyed invoke, `is_key_refusal`, the error codes, `read_otherwise` (number exactness by the host reader's rule), module doc
- `crates/loom-connectors/src/cli/write.rs`, `src/cli.rs`: `idempotency_key` (same signature, digest) and `commission_idempotency_key`
- tests: `connectors_invoker.rs`, `adversary.rs` (`a_replayed_success_is_an_error` runs on a read binding, assertion names this story), `adversary_w20261030w3_d.rs` (new), `security_w20261030w3_d.rs` (new); `crates/loom-agent/tests/serve.rs` (the key shape)
- `AGENTS.md` § Connectors (pin line, keyed paragraph); CHANGELOG.md by the coordinator
- Later items, outside this story: loom-agent's per-resume `Ids` can repeat a run and request id pair within one commission; Connectors could refuse a replay that names its own request as the original, or expose the request id on `Invoked`

## Amendment

Correction round 1 of wave 2026-10-30-w3 (review-result:adv-w20261030w3-d-pass-1 and
sec-w20261030w3-d-pass-1, finding lib.rs:273) changes the key. Where § Outcome and
§ Acceptance name `cli::idempotency_key(request)` as the invoker's key, read
`cli::commission_idempotency_key(commission, request)` instead. It is
`loom-action-request-<request id>-<SHA-256 of commission, case, run and request ids>`, so the
two transports no longer share one key for an admitted request: `ConnectorsCli` sends
`cli::idempotency_key`, the same digest without the commission. The reason: action request ids
are unique only per run or resume, so a key built from the request id alone let two runs share a
key and get a false replay.

The same round also settles three more cases:
- `approval_required` and `idempotency_conflict` are `Err` whatever mutation they carry.
- Only the key refusal's exact shape gets the standing-grants remedy.
- A number the host would read as another `f64` is refused before any call.
