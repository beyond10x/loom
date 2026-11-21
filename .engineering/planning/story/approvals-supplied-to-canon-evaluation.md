---
format: aep.planning-md/3
id: story:approvals-supplied-to-canon-evaluation
kind: story
status: proposed
title: The governor hands Canon the approvals Loom recorded, so Canon's own evaluation sees them
summary: AuthorityRecords port; canon-authority/1 in today's shape at the current case revision; a record whose request's grounds moved is withheld; the slice supplies its own, the tree in tree-waits-open-decision-requests; waits on no Canon pin
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:approval-decisions-resume-held-runs
- depends_on: story:grounds-resolve-against-held-cases
- depends_on: story:question-and-outcome-decisions-hold-runs
scope:
- confidence: inferred
  path: crates/loom-cli/src/evaluate.rs
- confidence: inferred
  path: crates/loom-cli/src/lib.rs
- confidence: inferred
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/tests/approval_decisions.rs
- confidence: inferred
  path: crates/loom-governor/src/authority_supplied.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: inferred
  path: crates/loom-governor/tests/adversary_w20261021w7_completion.rs
- confidence: inferred
  path: crates/loom-governor/tests/adversary_w20261121w2_u1.rs
- confidence: inferred
  path: crates/loom-governor/tests/adversary_w20261121w2_u1_pass2.rs
- confidence: cited
  path: crates/loom-governor/tests/authority_supplied.rs
- confidence: inferred
  path: crates/loom-governor/tests/canon_decision_bytes.rs
- confidence: inferred
  path: crates/loom-governor/tests/case_composition.rs
- confidence: inferred
  path: crates/loom-governor/tests/decision_document.rs
- confidence: inferred
  path: crates/loom-governor/tests/evaluate.rs
- confidence: inferred
  path: crates/loom-governor/tests/evaluate_adversary.rs
- confidence: inferred
  path: crates/loom-governor/tests/evaluate_adversary_2.rs
- confidence: inferred
  path: crates/loom-governor/tests/explicit_decisions.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/decision.rs
- confidence: cited
  path: crates/loom-intake-slice/src/grant.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/run.rs
- confidence: inferred
  path: crates/loom-intake-slice/tests/adversary_w9_u1_inputs.rs
- confidence: inferred
  path: crates/loom-intake-slice/tests/decision_authority_rule.rs
- confidence: cited
  path: ess/domains/evaluation.yaml
- confidence: cited
  path: ess/domains/governor.yaml
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T04:44:11Z", actor: "agent:sdc", revision: 11, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

Loom hands Canon the approvals it recorded, as a `canon-authority/1` document in today's shape
(`capability`, `decision`), in every evaluation of a case, so an approved action is `admissible`
in Canon's own decision and in a replay of it.

- `CanonGovernor::with_authority_records` takes the host's `AuthorityRecords` port. That port and
  its record, `HeldAuthorityRecord` (case, capability, decision, principal, case revision), are
  declared by `story:grounds-resolve-against-held-cases`, where `HeldCaseGrounds` already reads
  them. This story declares no second port: it hands the same port to the governor as well.
- The private `decide_case` (`crates/loom-governor/src/lib.rs:2058-2066`) is the function through
  which every evaluation of the governor goes. It gains the authority entries as an argument, and
  each caller passes them: `decide` (:1081), `decided_at` (:1342) and `applies` (:2055) pass the
  held case's entries, and `evaluate` (:2323-2353) passes the request's.
- The entries are the records of the case whose case revision is the held case's current
  revision, one entry per capability.
- Canon's `canon-authority/1` of the pinned release has no `case_revision` member
  (`~/beyond10x/canon/website/docs/reference/documents.md` at tag `0.9.0`, the version
  `Cargo.lock` pins, lines 110-132). So Loom binds an approval to its revision by choosing which
  records it hands over, and a record of an earlier revision is not handed over.
- Canon refuses a capability decided twice, whatever the decision (`documents.md` at `0.9.0`,
  line 112). Two records of one capability at one revision are therefore handled before Canon:
  - The same decision (different principals, say): one entry is handed over. Today's entry
    carries no principal, so the two records give the same entry.
  - Different decisions: the supply is refused `ConflictingAuthorityRecords`, naming the case and
    the capability, and the case is not decided.
