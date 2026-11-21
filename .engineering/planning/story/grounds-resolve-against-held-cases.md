---
format: aep.planning-md/3
id: story:grounds-resolve-against-held-cases
kind: story
status: draft
title: Grounds resolve against the governor-held case, its records and the decision store
summary: Revision still current, evidence still present, record ids in session sdc's forms
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:decider-port-and-grounding-check
- depends_on: story:decision-request-file-store
revision: 2
---
## Outcome

`HeldCaseGrounds` in `b10x-loom-governor` implements the `CitationResolver` port of
`b10x-loom-decision` for the citations a pending recommendation cites in a governor-held case. It
applies Canon's two rules for a ground to a recommendation that is not yet a Canon record: the
cited revision is still the current one, and the cited evidence record is still present.

- `artifact_revision`: resolves when the revision equals the current revision of the artifact in
  the held case, or in the held case the citation's `case` names; the artifact `case` means the
  case revision (`~/beyond10x/sdc/tools/decision-grounds.sh:47-52`).
- `record_id` per store:
  - `canon_evidence`: the evidence id is held by the case;
  - `canon_decisions`: `<outcome>/<decision>@<case_revision>` matches an explicit decision the
    governor holds for the case, recorded with `CanonGovernor::decide` at that case revision
    (`AGENTS.md:456-461`);
  - `canon_authority`: `<capability>@<case_revision>` matches a record the host's
    `AuthorityRecords` port reports for the case (capability, decision, principal, case revision);
    Loom keeps authority records outside Canon today (the slice's grant store,
    `crates/loom-intake-slice/src/grant.rs:116`). This story declares the port; the port is
    optional: `HeldCaseGrounds::new` takes none and `with_authority_records` adds one. Without
    one, a `canon_authority` citation is refused "no authority records for case <id>", so a ground
    nobody can resolve never counts. The slice offers its records from
    `story:approvals-supplied-to-canon-evaluation` on, the tree from
    `story:tree-waits-open-decision-requests` on;
  - `sdc_decisions`: the id names a Decided request in the decision store;
  - `aep_artifact`: `kind:name` names the file `<kind>/<name>.md` under the AEP root the host
    names (`decision-grounds.sh:44`).
- `path_line` is delegated to `PathLineResolver` with the case's directory and the host's root.

A bare `capability` or `decision@outcome` id, the forms `decision-grounds.sh:35-37` reads today,
is refused naming the form: the record ids are those of session `sdc`, 2026-11-21, point 7.

## Why

The grounding check of `story:decider-port-and-grounding-check` needs these citations resolved,
and only `b10x-loom-governor` evaluates protocols with Canon and holds cases (`AGENTS.md:436-437`).
Canon judges grounds only on a decided entry (`story:decision-grounds`, session `canon`,
2026-11-21); before the entry exists Loom checks the same two rules itself (session `sdc`,
2026-11-21). Canon 0.8.0 already carries artifact revisions and the case revision an explicit
decision binds to (`~/beyond10x/canon/website/docs/reference/documents.md:23-35`, `:141-148`), so
nothing here waits on a Canon release.

## Waits on

`story:decider-port-and-grounding-check`: the port and the refusal types.
`story:decision-request-file-store`: the store `sdc_decisions` reads, and both change
`crates/loom-governor/src/lib.rs`.

## ESS first

`ess/domains/governor.yaml` gains `GroundRoots` (repository root, AEP root) and
`HeldAuthorityRecord` (case, capability, decision, principal, case revision). On that commit
`task drift` fails, and `crates/loom-governor/tests/held_case_grounds.rs` does not compile.

## Acceptance

With a fixture protocol held by `CanonGovernor` over a `FileCaseStore`:

- `roadmap@<current>` resolves; after `update_revision` moves `roadmap`, the same citation is
  refused "`roadmap@<old>` is not current (`<new>`)".
- `case@<case revision>` resolves; a citation whose `case` names a second case held by the same
  governor resolves against that case, looked up by id (no parent-child link of
  `story:child-case-runtime` is read).
- A held evidence id resolves; an id the case does not hold is refused naming the store and id.
- `close/taken@<rev>` resolves after `CanonGovernor::decide(case, "close", "taken", p)` at that
  revision and is refused at any other revision.
- `owner.approve@<rev>` resolves against a fixture `AuthorityRecords` holding that record, and the
  bare `owner.approve` is refused naming the form.
- `sdc_decisions` resolves an id of a Decided request in a `FileDecisionStore` and refuses an Open
  one.
- An `aep_artifact` id under a fixture root resolves, and one
  outside it is refused.
- With no `AuthorityRecords` port, `owner.approve@<rev>` is refused "no authority records for
  case <id>".
- The check of `story:decider-port-and-grounding-check` over this resolver gives the four rows of
  the hand-made requests of `~/beyond10x/sdc/docs/decisions.md:148-153`, rebuilt as a fixture
  case: `roadmap-valid` verified with "5 grounds, all resolve"; `roadmap-dangling` refused with
  its three reasons (lines outside the file, no evidence record, `roadmap@<old>` not current);
  `roadmap-valid` with a prior-decisions store reusing the prior whose grounds resolve and naming
  the one whose `vision` ground is not current; a ground with a wrong digest refused "changed:
  lines 20-27 of work/aep/vision/product.md differ from the cited digest".
- The resolver evaluates no protocol and reads no clock (a test counts governor evaluations).
- `task check` exits 0.

## Out of scope

Writing grounds onto a Canon record, and reading `stale_grounds`
(`story:decided-records-carry-grounds-to-canon`). A record store of Loom's own for authority
records.
