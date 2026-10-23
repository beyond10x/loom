---
format: aep.planning-md/3
id: story:endpoint-api-key-secret
kind: story
status: implemented
title: A b10x-harness session reads its endpoint's API key from the secret store by name
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-23T22:03:33Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-23T22:03:34Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-23T22:58:51Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

A b10x-harness session reads its endpoint's API key from the operator's secret store by name:
`SessionExecutor::api_key_secret` (`<namespace>/<name>`) launches the loop with
`--credentials api-key` and metaharness's `RunSpec.api_key_secret`, so b10x-harness is started
with `--api-key-secret <name>` and no subscription token. The key never enters Loom, metaharness
or the child's environment.

## Why

takt's harness arm (ADR 0101, takt `story:harness-endpoint-is-configured`) points b10x-harness at an
endpoint whose key lives in the secret store. Loom's `Endpoint`
(`crates/loom-session-executor/src/lib.rs:259-271`) names only a subscription token file and
pointer, and `SessionExecutor::spec` (`lib.rs:799-813`) sets only `subscription_token_file` and
`subscription_token_pointer`. metaharness 0.15.0 has `RunSpec.api_key_secret` and
`Credential::Secret`; b10x-harness 0.19.0 has `--api-key-secret`.

## Acceptance

`crates/loom-session-executor/tests/endpoint_secret.rs`:

- `a_named_secret_launches_the_loop_with_api_key_secret`: the launch names `--api-key-secret` and
  the name, names no `--oauth-token-file`, `--api-key-env` or `--api-key-file`, and puts no API
  key variable in the child's environment.
- `a_secret_beside_a_subscription_token_is_refused`: an endpoint naming a token file or pointer
  beside a secret starts no session, and the report names both.
- `a_secret_for_a_vendor_session_is_refused`: a secret on a Claude Code or Codex executor starts no
  session.
- `a_secret_not_namespace_slash_name_is_refused`: `no-slash`, `/name` and `ns/` start no session,
  and the report names the value.
- Unchanged: every existing b10x-harness test, on the subscription token.

## ESS first

None. The executor's launch configuration (`Endpoint`, `credentials`, the new field) is host
configuration `ess/` does not declare, as the subscription token is not. The red run is the new
test file on a commit that adds only it.

## Not in scope

A secret for Claude Code or Codex sessions (metaharness refuses `api_key_secret` for them); a
credential field on `loom.agent/1`; a breaking reshape of `Endpoint` into a credential enum.

## Scope

- `crates/loom-session-executor/src/lib.rs` (`SessionExecutor`, `spec`)
- `crates/loom-session-executor/tests/endpoint_secret.rs`
- AGENTS.md § Session executor, README.md, CHANGELOG.md, `website/data/status.json`
