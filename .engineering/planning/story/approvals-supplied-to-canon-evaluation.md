---
format: aep.planning-md/3
id: story:approvals-supplied-to-canon-evaluation
kind: story
status: draft
title: The governor hands Canon the approvals Loom recorded, so Canon's own evaluation sees them
summary: AuthorityRecords port; canon-authority/1 in today's shape at the current case revision; the slice supplies its own, the tree in tree-waits-open-decision-requests; waits on no Canon pin
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approval-decisions-resume-held-runs
- depends_on: story:grounds-resolve-against-held-cases
revision: 3
---
## Outcome

Loom hands Canon the approvals it recorded, as a `canon-authority/1` document in today's shape
(`capability`, `decision`), in every evaluation of a case, so an approved action is `admissible`
in Canon's own decision and in a replay of it.

- `CanonGovernor::with_authority_records` takes the host's `AuthorityRecords` port
  (`story:grounds-resolve-against-held-cases`: case, capability, decision, principal, case
  revision). The private `decide_case` (`crates/loom-governor/src/lib.rs:1798-1815`), through
  which every evaluation of the governor goes, passes Canon the records of the case whose case
  revision is the held case's current revision, one entry per capability. Canon's
  `canon-authority/1` of the pinned release has no `case_revision` member
  (`~/beyond10x/canon/website/docs/reference/documents.md` at tag `0.8.0`, lines 109-131), so Loom
  binds an approval to its revision by choosing which records it hands over; a record of an earlier
  revision is not handed over.
- Two records of one capability at one revision with different decisions are refused as a store
  failure naming the case and the capability, and the case is not decided; Canon refuses a
  capability decided twice (`documents.md` at `0.8.0`, line 111).
- `evaluate` (`loom.evaluation`, `b10x-loom evaluate`) takes an optional `canon-authority/1`
  document beside the evidence and hands it to Canon as given, so a replay sees what the run saw.
- The slice's grant store (`story:approval-decisions-resume-held-runs`) implements
  `AuthorityRecords`, and the slice builds its governor and its `HeldCaseGrounds` with it when the
  decision flags are given. The tree's `CaseApproval` implements the port in
  `story:tree-waits-open-decision-requests`, which depends on this story. Without the port the
  governor supplies no authority decision, as today, and `HeldCaseGrounds` refuses a
  `canon_authority` citation (`story:grounds-resolve-against-held-cases`).

The port and the governor's supply go in their own module of `crates/loom-governor`, the slice's
implementation in the grant module of `crates/loom-intake-slice`.

Commission's revalidation then reads an approved action as `Admitted`, and the authority provider
is asked only about actions Canon still reports approval-required. Loom's own check of an
approval's grounds before it allows stays until Canon judges grounds on authority entries
(`story:authority-records-bound-and-grounded-in-canon`).

## Why

Session `sdc`, 2026-11-21 (answer h): an approval that Canon does not see cannot make an action
admissible in Canon's own evaluation or in a replay; SDC's driver hands Canon `authority.json`
today, and the Loom version must not lose that. Today the governor "supplies no authority
decision. It never invents authority, so an action that requires a capability is at best
`approval-required`" (`crates/loom-governor/src/lib.rs:53-56`; `AGENTS.md:457`). This part
needs no Canon release: the entry shape exists in Canon `0.8.0`. The `principal` and
`case_revision` members wait on Canon's `story:authority-revision-binding` and are
`story:authority-records-bound-and-grounded-in-canon`.

## Waits on

- `story:approval-decisions-resume-held-runs`: the slice's grant store and the decision flags.
- `story:grounds-resolve-against-held-cases`: the `AuthorityRecords` port and `HeldCaseGrounds`,
  and both change `crates/loom-governor/src/lib.rs`.

`story:question-and-outcome-decisions-hold-runs` also changes `crates/loom-intake-slice` and
`crates/loom-cli`, with no edge between the two: each story's code is in its own module, and the
wave planner keeps two stories that share a scope path out of one wave.

Not held by `upstream-blocker:canon-decision-side-unreleased`, and not by the Canon 0.9.0 pin of
`epic:protocol-tree-on-bindings`: it waits on no tree story.

## ESS first

`ess/domains/evaluation.yaml`: the evaluation request gains optional `authority` (a
`canon-authority/1` document), and the comment that the evaluation supplies no authority decision
is replaced. `ess/domains/governor.yaml`: `HeldAuthorityRecord` gains the refusal
`ConflictingAuthorityRecords`. On that commit `task drift` fails, and
`crates/loom-governor/tests/authority_supplied.rs` does not compile.

## Acceptance

With a fixture protocol whose action `merge` requires capability `change.merge`:

- With no record, the frontier reports `merge` approval-required, as today.
- With a fixture `AuthorityRecords` holding `change.merge` granted at the current case revision,
  the frontier reports `merge` admissible and the `canon-decision/1` document behind it shows the
  capability granted; with it denied, `merge` is blocked.
- After the case revision moves, the same record is not handed over and `merge` is
  approval-required again.
- Two records of `change.merge` at one revision, one granted and one denied, make the case
  undecided with `ConflictingAuthorityRecords` naming the capability.
- `b10x-loom evaluate` with `--authority FILE` gives the same decision bytes as the held case's
  frontier for the same snapshot, evidence and records.
- The slice run of `story:approval-decisions-resume-held-runs`: after `decisions decide … A`, the
  next evaluation shows `repository.merge` admissible, and `resume` performs it once.
- A slice approval whose decider ground cites `repository.merge@<rev>` in `canon_authority`
  resolves through the slice's port once the grant is recorded.
- Without `with_authority_records`, every existing governor test passes unchanged.
- `task check` exits 0.

## Out of scope

`principal`, `case_revision` and grounds on the entry
(`story:authority-records-bound-and-grounded-in-canon`). The tree's approvals
(`story:tree-waits-open-decision-requests`). Mandate approvals. An authority store of
the governor's own.
