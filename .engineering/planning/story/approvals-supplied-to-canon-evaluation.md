---
format: aep.planning-md/3
id: story:approvals-supplied-to-canon-evaluation
kind: story
status: draft
title: The governor hands Canon the approvals Loom recorded, so Canon's own evaluation sees them
summary: AuthorityRecords port; canon-authority/1 in today's shape at the current case revision; a record whose request's grounds moved is withheld; the slice supplies its own, the tree in tree-waits-open-decision-requests; waits on no Canon pin
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approval-decisions-resume-held-runs
- depends_on: story:grounds-resolve-against-held-cases
revision: 5
---
## Outcome

Loom hands Canon the approvals it recorded, as a `canon-authority/1` document in today's shape
(`capability`, `decision`), in every evaluation of a case, so an approved action is `admissible`
in Canon's own decision and in a replay of it.

- `CanonGovernor::with_authority_records` takes the host's `AuthorityRecords` port
  (`story:grounds-resolve-against-held-cases`: case, capability, decision, principal, case
  revision). The private `decide_case` (`crates/loom-governor/src/lib.rs:2042-2065`), through
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
- The slice's `AuthorityRecords` reports a record only while the grounds of the Decided request
  that produced it (the request whose `produced_record` names it) still resolve through
  `HeldCaseGrounds`, with the run's workspace as the `path_line` root and `canon_authority`
  citations resolved against the unfiltered records, so no record's check waits on another's
  filter. A record with a moved ground is withheld from Canon: Canon reports the action
  approval-required again, Commission asks `DecisionAuthority`, and `DecisionAuthority`
  supersedes the request and routes the stop anew, as it does today.

Once the approvals reach Canon, Commission's revalidation reads an approved action as `Admitted`
and asks the authority provider only about actions Canon still reports approval-required
(`crates/loom-commission/src/runtime.rs:2435-2449`), so the grounds check of
`DecisionAuthority` alone would no longer run for an approved action. Withholding the record at
the supply keeps it running. That is the smaller of the two mechanisms: the other, asking the
provider about an action Canon admits, changes Commission's revalidation for every host. The check
matters only for grounds that move while the case stays (`path_line`, `aep_artifact`,
`sdc_decisions`): a moved `artifact_revision` ground raises the case revision
(`crates/loom-governor/src/lib.rs:1042`), and a record of an earlier case revision is not handed
over at all. It stays until Canon judges grounds on authority entries
(`story:authority-records-bound-and-grounded-in-canon`), and for the grounds Canon cannot see,
after that.

The port and the governor's supply go in their own module of `crates/loom-governor`, the slice's
implementation in the grant module of `crates/loom-intake-slice`
(`crates/loom-intake-slice/src/grant.rs`).

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

- `story:approval-decisions-resume-held-runs`: the slice's grant store, the decision flags, and
  `DecisionAuthority`, whose supersede the withheld record leads to.
- `story:grounds-resolve-against-held-cases`: the `AuthorityRecords` port and `HeldCaseGrounds`,
  and both change `crates/loom-governor/src/lib.rs`.

`story:decision-suspension-reason` also changes `crates/loom-intake-slice/src/grant.rs`; this story
is ordered after it through `story:approval-decisions-resume-held-runs`.
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
- The slice run at `A1` whose request was taken on a strong recommendation with a `path_line`
  ground on lines 1-3 of `notes.md` in the workspace: after those lines are rewritten and not
  committed (the case revision unchanged), the next evaluation shows `repository.merge`
  approval-required with no authority entry handed over, `resume` is refused naming the
  capability as not granted, the request is Superseded naming a new Open request, and nothing is
  performed.
- A slice approval whose decider ground cites `repository.merge@<rev>` in `canon_authority`
  resolves through the slice's port once the grant is recorded.
- Without `with_authority_records`, every existing governor test passes unchanged.
- `task check` exits 0.

## Out of scope

`principal`, `case_revision` and grounds on the entry
(`story:authority-records-bound-and-grounded-in-canon`). The tree's approvals
(`story:tree-waits-open-decision-requests`). Mandate approvals. An authority store of
the governor's own. Asking the authority provider about an action Canon admits.