- How a caller reads that refusal. `frontier` returns Commission's `GovernorError`, which carries
  no payload (`generated/rust/commission/src/responsibility.rs:510-515`), and `decide_case`'s
  failures already become `GovernorUnavailable` (`crates/loom-governor/src/lib.rs:1342-1349`).
  - The governor gains a public `CanonGovernor::supplied_authority(case)`. It answers the entries
    it would hand Canon, or the declared refusal `AuthoritySupplyRefusal::ConflictingAuthorityRecords
    { case, capability }`.
  - `frontier` and `decision_document` give `GovernorUnavailable` on that refusal.
  - A caller that gets `GovernorUnavailable` calls `supplied_authority` to read the case and the
    capability.
  - This is smaller than giving Commission's `GovernorError` a payload, which every host matches.
- `evaluate` (`loom.evaluation`, `b10x-loom evaluate`) takes the authority as an optional member
  `authority` of its one request object, as it reads every input today: `MEMBERS` at
  `crates/loom-cli/src/evaluate.rs:15`, and the request described at
  `crates/loom-cli/src/lib.rs:41-46`. There is no `--authority` flag.
  - It hands the document to Canon as given, so a replay sees what the run saw.
  - `EvaluationInput` (`ess/domains/evaluation.yaml:137-139`) gains the variant `Authority`. A
    member that is not a readable `canon-authority/1` document, or that Canon refuses (a capability
    decided twice), is refused naming input `Authority`.
- The slice's grant store implements `AuthorityRecords`. It reads the records that
  `GrantRecordWriter` writes (`story:approval-decisions-resume-held-runs`): record id
  `<capability>@r<N>` at the request's decided case revision, with the request's principal. It
  does not read today's `intake.routing.Grant`, which keeps one grant per commission
  (`ess/intake/domains/routing.yaml:455-456`, the entity at :466). The slice builds its governor
  and its `HeldCaseGrounds` with it when the decision flags are given. The tree's `CaseApproval` implements the port in
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

Files:

- The governor's supply (`with_authority_records`, `supplied_authority`, the `decide_case`
  argument) goes in its own module of `crates/loom-governor`, with a `mod` line in `lib.rs`. The
  port stays in `story:grounds-resolve-against-held-cases`'s module.
- The slice's implementation goes in `crates/loom-intake-slice/src/grant.rs`.
- `evaluate`: `crates/loom-cli/src/evaluate.rs` and its help in `crates/loom-cli/src/lib.rs`.
- Mechanical edits: adding `authority` to the generated `EvaluationRequest`
  (`generated/rust/loom/src/evaluation.rs:120`) breaks every struct literal of it. Each gains the
  member absent (`authority: None`). These are ten governor test files
  (`adversary_w20261021w7_completion.rs`, `adversary_w20261121w2_u1.rs`,
  `adversary_w20261121w2_u1_pass2.rs`, `canon_decision_bytes.rs`, `case_composition.rs`,
  `decision_document.rs`, `evaluate.rs`, `evaluate_adversary.rs`, `evaluate_adversary_2.rs`,
  `explicit_decisions.rs`, all under `crates/loom-governor/tests/`) and
  `crates/loom-intake-slice/tests/adversary_w9_u1_inputs.rs`. No assertion changes.
- Tests: `crates/loom-governor/tests/authority_supplied.rs` (new), and two tests added to
  `crates/loom-cli/tests/approval_decisions.rs`, one added to
  `crates/loom-intake-slice/tests/decision_authority_rule.rs`. Both files are created by
  `story:approval-decisions-resume-held-runs`.

## Why

Session `sdc`, 2026-11-21 (answer h): an approval that Canon does not see cannot make an action
admissible in Canon's own evaluation or in a replay; SDC's driver hands Canon `authority.json`
today, and the Loom version must not lose that. Today the governor "supplies no authority
decision. It never invents authority, so an action that requires a capability is at best
`approval-required`" (`crates/loom-governor/src/lib.rs:54-56`; `AGENTS.md:461`). This part
needs no new Canon release: the entry shape exists in Canon `0.9.0`, the pinned version. The
`principal` and `case_revision` members of an entry wait on Canon's
`story:authority-revision-binding`, and `story:authority-records-bound-and-grounded-in-canon`
carries them.

## Waits on

- `story:approval-decisions-resume-held-runs`: the slice's grant store, the decision flags, and
  `DecisionAuthority`, whose supersede the withheld record leads to.
- `story:grounds-resolve-against-held-cases`: the `AuthorityRecords` port, `HeldAuthorityRecord`
  and `HeldCaseGrounds`, which this story uses and does not declare again. Both change
  `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`.
