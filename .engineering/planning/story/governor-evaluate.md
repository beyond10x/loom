---
format: aep.planning-md/3
id: story:governor-evaluate
kind: story
status: implemented
title: One call evaluates a protocol over a case snapshot and evidence, without Commission types
relations:
- decomposes: epic:downstream-adoption
- serves: vision:O3
- depends_on: story:run-event-stream
- depends_on: story:llm-credentials-bearer
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-cli/src/evaluate.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: cited
  path: crates/loom-cli/tests/evaluate.rs
- confidence: cited
  path: crates/loom-cli/tests/evaluate_adversary.rs
- confidence: cited
  path: crates/loom-governor/Cargo.toml
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/
- confidence: cited
  path: crates/loom-governor/tests/evaluate.rs
- confidence: cited
  path: crates/loom-governor/tests/evaluate_adversary.rs
- confidence: cited
  path: crates/loom-governor/tests/evaluate_adversary_2.rs
- confidence: cited
  path: ess/domains/evaluation.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: ess/system.yaml
- confidence: cited
  path: generated/rust/loom/
- confidence: cited
  path: website/data/ess/loom-evaluation.domain-graph.json
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/concepts/governor-and-intake.md
- confidence: cited
  path: website/docs/reference/cli.md
- confidence: cited
  path: website/docs/reference/ess
revision: 41
transitions:
- {from: "draft", to: "proposed", at: "2026-10-08T08:47:51Z", actor: "human:timo", revision: 15}
- {from: "proposed", to: "active", at: "2026-10-08T08:47:52Z", actor: "human:timo", revision: 16}
- {from: "active", to: "implemented", at: "2026-10-08T09:48:06Z", actor: "human:timo", revision: 41, decided_on: {"recorded":{"test_result":1,"review_outcome":2,"verification":1}}}
---
## Outcome

A caller that keeps its own case record today must embed Commission's types and implement
`CaseStore` or `FallibleCaseStore` to get a decision from Canon (`crates/loom-governor/src/lib.rs`;
the evaluation itself is the private `CanonGovernor::decide`). Add one narrow entry point that takes
a protocol named `<name>@<major>` from the host's protocol catalog, a `canon-case/1` case snapshot,
the `canon-evidence/1` records and an optional trusted evaluation time, and returns Canon's
decision, with no Commission type in its signature: a library function in `loom-governor`, and
`b10x-loom evaluate` reading that input as JSON and writing the decision as JSON, so a supervisor
written in any language can call it. It decides and never acts, like the rest of the governor.
Gap 4 of `epic:downstream-adoption`.

## Domain relations

- Evaluation request -> protocol, many-to-one, the catalog owns the protocol and the request only
  names it - inferable (inferred from `crates/loom-governor/src/lib.rs:83-84`, `CaseState.protocol`
  as `<name>@<major>`, and `lib.rs:344` `with_catalog`; no ess/1 document declares this relation).
- Evaluation request -> evidence record, one-to-many, the request owns its records and none
  outlives it - inferable (inferred from `crates/loom-governor/src/lib.rs:89-90` and `586-591`,
  every applying record of one case goes into one evaluation; no ess/1 document declares this).
- Evaluation request -> evaluation time, zero-or-one, supplied by the host, never by a model -
  inferable (inferred from `crates/loom-governor/src/lib.rs:592-603`, `Supplied { at }`).

## Acceptance

For a bundled protocol, `b10x-loom evaluate` given a case snapshot and evidence returns the
decision `CanonGovernor` reports for the same case after the same evidence (same admissible actions
and completion).

## Checks

- The library function returns the same decision as the subcommand for the same input.
- The public signature of the library function names no `b10x-loom-commission` type.
- An unknown protocol, a malformed snapshot or a malformed evidence record is refused with an error
  that names which input.
- The evaluation makes no clock, network or model call (the time is supplied or absent).

## ESS first

The governor has no domain of its own (AGENTS.md § ESS), so declare a new `loom.evaluation` domain
in `ess/domains/evaluation.yaml`, listed in `ess/system.yaml`: the evaluation request (protocol
name, snapshot, evidence records, optional time), the decision it returns, and its refusals.
Validate with the newest `ess`, `task generate`, and implement against `generated/rust/loom/`. The
red test of the first commit is the new equivalence test in `crates/loom-governor/tests/`.

## Notes

- The protocol comes only from the catalog the host installed (`with_catalog`, `protocols add`),
  never as raw YAML from the caller, so host review of protocols still holds.
- Evidence authentication stays the host's job, as for `submit_evidence`.
- Shared surfaces, hence `depends_on`: `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/main.rs`
  and `website/docs/reference/cli.md` with `story:run-event-stream`; `generated/rust/loom/` with
  `story:llm-credentials-bearer`.
