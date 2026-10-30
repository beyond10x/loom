---
format: aep.planning-md/3
id: story:session-executor-claude-loopback-endpoint
kind: story
status: implemented
title: A Claude Code session executor is pointed at a keyless loopback model endpoint
relations:
- decomposes: epic:credential-free-agent-pod
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: crates/loom-session-executor/src/lib.rs
- confidence: cited
  path: crates/loom-session-executor/tests/claude_endpoint.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-30T16:39:59Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-30T17:37:26Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
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
  metaharness receives names `model_endpoint` = the `base_url` less exactly one trailing `/v1` path
  segment (the gateway root: Claude Code posts to `{root}/v1/messages`), `credentials` none, and the
  executor's model or else the endpoint's, a blank one being none (no `--model`). The child
  environment metaharness builds carries `ANTHROPIC_BASE_URL` = that root and the
  nonessential-traffic switch, and no operator credential. A `base_url` whose path does not end in
  one `/v1` segment, or that carries a query or fragment, suspends the step `ExternalAvailability`
  before any copy. Every refusal names the endpoint by its scheme and host only.
- `a_claude_endpoint_that_is_not_loopback_or_names_a_key_is_refused`: `https://api.example.test/v1`,
  a token file, a token pointer: the step suspends `ExternalAvailability` before any copy.
- `a_claude_endpoint_on_another_wire_is_refused`: `openai-responses` suspends the same way.
- Unchanged: a Claude executor with no endpoint; every b10x-harness endpoint test.

Amended in correction round 1 of wave 2026-10-30-w3 (review-result:adv-w20261030w3-b-pass-1, finding
lib.rs:1155): the first version said `model_endpoint` = the `base_url`, which made the child post to
`/v1/v1/messages`.

## ESS first

None: the executor's launch configuration is host configuration `ess/` does not declare, as for
`story:session-executor-keyless-loopback`.

## Scope

Confirmed by the wave 2026-10-30-w3 diff (`git diff a5f30124 impl/session-executor-claude-loopback-endpoint`, head 41314952):

- `crates/loom-session-executor/src/lib.rs`: the `Endpoint` doc, `fn spec` (`Kind::Claude` arm), `check_claude_endpoint`, `claude_root`, `endpoint_origin`, and the call in `fn session`; `keyless_loopback` reused, not changed
- tests: `claude_endpoint.rs` (new), `adversary_w20261030w3_b.rs` (new), `security_w20261030w3_b.rs` (new), `adversary_w20261030w2_u1.rs` (one Codex-only test renamed, one Claude refusal test added, assertions name this story)
- `AGENTS.md` § Session executor; CHANGELOG.md by the coordinator
- Shared `src/lib.rs` with `story:session-executor-stable-copy-path`; the merges were clean apart from AGENTS.md, which was resolved by hand
