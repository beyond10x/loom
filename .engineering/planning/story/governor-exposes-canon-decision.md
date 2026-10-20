---
format: aep.planning-md/3
id: story:governor-exposes-canon-decision
kind: story
status: implemented
title: CanonGovernor exposes the canon-decision/1 it already computes
relations:
- decomposes: epic:loom-on-llm-and-metaharness
- serves: vision:governed-autonomy
- depends_on: story:engineering-protocols-090
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/adversary_w20261020_u3.rs
- confidence: cited
  path: crates/loom-governor/tests/decision_document.rs
- confidence: cited
  path: crates/loom-sdk/tests/host_ports.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:24:52Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-20T01:24:52Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-20T03:29:52Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":5,"verification":1}}}
---
## Outcome

A host running a case on `CanonGovernor` reads the `canon-decision/1` document behind the frontier
and the completion the governor issued, through a new method
`CanonGovernor::decision_document(&self, case: &CaseId)`: the whole document Canon wrote,
explanation included, at the case's current revision. The governor already computes and renders it:
`frontier` renders the decision only to hash it into the frontier id
(`crates/loom-governor/src/lib.rs:1141-1143`), `completion` reads it through the private `decided`
(`:1071`, `:1152`), and no public method of `CanonGovernor` returns it (`:701-1173`). The stateless
`evaluate` already returns the document beside its projection (`:109-113`, `:1763`).

## Why

takt keeps a decision record per evaluation (`decisions/NNNN.json`) and scores loops from it. When
its cases move onto Loom's governor, it needs the document the governor decided on, not a second
evaluation beside it, and not a hash of it.

## Acceptance

- `crates/loom-governor/tests/decision_document.rs::the_document_is_the_one_the_frontier_id_hashes`:
  for a case opened on `software-change@1`, `decision_document` returns the document whose rendering
  gives the id of the frontier `frontier` issued at the same revision.
- `…::the_document_equals_what_evaluate_returns_for_the_same_snapshot`: on the same protocol,
  evidence, decisions and time, the document is byte-equal to the one `evaluate` returns.
- `…::a_case_the_governor_cannot_decide_returns_no_document`: a case whose stored inputs do not
  satisfy its protocol is refused `GovernorUnavailable`, as `frontier` refuses it.
- `crates/loom-sdk/tests/host_ports.rs` calls `loom_sdk::CanonGovernor::decision_document` (a
  compile-time check through the SDK path; the page's `loom_sdk::CanonGovernor` row covers the
  method, so the page gains no row).
- `task check` exits 0.

## ESS first

The document type exists: `loom.evaluation` declares the whole `canon-decision/1` document
(`ess/domains/evaluation.yaml:116`), and the method returns that generated type, so `ess/` is
unchanged. The red step is `the_document_is_the_one_the_frontier_id_hashes`, failing before the
method exists.

## Depends on

`story:engineering-protocols-090`: the byte-equality tests run on the protocol set it moves.

## Not in scope

- A second evaluator, or evaluating on a read: the document comes from the evaluation `frontier` and
  `completion` already make.
- Authority in Canon's input: the governor still supplies no authority decision.
- takt's own decision record format.

## Scope

Crates: `loom-governor`, `loom-sdk` (test only).

- `crates/loom-governor/src/lib.rs` (the `CanonGovernor` impl, `:701-1173`)
- `crates/loom-governor/tests/decision_document.rs` (new)
- `crates/loom-sdk/tests/host_ports.rs` (one call)
- `CHANGELOG.md`, `AGENTS.md` (§ Governor, one sentence)

Shared with `story:engineering-protocols-090` (`crates/loom-governor/src/lib.rs:127`, a doc line)
and `story:authority-sees-the-request` (`crates/loom-sdk/tests/host_ports.rs`, a different test
body).

## Correction at close (wave 2026-10-20-w1)

- The method that shipped is `CanonGovernor::decision_document(&self, case: &CaseId, frontier: &FrontierId) -> Result<Option<Value>, GovernorError>`,
  not `decision_document(&self, case)` as the Outcome says. Two adversary passes showed that a
  second evaluation, and then a keep of only the most recent one, return another document than the
  one behind the frontier when the trusted time moves (`b10x-loom` evaluates at the host clock) and
  when Commission's revalidation evaluates after the frontier it reports.
- What holds now: `frontier` keeps the rendered document under the id it issues; a lookup never
  evaluates; it answers `None` for an id the governor did not issue or no longer keeps; at most 16
  documents (`KEPT_DOCUMENTS_PER_CASE`) are kept per case, for the case as it is held now; a case the
  store no longer holds loses them; refusals are `frontier`'s (`b91af11e`, `6feccac2`, `e196fccc`).
- Return type: the generated JSON value `evaluate` returns in `canon` (`ess/domains/evaluation.yaml:116`),
  not the whole `EvaluationDecision`. `ess/` is unchanged.
