---
format: aep.planning-md/3
id: story:mandate-authority
kind: story
status: implemented
title: A host answers the authority port with Mandate
relations:
- decomposes: epic:effect-bindings
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Cargo.toml
- confidence: inferred
  path: crates/loom-commission-testkit/tests/executor_port.rs
- confidence: inferred
  path: crates/loom-executor/tests/crate_names.rs
- confidence: inferred
  path: crates/loom-mandate
- confidence: inferred
  path: ess/domains/authority.yaml
- confidence: inferred
  path: generated/rust/loom
- confidence: inferred
  path: website/docs/reference/crates.md
revision: 15
transitions:
- {from: "draft", to: "proposed", at: "2026-10-14T02:37:56Z", actor: "human:timo", revision: 13}
- {from: "proposed", to: "active", at: "2026-10-14T02:37:56Z", actor: "human:timo", revision: 14}
- {from: "active", to: "implemented", at: "2026-10-14T07:29:11Z", actor: "human:timo", revision: 15, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A library host can answer Commission's authority port with Mandate: `MandateAuthority` in a new
host-side crate `b10x-loom-mandate` (`crates/loom-mandate`) asks `mandate_authz::check` for each
decision and maps its answer to Commission's grant, approval-required or denial; a Mandate failure
yields no grant.

## Why

`epic:effect-bindings` asks that a consequential action execute after a Mandate approval. Loom
links no Mandate crate today; its authority providers are the testkit's fakes, `PluginAuthority`
and the CLI's `GrantAuthority`. Mandate v0.9.0 (2026-10-13) ships `mandate_authz::check`, a library
with approval-required decisions.

## Decision

- The provider lives in a new crate `b10x-loom-mandate`, the one crate that links Mandate
  (`mandate-authz` and its types at tag `v0.9.0`); Commission, the governor and the CLI do not.
- The port stays as it is: `AuthorityProvider::decide` gets the commission and the capability.
  The capability is the Mandate action; the resource is the case (`loom.case`, the case id).
  Widening the port to the action request is a later Commission story.
- The subject comes from the commission's authority context, which the host fills with a Mandate
  verified context; the model never supplies it. A context that does not decode grants nothing.
- The host passes Mandate's ports to a builder; the crate configures no policy of its own and the
  CLI is not wired in this story.

## Acceptance

- A request Mandate allows is granted (`crates/loom-mandate/tests/mandate_authority.rs`
  `a_request_mandate_allows_is_granted`).
- A request Mandate denies is denied, for every denial reason (`a_request_mandate_denies_is_denied`).
- An approval-required decision is Commission's approval-required, and a run through
  `run_with_suspension` holds a Suspension
  (`an_approval_required_decision_suspends_as_the_gate_does`).
- A Mandate port that fails grants nothing (`an_unavailable_mandate_grants_nothing`).
- An authority context that is no Mandate context grants nothing
  (`an_authority_context_that_is_no_mandate_context_grants_nothing`).
- Commission names no Mandate crate (`task commission:deps-guard`,
  `crates/loom-commission-testkit/tests/executor_port.rs`).

## ESS first

The first commit changes only `ess/`: a `loom.authority` domain (`ess/domains/authority.yaml`)
declares the mapping from Mandate's outcome to the authority verdict and the context the host
supplies. On it `task drift` fails; later commits regenerate and make
`a_request_mandate_denies_is_denied` pass without changing `ess/`.

## Not in scope

Wiring Mandate into `b10x-loom run`, `approve` or `resume`; consuming an approval on resume
(story:mandate-approval-on-resume); a policy engine.