- `story:question-and-outcome-decisions-hold-runs`: both change
  `crates/loom-intake-slice/src/run.rs` (the slice's governor build) and
  `crates/loom-cli/src/main.rs`, so a direct edge orders this story after it.

`story:decision-suspension-reason` also changes `crates/loom-intake-slice/src/grant.rs`; this story
is ordered after it through `story:approval-decisions-resume-held-runs`.

Not held by `upstream-blocker:canon-decision-side-unreleased`, and not by the Canon 0.9.0 pin of
`epic:protocol-tree-on-bindings`: it waits on no tree story.

## ESS first

`ess/domains/evaluation.yaml`:

- `EvaluationRequest` gains optional `authority` (a `canon-authority/1` document);
- `EvaluationInput` gains the variant `Authority`;
- the comment that the evaluation supplies no authority decision is replaced.

`ess/domains/governor.yaml` gains `AuthoritySupplyRefusal`, a union with one variant,
`ConflictingAuthorityRecords` (case, capability). On that commit `task drift` fails, and
`crates/loom-governor/tests/authority_supplied.rs` does not compile. The struct literals listed
under Files fail to compile until they gain `authority: None`.

## Acceptance

In `crates/loom-governor/tests/authority_supplied.rs`, with a fixture protocol whose action
`merge` requires capability `change.merge`:

- With no record, the frontier reports `merge` approval-required, as today.
- With a fixture `AuthorityRecords` holding `change.merge` granted at the current case revision,
  the frontier reports `merge` admissible.
- With that record, the `canon-decision/1` document behind the frontier shows the capability
  granted.
- With the record denied, `merge` is blocked.
- After the case revision moves, the same record is not handed over and `merge` is
  approval-required again.
- Two records of `change.merge` at one revision, one granted and one denied: the frontier answers
  `GovernorUnavailable`.
- With those two records, `supplied_authority(case)` answers `ConflictingAuthorityRecords` naming
  the case and `change.merge`.
- Two records of `change.merge` at one revision, both granted, with principals `timo` and `ana`:
  `supplied_authority(case)` answers one entry, and the frontier reports `merge` admissible.
- `b10x-loom evaluate` with the request member `authority` holding that entry gives the same
  decision bytes as the held case's frontier for the same snapshot, evidence and records.
- `b10x-loom evaluate` with an `authority` member that lists `change.merge` twice is refused naming
  input `Authority`.

In `crates/loom-cli/tests/approval_decisions.rs`, with the fixture of
`story:approval-decisions-resume-held-runs`. At this story, `decisions decide` and `take` with
`--state` record through `loom_sdk::decisions` and have no resume step;
`story:decisions-resume-the-held-run` adds that step and edits these two tests. They are:

- `authority_supplied_after_decide`: after `decisions decide DR-… A --as timo --state DIR`, the
  next evaluation of the case shows `repository.merge` admissible.
- After that, `b10x-loom resume --commission <commission> --state DIR` performs the merge once
  (the stub merge program records one call).
- `authority_withheld_after_moved_ground`: at `A1`, a request has a strong recommendation with a
  `path_line` ground on lines 1-3 of `notes.md` in the workspace, and is taken with
  `decisions take DR-… --as timo --state DIR`. Those lines are then rewritten and not committed,
  so the case revision is unchanged. The next evaluation shows `repository.merge`
  approval-required, with no authority entry handed over.
- After that, `resume` is refused naming the capability as not granted.
- After that refusal, the request is Superseded naming a new Open request, and the stub merge
  program records no call.

Further bullets:

- A slice approval whose decider ground cites `repository.merge@<rev>` in `canon_authority`
  resolves through the slice's port once the grant is recorded
  (`crates/loom-intake-slice/tests/decision_authority_rule.rs`).
- Without `with_authority_records`, every existing governor test passes, with only the mechanical
  `authority: None` added to the struct literals listed under Files.
- `task check` exits 0.

## Out of scope

`principal`, `case_revision` and grounds on the entry
(`story:authority-records-bound-and-grounded-in-canon`). The tree's approvals
(`story:tree-waits-open-decision-requests`). Mandate approvals. An authority store of
the governor's own. Asking the authority provider about an action Canon admits.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (`decide_case`, a new `CanonGovernor::with_authority_records`) — cited
- **Files:** `crates/loom-governor/src/lib.rs` (`decide_case` at :2063; its callers :1081, :1342, :2055, :2323-2353) — cited
- **Files:** `crates/loom-governor/tests/authority_supplied.rs` (new, "ESS first") — cited
- **Files:** `ess/domains/governor.yaml` (`HeldAuthorityRecord` is added by `story:grounds-resolve-against-held-cases`; this story extends it) — cited
- **Files:** `ess/domains/evaluation.yaml` (request gains `authority`; comment at :71-72 replaced) — cited
- **Files:** `crates/loom-intake-slice/src/grant.rs` (the slice's `AuthorityRecords`) — cited
- **Symbols:** `AuthorityRecords`, `CanonGovernor::with_authority_records`, `ConflictingAuthorityRecords`, `HeldCaseGrounds`, `decide_case`, `evaluate`, `Supplied` — cited
- **Also likely:** `crates/loom-governor/src/authority_supplied.rs` (the story says "their own module" and gives no name) — inferred
- **Also likely:** `crates/loom-cli/src/evaluate.rs`, `crates/loom-cli/src/lib.rs` (`EVALUATE_HELP`, `EvaluateArgs`), `crates/loom-cli/src/main.rs` (`evaluate_command` :798) — inferred, from the `evaluate` bullet
- **Also likely:** `crates/loom-intake-slice/src/run.rs` (`slice_governor` :175) and `crates/loom-cli/src/main.rs:550` build the governor; both need the port — inferred
- **Also likely:** `crates/loom-intake-slice/src/decision.rs` (planned by `story:approval-decisions-resume-held-runs`; builds `HeldCaseGrounds` and the withholding) — inferred
- **Also likely:** `generated/rust/loom/src/evaluation.rs`, `generated/rust/loom/src/governor.rs` (regenerated), `website/docs/reference/ess/loom-evaluation.md`, `website/docs/reference/ess/loom-governor.md`, `website/docs/reference/cli.md` — inferred
- **Also likely:** the 11 test files that build `EvaluationRequest { .. }` literals, which stop compiling when the field is added — inferred (list in the `paths` block)
- **Documents:** `AGENTS.md` (:461 "supplies no authority decision"), module docs `lib.rs:53-56` and `:2230` — inferred
- **Confidence:** medium — governor, ESS and grant module are cited. The CLI `evaluate` wiring, the slice wiring and the new module's name are inferred, and the story has no Scope section.
- **Would collide with (a `depends_on` path orders it before this story):**
  - `story:decision-request-file-store`, `story:decided-record-written-then-recorded`, `story:question-and-outcome-decisions-hold-runs`: `crates/loom-governor/src/lib.rs` — cited
  - `story:grounds-resolve-against-held-cases`, `story:decision-request-file-store`: `ess/domains/governor.yaml` — cited
  - `story:approval-decisions-resume-held-runs`, `story:decision-suspension-reason`: `crates/loom-intake-slice/src/grant.rs` — cited
  - `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`: `crates/loom-intake-slice/src/run.rs`, `crates/loom-cli/src/main.rs` — cited
  - `story:decision-inbox-commands`, `story:silence-and-expiry-at-a-host-tick`: `crates/loom-cli/src/lib.rs` — cited
- **Would collide with (ordered after this story by `depends_on`):** `story:goals-entries-from-a-resolved-document` (`crates/loom-governor/src/lib.rs`, `crates/loom-cli/src/lib.rs`), `story:goals-grounds-and-free-answers` (`crates/loom-intake-slice/src/run.rs`, `crates/loom-cli/src/lib.rs`), `story:decisions-resume-the-held-run`, `story:daemon-continues-a-decided-request`, `story:concurrent-cases-in-the-tree-daemon`, `story:flow-limits-recorded-as-events` (`crates/loom-cli/src/lib.rs`) — cited
- **Would collide with (NO `depends_on` path either way):**
  - `crates/loom-governor/src/lib.rs`: `story:hash-chained-run-log` (active, wave w3), `story:bound-artifact-revisions`, `story:child-case-runtime`, `story:child-case-own-workspace`, `story:record-copy-between-cases`, `story:engineering-registry-calls-replaced`, `story:gates-document-evaluated-in-the-governor`, `story:standing-tree-rests-quiescent` — cited
  - `ess/domains/governor.yaml`: `story:child-case-runtime`, `story:child-case-own-workspace` — cited
  - `crates/loom-cli/src/lib.rs` or `main.rs`: `story:agent-node-run-records`, `story:agent-selects-the-connectors-relay`, `story:bound-commands-run-confined`, `story:confinement-opens-from-engine-config`, `story:run-caps-per-arrival`, `story:secrets-subcommand`, `story:standing-tree-wakes-at-a-host-instant`, `story:standing-tree-rests-quiescent`, `story:tree-run-command`, `story:tree-usage-and-model-budget` — cited
  - `crates/loom-intake-slice/src/run.rs`: `story:bound-commands-run-confined` — cited
  - None of the 11 test files with `EvaluationRequest` literals is in another open story's scope — inferred
- **Safety fact:** with no port given, `decide_case` passes `Supplied { authority: None }`, which is what it passes now (`lib.rs:2075-2078`), so unported hosts decide as before. Canon accepts an entry for a capability no action requires (`eval/authority.rs` docs). — step 2, unproven
