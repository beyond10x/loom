---
format: aep.planning-md/3
id: story:hosted-governor
kind: story
status: implemented
title: Host validated protocols and fallible case storage through CanonGovernor
relations:
- decomposes: epic:governor
- serves: vision:O1
scope:
- confidence: cited
  path: .github/workflows/check.yml
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-commission-conformance
- confidence: cited
  path: crates/loom-commission-docs
- confidence: cited
  path: crates/loom-commission-testkit/tests
- confidence: cited
  path: crates/loom-commission/tests
- confidence: cited
  path: crates/loom-executor/tests
- confidence: cited
  path: crates/loom-governor
- confidence: cited
  path: ess
- confidence: cited
  path: ess/commission
- confidence: cited
  path: generated
- confidence: cited
  path: website
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T23:13:41Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-05T23:13:41Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-05T23:28:00Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":3}}}
---
## Outcome

A host uses CanonGovernor for a reviewed product protocol, a fallible storage adapter, and trusted freshness time. This change supplies no database backend or durable-case recovery implementation. Canon semantics, frontier projection and completion remain in Loom; the host retains case persistence, protocol admission, evidence authentication and authority policy. Commission revalidates and invokes effects. No model value selects a protocol, time, evidence producer or authority.

## API evidence

At base 75eeb40, protocol_ir only reads canon_engineering::registry, decide supplies Supplied::default(), and CaseStore methods have no error channel. Returning false on persistence failure makes open retry forever. Commission already defines GovernorUnavailable and attributed evidence ports; use those contracts.

## Scope

crates/loom-governor, ess/commission/domains/responsibility.yaml (existing port semantics), generated docs, README/AGENTS and capability status. No scheduler, UI, provider loop or protocol definition changes.

## ESS first

The existing Commission domain owns GovernorError, case, frontier and evidence; Canon owns protocol and evaluation time. No new domain noun. First commit clarifies unavailable semantics beside GovernorError in ESS. Named regression hosted_protocol_projects_and_expires_evidence initially fails because with_protocol and with_evaluation_time do not exist. Tests also cover invalid protocol, multiple capabilities, duplicate registration, storage failures and existing infallible store compatibility.

## Acceptance and review

- hosted_protocol_projects_and_expires_evidence: custom protocol produces Canon frontier, time expires evidence, host time failure refuses.
- storage_failures_are_unavailable_without_retry: failed insert terminates once; failed reads, updates and observation writes are explicit.
- registry_cannot_be_replaced_or_weakened: duplicate/built-in names and unsupported multiple capabilities refused before opening.
- Existing governor tests and Commission conformance stay green.
- task check and task plan; targeted tests prove only foundation contract, not control-plane end-to-end delivery.

## Authorization

Operator explicitly requested implementing supported Loom integration and genuine foundation gaps on 2026-10-06. This bounded foundation correction is part of control-plane story:runtime-boundary. Independent integration review remains with the coordinating agent.

## Dependency correction

The installed newest ESS is 0.53.0. The original strict gates refused the 0.52.0 requirements. Updated specification/CI/tooling pins and generated Rust/docs through the existing generators; the new generated storage ports come from ESS, not hand-written models. This is included in the story's verification scope.

## Current evidence

First spec-only commit: 7a92885. Red: `cargo test -p b10x-loom-governor --test hosted_protocol --locked` failed because FallibleCaseStore was absent. After implementation, four hosted-contract tests passed, including canonical time expiry and failed-write recovery. Both strict ESS gates pass on 0.53.0. Full repository and site gates are still required before foundation completion; product end-to-end verification belongs to control-plane.

## Verification completed

- `task check`: exit 0, including strict ESS gates, generated drift, no-hand-model, Commission conformance, dependency guard, workspace Clippy/tests and documentation drift.
- `task website`: exit 0; generated static site validated. No deployment claimed.
- ESS 0.53 also required two test context signatures and the Run testkit's `expect_event_values` interpreter to follow current generated contracts. `event_value_checks_reject_the_wrong_captured_instance` proves incorrect event identity is rejected; all nine Run scenarios pass.
- Existing ignored generated revalidation/recovery tests remain outside this claim (`ESS-SYNTH-003`, `ESS-SYNTH-004`). Commission runtime tests and control-plane integration must establish their own fresh authority/revision checks.
- No model/network calls were made by these tests. No product end-to-end outcome, database persistence implementation, or live documentation publication is claimed here.
