---
format: aep.planning-md/3
id: story:canon-governor
kind: story
status: implemented
title: Govern a case with Canon behind Commission's governor and evidence ports
refs:
- provider: governor
  reference: story:canon-governor
relations:
- decomposes: epic:governor
- serves: vision:governed-autonomy
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-05T00:06:01Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-05T00:06:01Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-10-05T00:06:01Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
> Re-filed from `beyond10x/governor` `story:canon-governor` at `81fcc1e` (status there: `implemented`) under Atlas ADR 0090
> (loom `story:import-governor`).

## Outcome

A Canon-backed governor behind Commission's ports, so the slice drives the case the way
Commission's runtime will:

- Crate `governor` (`b10x-governor`) implements `commission::ports::governor::Governor` (`current_revision`,
  `frontier`, `completion`) and `commission::ports::evidence::EvidencePort` over Canon.
- `open(protocol, revisions)` opens a `canon-case/1` on an ELS protocol with the caller's artifact
  revisions; it refuses a protocol the ELS registry does not hold and a revision map that misses a
  declared artifact.
- `update_revision(case, artifact, revision)` records a new artifact revision (a new
  `implementation` commit) and bumps the case revision. It is the governor's only write entry
  besides `EvidencePort`, and the executor calls it after `repository.edit`.
- `frontier` evaluates the case with the evidence submitted so far, through `ir::compile` and
  `eval::evaluate_with`. Each action's status (`admissible`, `approval-required`, `blocked`), its
  reasons and the capability it needs become a `FrontierAction` of Commission's `FrontierData`,
  issued for the case's current revision.
- `completion` reports an outcome Canon holds legitimate, and only that.

## Acceptance

`a_governed_case_issues_the_frontier_canon_decides`:

- **Fresh case.** A `software.change/1` case opened with revisions for every declared artifact
  holds them as given; a revision map missing one is refused, naming it. Its first frontier makes
  `repository.edit` and `tests.run` admissible, and `repository.merge` blocked with the reason
  `implementation.verified` (its precondition claim).
- **Revision update.** `update_revision(case, implementation, <new revision>)` raises
  `current_revision` by one, and the next frontier is issued for the new revision.
- **Completion.** `completion` reports no outcome while `accepted`'s claims are unknown, and
  `accepted` once evidence makes them true (fed through `EvidencePort`); never an outcome Canon
  holds blocked.
- **ELS fixture states.** For every state of the ELS fixture
  `fixtures/software-change/chg-1842.fixture.yaml` that adds evidence, fed through
  `EvidencePort`, the frontier lists each action with the status Canon gives it, the capability
  `repository.merge` needs, and the case revision. The `merge-approved` state adds only an
  authority grant, which the governor never supplies; it is out of this acceptance.

## ESS first

None: the governor declares no noun of its own (`AGENTS.md` § ESS). It speaks Commission's generated types (`FrontierData`,
`CompletionDetermination`, `Evidence`). The first commit is the named test, red because the
governor does not exist.

## Reuses

- `canon/crates/canon/src/ir/mod.rs:132`
- `canon/crates/canon/src/eval/mod.rs:319`
- `commission/crates/commission/src/ports/governor.rs:14`
- `commission/crates/commission/src/ports/evidence.rs` (`EvidencePort`, `submit_evidence`)
- `els/crates/els/src/registry.rs` (`get`)

## Design

- **Stateless evaluation.** Each `frontier` and `completion` call evaluates afresh, with Canon's
  determinism: the same protocol, snapshot, evidence and authority decisions give the same decision
  bytes. Case state (snapshot, revisions, evidence) lives in a `CaseStore` the governor is given:
  in memory for this story, durable later.
- **Inputs.** Protocols come from the ELS registry. Evidence arrives through `EvidencePort`, and the
  pull-based `EvidenceProvider` of ADR 0079 is later. Authority decisions are the ones Commission's
  authority provider returns (`canon-authority/1`); the governor never invents one.
- **What the governor does not do.** It never invokes an effect (ADR 0082). It sees action names and
  capabilities only; the binding to runtime tools stays in Commission (ADR 0083). It adds no
  sequencing beyond the protocol's own (ADR 0084).
- **Later.** Child cases compose at run time (ADR 0081): a child's outcome is evidence for its
  parent. Generated protocols are admitted only after `protocol.adopt/1` (ADR 0086). Both are later
  stories.
