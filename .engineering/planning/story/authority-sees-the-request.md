---
format: aep.planning-md/3
id: story:authority-sees-the-request
kind: story
status: active
title: 'AuthorityProvider::decide receives the action request it decides: action, case revision and artifact revisions'
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-commission-testkit/src/fake_authority.rs
- confidence: cited
  path: crates/loom-commission/src/ports/authority.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-mandate/src/lib.rs
- confidence: cited
  path: crates/loom-plugin/src/authority.rs
- confidence: cited
  path: crates/loom-sdk/examples/software_change.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: crates/loom-sdk/tests/sdk_model_ports.rs
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
---
## Outcome

`AuthorityProvider::decide` receives the action request it is deciding: the action, the case
revision the request is bound to and the artifact revisions it carries, beside the commission and
the capability. Today it receives the commission and the capability only
(`crates/loom-commission/src/ports/authority.rs:21-30`), so a provider cannot grant one request at
one revision and refuse the same capability at the next.

## Why

takt's grants are bound to a revision: a grant given at one case revision does not authorise the
same action after the case moved. Moving takt's grants from Canon's authority input to an
`AuthorityProvider` keeps that binding only if the provider sees the request. The runtime already
holds the request at both call sites (`crates/loom-commission/src/runtime.rs:1098`, `:1391`).

## Acceptance

- `crates/loom-commission-testkit/tests/authority_request.rs::the_provider_sees_the_request_it_decides`:
  a provider records what it was asked; the action, case revision and artifact revisions equal the
  request the runtime revalidated.
- `…::a_grant_bound_to_one_revision_does_not_allow_the_next`: a provider that allows only revision
  N allows the request at N, and the same action at N+1 is answered `ApprovalRequired` and nothing
  is performed.
- `…::a_failing_provider_still_grants_nothing`: `check_authority` keeps refusing on an `Err`
  (`adversary_authority_fail_closed.rs` still passes).
- Every provider in the workspace takes the new argument: `loom-mandate`, `loom-plugin`'s
  `PluginAuthority`, the slice's `GrantAuthority` and run authority, the testkit fakes, the SDK
  example and `crates/loom-sdk/tests/host_ports.rs`; the Mandate and grant tests pass unchanged in
  what they assert.
- `CHANGELOG.md` names the signature change under `Breaking:`; `loom-xtask release-check` refuses
  `Breaking: none.` on it, since `crates/loom-sdk/tests/host_ports.rs` changed.
- `task check` exits 0.

## ESS first

The request is Commission's generated `ActionRequest`, already declared in
`ess/commission/domains/responsibility.yaml`, and the port is a hand-written trait, so `ess/` is
unchanged. The red step is `the_provider_sees_the_request_it_decides`, failing to compile before
the signature changes.

## Not in scope

- Which grants takt keeps and how it stores them: the operator decides that in takt's story.
- The governor supplying authority: it still supplies none.
- Changing what any existing provider grants.

## Scope

Crates: `loom-commission`, `loom-commission-testkit`, `loom-mandate`, `loom-plugin`,
`loom-intake-slice`, `loom-sdk`.

- `crates/loom-commission/src/ports/authority.rs`, `crates/loom-commission/src/runtime.rs` (the two
  `check_authority` calls only)
- `crates/loom-commission-testkit/src/fake_authority.rs` and the testkit tests implementing the trait
- `crates/loom-mandate/src/lib.rs`, `crates/loom-plugin/src/authority.rs`,
  `crates/loom-intake-slice/src/grant.rs`, `crates/loom-intake-slice/src/run.rs` (its
  `AuthorityProvider` impl only)
- `crates/loom-sdk/examples/software_change.rs`, `crates/loom-sdk/tests/host_ports.rs`,
  `crates/loom-sdk/tests/sdk_model_ports.rs`
- `website/docs/reference/sdk-stability.md` (the `AuthorityProvider` row only)
- `CHANGELOG.md`, `AGENTS.md`

Shared files: the stability page with `story:engineering-protocols-090` (pin line) and
`story:agent-executor-host-port` (which depends on this story); `crates/loom-sdk/tests/host_ports.rs`
with `story:governor-exposes-canon-decision` (another test body); `runtime.rs` and
`crates/loom-intake-slice/src/run.rs` with the Commission outcome stories, which depend on this one
(`story:refused-effect-ends-run` first).
