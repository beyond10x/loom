---
format: aep.planning-md/3
id: story:authority-sees-the-request
kind: story
status: implemented
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
  path: crates/loom-cli/tests/security_w10_u3.rs
- confidence: cited
  path: crates/loom-commission-testkit/src/fake_authority.rs
- confidence: cited
  path: crates/loom-commission-testkit/src/kits/authority.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/effect_invocation.rs
- confidence: cited
  path: crates/loom-commission-testkit/tests/runtime_effect.rs
- confidence: cited
  path: crates/loom-commission/src/ports/authority.rs
- confidence: cited
  path: crates/loom-commission/src/runtime.rs
- confidence: cited
  path: crates/loom-executor/tests/interruption_recovery.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: cited
  path: crates/loom-intake-slice/src/run.rs
- confidence: cited
  path: crates/loom-mandate/src/lib.rs
- confidence: cited
  path: crates/loom-mandate/tests
- confidence: cited
  path: crates/loom-plugin/src/authority.rs
- confidence: cited
  path: crates/loom-plugin/tests/hooks.rs
- confidence: cited
  path: crates/loom-sdk/examples/software_change.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
- confidence: cited
  path: crates/loom-sdk/tests/sdk_model_ports.rs
- confidence: cited
  path: docs/commission/design/approval-gate.md
- confidence: cited
  path: ess/commission/domains/responsibility.yaml
- confidence: cited
  path: website/docs/reference/sdk-stability.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-20T01:24:51Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-20T03:29:52Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":7,"verification":1}}}
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

## Correction at close (wave 2026-10-20-w1)

- "Artifact revisions" in the Outcome and the first acceptance line are not part of what shipped:
  Commission's `ActionRequest` carries none (`ess/commission/domains/responsibility.yaml:938-952`:
  run, case, `expected_case_revision`, action, arguments). The provider receives that request as
  declared; `ess/` was not changed for it. Whether the case revision moves with every artifact
  revision is inferred, not checked.
- The security review and the adversary found that a request the provider allowed was invoked even
  when the case moved during the decision (pre-existing on the base). The unit closed it: the
  runtime reads the case revision again after an allow and once more right before a resumed request
  is invoked; on a move it sees, the run loop records the request `Stale`, a resume before its take
  is refused `CaseMoved`, and a resume after its take releases the Suspension. A move while the
  effect itself runs is not seen; an effect port that must refuse such a write binds it to
  `expected_case_revision` (`7e5fb700`, `64dc06b1`, `ea03fef6`).
- Files touched beyond the Scope below, call-site updates only: `crates/loom-commission-testkit/src/kits/authority.rs`,
  testkit `authority_port.rs`, `run_outcomes.rs`, `adversary_run_outcomes.rs`, `effect_invocation.rs`,
  `runtime_effect.rs`; `crates/loom-mandate/tests/{mandate_authority,adversary_w10_u2,security_w10_u2}.rs`;
  `crates/loom-plugin/tests/hooks.rs`; `crates/loom-cli/tests/security_w10_u3.rs`;
  `crates/loom-executor/tests/interruption_recovery.rs`; `docs/commission/design/approval-gate.md`;
  `ess/commission/domains/responsibility.yaml` (comments only).
