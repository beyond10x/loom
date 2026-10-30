---
format: aep.planning-md/3
id: story:session-executor-claude-loopback-endpoint
kind: story
status: active
title: A Claude Code session executor is pointed at a keyless loopback model endpoint
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-session-executor/tests/claude_endpoint.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 5}
---
## Outcome

A Claude Code session executor given an `Endpoint` points the session at it: when the endpoint
names no key (`token_file` empty, no `token_pointer`), its `base_url` host is loopback (the rule
`keyless_loopback` already applies to b10x-harness) and its `wire` is `anthropic-messages`, `fn spec`
sets metaharness's `model_endpoint` to the `base_url` with `credentials: none`, and the model to
the executor's model or else the endpoint's. metaharness then gives the child `ANTHROPIC_BASE_URL`,
its placeholder key and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, so the session sends nothing
past the endpoint. A Claude endpoint that names a key, is not loopback, or speaks another wire
suspends the step with `ExternalAvailability` before any copy, naming why. A Claude executor with
no endpoint is unchanged.

## Why

In a credential-free pod (`spec.boundary`) the model is reached only through the boundary's
loopback relay (`http://127.0.0.1:8401/v1`). The session executor sets `model_endpoint` for
b10x-harness only (`crates/loom-session-executor/src/lib.rs:1140-1163` at 0.52.0); a Claude
session ignores its endpoint today, so `--harness claude` cannot run in such a pod or under
`--unshare-net`. metaharness 0.23.0 already serves a declared endpoint for Claude
(`crates/metaharness-claude/src/launch.rs:1538-1557`: base URL plus placeholder key; `credential_free`
sets the nonessential-traffic switch). Source: the trial analysts' report 5, item 3 (K3 in the trial
summary), and the orchestrator's item D283. takt's own flag (`--model-endpoint` for `--harness
claude`, takt `crates/takt-cli/src/main.rs:2629`) is takt's story, not this one.

## Acceptance

`crates/loom-session-executor/tests/claude_endpoint.rs` (scripted sessions, as every executor test):

- `a_claude_session_with_a_keyless_loopback_endpoint_gets_it_as_its_model_endpoint`: the `RunSpec`
  metaharness receives names `model_endpoint` = the `base_url`, `credentials` none and the model;
  the child environment metaharness builds carries `ANTHROPIC_BASE_URL` and the nonessential-traffic
  switch and no operator credential.
- `a_claude_endpoint_that_is_not_loopback_or_names_a_key_is_refused`: `https://api.example.test/v1`,
  a token file, a token pointer: the step suspends `ExternalAvailability` before any copy.
- `a_claude_endpoint_on_another_wire_is_refused`: `openai-responses` suspends the same way.
- Unchanged: a Claude executor with no endpoint; every b10x-harness endpoint test.

## ESS first

None: the executor's launch configuration is host configuration `ess/` does not declare, as for
`story:session-executor-keyless-loopback`.

## Scope

- cited: `crates/loom-session-executor/src/lib.rs`, `fn spec` (`Kind::Claude` arm, about line 1141)
  and `fn check_api_key_secret` (about line 1171) or a sibling check called beside it in
  `fn session` (about line 1243); `keyless_loopback` (about line 2977) is reused, not changed
- cited: `crates/loom-session-executor/tests/claude_endpoint.rs` (new)
- AGENTS.md § Session executor (the endpoint paragraph); CHANGELOG.md is the coordinator's
- Shares `src/lib.rs` with `story:session-executor-stable-copy-path`: this story stays inside the
  functions named above.
