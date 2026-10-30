---
format: aep.planning-md/3
id: story:session-executor-keyless-loopback
kind: story
status: active
title: A b10x-harness session executor admits a keyless endpoint on a loopback address
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-30T06:45:42Z", actor: "human:timo", revision: 3}
---
## Outcome

A b10x-harness session executor admits an endpoint that names no token file, no token pointer and
no API key secret when the endpoint's `base_url` is a loopback address (`127.0.0.0/8`, `::1` or
`localhost`): the loop is launched with `--credentials none` and no credential flag, and the far
end decides. A keyless endpoint whose URL is not loopback is refused before any copy with the error
it has today. Keyed endpoints (a token file, or a named secret) behave as before.

## Why

In Loom 0.51.0's credential-free pod (`spec.boundary`, D243) the agent reaches the model through
the boundary sidecar on loopback (`LOOM_BOUNDARY_MODEL_URL`, `http://127.0.0.1:8401/v1`), and the
sidecar adds the credential. The agent holds no key, so `check_api_key_secret`
(`crates/loom-session-executor/src/lib.rs:1176-1188` at 0.51.0) refuses the only endpoint such a
pod can name. The consumer is takt wave 47, story U2 (AC2, AC4): a keyless loopback model endpoint
for a takt run in the credential-free pod.

The executor does not see the boundary's URL (it is the pod's environment, not the executor's
input), so the rule is the loopback address, the orchestrator's first choice in D264.

## Acceptance

`crates/loom-session-executor/tests/endpoint_keyless.rs`:

- `a_keyless_loopback_endpoint_launches_the_loop_with_no_credential`: `http://127.0.0.1:8401/v1`,
  `http://[::1]:8401/v1` and `http://localhost:8401/v1` with an empty token file, no pointer and no
  secret run the session; the launch names no `--oauth-token-file`, `--api-key-*` flag.
- `a_keyless_non_loopback_endpoint_is_refused`: `https://api.example.test/v1`,
  `http://127.0.0.1.example.test/v1`, `http://localhost.example.test/v1` and a URL with userinfo
  before a foreign host start no session, and the report carries today's error.
- `a_keyless_loopback_endpoint_with_a_pointer_is_refused`: a token pointer with no token file is
  still refused.
- Unchanged: every existing test on a token file or a named secret.

## ESS first

None. The executor's launch configuration (`Endpoint`, its credential) is host configuration `ess/`
does not declare, as for `story:endpoint-api-key-secret`. The red run is the new test file on a
commit that adds only it.

## Scope

- `crates/loom-session-executor/src/lib.rs` (`check_api_key_secret`, `spec`)
- `crates/loom-session-executor/tests/endpoint_keyless.rs`
- AGENTS.md § Session executor, CHANGELOG.md
