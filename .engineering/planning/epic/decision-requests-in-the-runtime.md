---
format: aep.planning-md/3
id: epic:decision-requests-in-the-runtime
kind: epic
status: proposed
title: A run that stops on a decision opens a decision request, routed by autonomy level, and resumes once it is decided
summary: loom.decision entity, autonomy grant table, decider port behind the grounding check, escalate_when, inbox, host-tick expiry, resume after the decision, approvals supplied to Canon, a non-independent review as a decision request
tags:
- sdc
relations:
- serves: vision:O1
- serves: vision:governed-autonomy
revision: 18
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T03:05:17Z", actor: "agent:sdc", revision: 11, decided_on: {"recorded":{"review_outcome":12}}, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

When a governed run stops on a decision (an approval-required action, a handed-off question, or
an outcome that needs an explicit decision), Loom opens a decision request for it, resolves who
holds the decision from the run's autonomy level and the request's class, asks a decider for a
recommendation where the holder involves one, accepts that recommendation only when every ground
it cites resolves, and routes the request to the principal who decides: a person's inbox, the
decider, or a rule. The run's `escalate_when` conditions send a decider's result to a person, or
end the run under a final holder; the slice and the tree each end such a run with a stated end
state and exit status. A request whose class the host gives an expiry duration expires at a host
tick, or is decided with its default on silence. Once it is decided on an option that produces a
Canon record, Loom writes that record, records it on the request, and resumes the held run; an
option that produces none resumes or ends the run directly. Loom hands the approvals it recorded
to Canon's evaluation, and withholds one whose grounds moved. In a tree, a review recorded by a
producer that is not independent becomes a decision request for a person or a decider, never a
re-run by the same producer. The request is an entity of Loom's ESS, `loom.decision`, with eight
commands; its store is durable, and its inbox is a set of CLI and SDK calls, not a service. A
decision recorded through the inbox over a state directory resumes the run held for it in the
same invocation, for an approval hold of a `software-change@1` run. A held question or outcome
resumes through the SDK function, with the host's executor.

## Why

Session `sdc` relayed a decision of the operator on 2026-11-21: a decision request is a
first-class entity, split between Canon (the pending decision as a derived fact, grounds
invalidated when a revision moves, the decided record) and Loom (the runtime)
(`~/beyond10x/sdc/docs/decisions.md:243-271`). The request to Loom, in the requester's words
(`decisions.md:299-315`):

> 1. A decider executor port: given a pending decision and its inputs, return an option, reasons and
>    grounds (rule or LLM); a recommendation is accepted only when every ground resolves
>    (`sdc/tools/decision-grounds.sh` is the reference behaviour), otherwise none, with the reason.
> 2. The autonomy policy of `sdc/docs/autonomy-levels.md`: level A0 to A5, holder per decision class
>    (`:242-257`), overrides and the order of holders (`:297-320`), escalation mode person or final
>    (`:385-387`), recording `escalation_suppressed`.
> 3. Routing and inbox: eligible principals (handed in by the host, resolved by Mandate), the
>    open-decisions query per principal, expiry with an optional default on silence (fired by a
>    host tick that passes the instant), escalation to a named person, and resume of the suspended
>    run (`resume_after_grant`) in this order: `Decide` (all guards, no Canon write), the Canon
>    write keyed by the record id, `RecordProduced`, then resume, which requires `produced_record`
>    where the decided option produces a record.
> 4. The entity itself (`sdc/ess/domains/decision.yaml`, commands `Request`, `Recommend`, `Decide`,
>    `TakeRecommendation`, `RecordProduced`, `Escalate`, `Supersede`, `Expire`) as the contract,
>    moved into Loom's ESS when it is taken.

The escalation mode of item 2 is met by the holders: the contract's guards read a request's
holder, not the level (`decisions.md:90-92`), suppression is what the final holders mean, and an
Escalate always sets the holder to `person`; so the grant table carries no separate mode
(`story:autonomy-grant-table`).

The contract is `~/beyond10x/sdc/ess/domains/decision.yaml` (final at commit `00e3557`; story 1
copies it whole, and story 23 gives behaviour to what it gained after the first read), which session `sdc` declared final
on 2026-11-21 (49 declarations, with the scenario `ess/scenarios/merge-a5-veto.yaml`). Session
`sdc` answered nine questions on it that day; the stories cite them as "session `sdc`,
2026-11-21, point N":

1. `Holder` keeps the underscored values and gains `rule_final`; "R, then P" is `rule`, "R: end"
   is `rule_final`; overrides use the same values.
2. Class names are the contract's (`decision.yaml:176`).
3. Under `rule_and_decider*` the rule's verdict is the strong recommendation; the decider takes it
   or vetoes, never grants against it. A veto is `Escalate` under `rule_and_decider` and a
   `Decide` on the denying option under `rule_and_decider_final`, which ends the run as refused.
4. `Escalate` moves from Open and Escalated; `rule_final` joins the suppressed holders; under
   `person` or `advised` an Escalate is a person re-addressing the request.
5. `TakeRecommendation` needs strength strong for every principal.
6. `Decide` first, with no Canon write; then the runtime writes the Canon record (idempotent);
   then a new command `RecordProduced` sets `produced_record`; then resume, which requires
   `produced_record`.
7. Record ids: `<capability>@<case_revision>` and `<outcome>/<decision>@<case_revision>`. Loom
   writes the case revision `r<N>`, as its governor stamps it, and reads an outcome's decision
   name from the request's `record_key` (`story:decided-record-written-then-recorded`).
8. Expiry fires at a host tick with an explicit instant; no clock read inside the transition.
9. Loom owns the types and the resolver; presets, classes and admitted ceilings are documents the
   host passes in. A class is an identifier checked against the host's classes document;
   `budget_overrun` and `limit_hit` are built in. `escalation` is a policy row, not a request
   class.

Eligible principals are handed in by the host, not resolved by Mandate (accepted by session `sdc`
the same day), and a resume needs a recorded decision, not only a grant.

Session `sdc` answered a second set the same day; the stories cite these as "session `sdc`,
2026-11-21, answer x":

- a. Holders are partially ordered: `person < advised < rule_and_decider < {rule, decider}`, each
  non-final holder below its final variant, `rule_and_decider_final < {rule_final,
  decider_final}`. A move down is a lowering; every other move, between incomparable holders
  included, is a raise. `limit_hit` has no override (`autonomy-levels.md:299-316`).
- b. Under `decider_final` a missing, failed or withheld decider result decides the denying
  option, or ends the run where no option denies; `escalation_suppressed` is recorded with the
  cause. Loom treats an absent decider or rule port as a failed result of that port, with no new
  cause.
- c. A rule verdict withheld by the grounding check under `rule_and_decider*` is no
  recommendation: to a person when non-final, the run ends when final.
- d. An option declares whether it produces a Canon record. "Leave it untaken" produces none, so
  resume or end follows `Decide` directly; `RecordProduced` and the `produced_record` resume
  condition apply only to options that produce a record.
- e. The record's revision is the case revision at `Decide`; if the case moved before the Canon
  write, the write is refused and the request is superseded.
- f. Several evidence subjects give the subject `case`.
- g. Idempotence by record id; no request id on the Canon entry, `produced_record` holds the link.
  Two open requests for one record id never exist: a second `Request` is refused, naming the open
  request, and the runtime reuses that one (the coordinator's choice of the two sdc allowed).
- h. Loom supplies `canon-authority/1` records to Canon's evaluation: an approval Canon does not see
  cannot make an action admissible in Canon's own evaluation or in a replay. Once Canon's
  `story:decision-grounds` lands, Canon checks grounds on approvals and Loom's own check stays for
  pending recommendations only.

`escalate_when` is a list of single-key conditions from a closed set (`decider: escalate`,
`decider: invalid`, `paths_touched`, `decision_cost_usd_over`), evaluated by a program after the
decider replies and before `Decide` (`autonomy-levels.md:366-370`); `budget_overrun` and
`limit_hit` requests are wanted under `epic:tree-daemon-under-enforced-limits`.

A third message the same day, written by the contract's author and accepted by `sdc`, settled
six points the contract now checks: the denying option is the one whose `consequence.grant` is
`denied`; Escalate always sets the holder to `person`; `RecordProduced` with the same record id
again changes nothing and a different one is refused `ProducedRecordConflict`; `Expire` takes
`at`, and Expire and silence before `expires_at` are refused `NotDue`; `Request` refuses class
`escalation` (`NotARequestClass`); `RecordProduced` on an option that produces no record is
refused `NoRecordToProduce`. Two rules stay prose only, and Loom enforces them: supersede when the
case revision moved before the Canon write, and no two open requests for one record id
(`decisions.md:225-228`). The same message records SDC's selector rule: a review recorded by a
producer that is not independent becomes a pending decision of class `code_review` or
`spec_acceptance`, never a re-run by the same producer. A fourth message answered that a decider's
review evidence is independent under `independent_on: [different_principal]` when its principal
differs from the producer's, on three conditions
(`story:non-independent-review-becomes-decision-request`).

A fifth message the same day accepted what the stories assume on three points: Loom keeps checking,
on decided approvals, the ground kinds Canon's entry cannot carry; a question's record is
`ARTIFACT@REV` of any artifact the case declares, SDC's `questions.yaml` being one instance; and
the run log counts as "beside the principal" for a decider's model and adapter. On the fourth
point SDC corrected its table: the A5 rollback cell is now "R: end" (`autonomy-levels.md:253`),
and an eval run refuses every override that names a non-final holder (`:317`), which Loom's
`EvalOverride` follows (`story:autonomy-grant-table`). Session `canon` answered the same day that
expiry of cited evidence makes an evidence ground stop holding at an unchanged case revision
(stories 18 and 19).

What Loom has, read at `ecefe180`: a run suspends only when the authority provider answers
`ApprovalRequired` (`crates/loom-commission/src/runtime.rs:384-392`), and `resume_after_grant`
performs the held request once (`runtime.rs:2374`); `NeedsHumanJudgment` ends the run with no
Suspension (`crates/loom-commission/src/outcome.rs:118-124`), and the intake slice treats it as
unexpected (`crates/loom-intake-slice/src/run.rs:928`). Nothing in Loom resembles a decision
request, an inbox, a holder, an autonomy level, a grounding check or a decider. "Commission sets
no timer: the host resumes the work" (`AGENTS.md:423`).

## Stories

In dependency order. Story 0 raises the format of Loom's main ESS system so the contract's copy
is valid there; story 1 depends on it. Stories 1 to 15 and 21 build against fixtures on the Canon
Loom pins today. Stories 16 and 20 wait on `story:tree-run-command`, and through it on
`story:canon-0-9-0-pin`, which is unblocked (engineering-protocols `0.16.0` pins Canon `0.9.0`) and
not yet delivered. Story 22 waits on story 16 and on `story:tree-daemon`
(`epic:tree-daemon-under-enforced-limits`): a decision on a served tree goes through the daemon,
and story 16 does not wait on the daemon chain. Story 23 waits on story 22, and story 20 waits on
story 23. Story 24 waits on story 23, `story:engine-configuration-document` and `story:tree-daemon`.
Stories 17,
18 and 19 wait on Canon (§ Upstream). Each story's code goes in its own module, so a crate's
`lib.rs` shared by several stories gains one `mod` line per story; the order is kept, and the wave
planner keeps two stories that share a file apart. Story 13 depends on story 10 because the type
change of story 10 forces an edit in `crates/loom-intake-slice/src/grant.rs`, which story 13 also
edits. Story 11 depends on story 10 because both edit `crates/loom-cli/src/lib.rs` (story 10's
resume refusals). Story 15 depends on story 14 because both edit `crates/loom-cli/src/main.rs`,
`crates/loom-cli/src/lib.rs` and the slice's governor build. Story 20 depends on story 7 because it
adds the review stop to routing's `ess/domains/decision.yaml` and routing module. Story 21 is
listed last but comes after story 15 and before story 16: it depends on story 14
(`resume_decision_hold`), on story 15 (both edit `crates/loom-cli/src/main.rs` and `lib.rs`) and
on story 12 (the tick it resumes after), and story 16 depends on it (both edit
`crates/loom-cli/src/decisions.rs`, `lib.rs` and `main.rs`). Story 22 depends on four stories:
stories 16 and 21 (story 21 for `crates/loom-cli/src/decisions.rs`), `story:tree-daemon`, and
`story:concurrent-cases-in-the-tree-daemon` (`epic:concurrent-and-standing-case-trees`), whose
per-case states `Decide` and `Tick` move a case through. Story 23 comes after stories 1 to 14 and
holds none of them. It depends on ten stories:
- stories 2, 5, 6, 11, 12, 14 and 21, the surfaces it extends;
- story 22, its eighth edge: its type changes to `Decide` and `Ground.supports` force an edit in
  the `--tree` and daemon code, so it lands after story 22, and through it after story 16;
- stories 1 and 4, whose declarations and `check_grounds` it cites.

Story 20 depends on story 23 for two reasons. Story 23's change to `DeciderReply::Propose` and to
the `ModelDecider` schema meets story 20's extension of `ModelDecider` and its `decider:stub`
replies, so whichever lands second edits the other's code. And story 20 also waits on
`story:selector-stop-names-set-aside-record`, which should not hold story 23.

Story 24 has three edges:
- story 23 (the port, `goals_verdict` and the tree's port);
- `story:engine-configuration-document` (`loom.engine/1` and its loader);
- `story:tree-daemon` (`tree serve --engine`).

No edge orders it against the other stories that add a section to `ess/domains/engine.yaml`. Each
adds a section of its own and reads none of the others', the typed scope names the shared file,
and `aep plan artifact waves` keeps two stories with one path out of one wave. The coordinator
removed seven such edges, because five of those stories are drafts of an epic nobody has accepted
(story 24 § Waits on).

A host port
whose answer becomes a command input is declared in `ess/domains/decision.yaml`: `CaseRevisions`
(the `case_revision` of `Decide` and `TakeRecommendation`, story 11) and `ExpiryInstant` (the
`expires_at` of `Request`, story 7). A port no command input comes from, such as `DecisionStore`,
is declared in Rust only.

0. `story:ess-format-31`: `ess/system.yaml` goes from `ess/25` to `ess/31` on the pinned `ess`
   0.86.0, so the contract's `[]` in `sets:` and its `coalesce` guard are valid; the raise
   regenerates four files of `generated/rust/loom` and changes no command's outcome or stored row.
0b. `story:ess-format-37`: `ess/system.yaml` goes from `ess/31` to `ess/37` on the same pinned
   `ess` 0.96.0, so the copy of the decision domain keeps `ordered: true` on `Strength` and
   `AutonomyLevel`; the raise changes no command's outcome, stored row or generated file.
1. `story:decision-request-entity`: the `loom.decision` domain, the crate that applies its eight
   commands, and the `DecisionStore` port with the records kept beside a request.
2. `story:decision-request-file-store`: requests and their records kept a file each in a store
   directory, with the three views, across a restart.
3. `story:autonomy-grant-table`: level, presets, classes (with an expiry duration per class),
   overrides and admitted ceilings resolve into one grant table per run; an eval run refuses an
   override to a non-final holder.
4. `story:decider-port-and-grounding-check`: the `Decider` port, a rule adapter, and the
   grounding check of `decision-grounds.sh` over a citation resolver.
5. `story:grounds-resolve-against-held-cases`: the citation resolver over a governor-held case,
   its records, the decision store and file roots.
6. `story:llm-decider`: a decider over an llm `Model`, recorded responses only.
7. `story:decision-routing-by-holder`: a stop becomes a request with its `expires_at`, routed by
   its holder (an absent decider or rule as a failed result), with escalation and
   `escalation_suppressed`; a Decided request for the record key is reused.
8. `story:escalate-when-conditions`: the four `escalate_when` conditions, checked by a program
   after the decider replies and before `Decide`.
9. `story:decided-record-written-then-recorded`: Decide, then the Canon record, then
   `RecordProduced` for an option that produces one; a retry completes a half-finished decision;
   a request whose record kind has no writer waits for one.
10. `story:decision-suspension-reason`: Commission holds a run for a decision request and resumes
    it only on a decided request, with its record where the option produces one.
11. `story:decision-inbox-commands`: `b10x-loom decisions` list, show, history, decide, take,
    escalate, supersede, and the same calls in the SDK.
12. `story:silence-and-expiry-at-a-host-tick`: `decisions tick --at` expires or takes the default
    on silence.
13. `story:approval-decisions-resume-held-runs`: `b10x-loom run --state` routes its approvals
    through requests, ends the run on a final holder's end, and `resume` performs the held request
    only on a recorded grant.
14. `story:question-and-outcome-decisions-hold-runs`: a slice run is held, not ended, on a
    question or an outcome decision, and resumed on its decision; `decisions record` completes a
    question's record through `complete_decision`.
15. `story:approvals-supplied-to-canon-evaluation`: the governor hands Canon the approvals the
    slice recorded, as `canon-authority/1` records at the current case revision, withholding one
    whose grounds moved.
16. `story:tree-waits-open-decision-requests`: each decision wait of `tree run` is a decision
    request, `decisions --tree` acts on the tree's store, the tree's approvals reach Canon, and
    the run log records the decision events.
17. `story:requests-from-canon-pending-decisions`: Loom moves to the Canon release with
    `pending_decisions` and builds every stop from that section (blocked).
18. `story:decided-records-carry-grounds-to-canon`: an explicit decision carries its grounds to
    Canon, and one Canon sets aside supersedes the request (blocked).
19. `story:authority-records-bound-and-grounded-in-canon`: those records carry principal, case
    revision and grounds, and Canon, not Loom, checks the grounds it can see on a decided approval
    (blocked).
20. `story:non-independent-review-becomes-decision-request`: a review recorded by a producer that
    is not independent becomes a `code_review` or `spec_acceptance` request in `tree run`, decided
    by a person or an independent decider, never re-run; it also waits on story 7, on
    `story:selector-stop-names-set-aside-record`, and on story 23 (listed later, landing before it).
21. `story:decisions-resume-the-held-run`: `decisions decide`, `take` and `record` over
    `--state` resume the run held for the request in the same invocation and print its next
    state, for approval holds of `software-change@1` runs. A held question or outcome resumes
    through `resume_decided_request` with the host's executor, and the command line refuses it
    `NotRebuildable`. `tick --state DIR --resume` resumes every run held on a request decided by
    silence, `b10x-loom stop` keeps a stopped run from any resume, and no resume command is added.
22. `story:daemon-continues-a-decided-request`: on a tree a daemon serves, `decisions decide`,
    `take` and `tick` with `--tree` go through the daemon's `Decide` and `Tick` commands, and the
    decided case leaves `Waiting` and continues with no further command, after a restart too.
23. `story:goals-grounds-and-free-answers`: a ground citing one goals entry, checked by rule D93
    through a host port, and a free answer on a question request, as the contract at `00e3557`
    declares them. The contract's members are story 1's copy; this story adds Loom's declarations
    and the behaviour.
24. `story:goals-entries-from-a-resolved-document`: a file-backed `GoalsEntries` provider. It
    reads the resolved goals document (`sdc.goals/1`, as `sdc-goals resolve` writes it) that the
    engine configuration names, for `tree run` and the daemon (low priority). It reads the
    document only when its sha256, written as SDC writes a revision, is the root case's `goals`
    revision.

Two stories session `sdc` also asked for are under `epic:tree-daemon-under-enforced-limits`:
`story:run-log-references-exec-file-changes` (the per-exec file-change reference) and
`story:budget-and-limit-stops-raise-decision-requests` (budget and limit stops raise
`budget_overrun` and `limit_hit` requests).

## Not in scope

- A timer or a service. Expiry and the default on silence fire at a host tick with an explicit
  instant (`story:silence-and-expiry-at-a-host-tick`); the tree daemon of
  `epic:tree-daemon-under-enforced-limits` calling that tick is a later story of that epic. No
  CLI flag sets an expiry: the classes document gives it per class. No flag fixes the instant of
  `b10x-loom run`: a request's printed `expires_at` is what a tick is given.
- Listing or resolving principals. The host hands in the eligible principals per class; Loom
  resolves none through Mandate and authenticates no principal named on the command line.
- The level presets, the decision classes and the admitted ceilings as data. They are documents
  the host passes in; SDC ships its own (session `sdc`, 2026-11-21, point 9).
- A rule. Loom ships none, and no command-line flag supplies one; a host hands a rule in through
  the SDK, and on the command line a rule holder has none.
- A run-wide escalation mode beside the holders.
- A change of autonomy level within a run, and any record of one: no story changes a run's level.
- An action bound to a `person` executor in a tree. It is work, not a decision, and stays the wait
  of `story:tree-run-command` (`tree approve --action`).
- Computing the paths an action touched. The host hands them in for `paths_touched`; the slice
  and the tree hand none, so the condition counts as matching there
  (`story:escalate-when-conditions`).
- Opening `budget_overrun` and `limit_hit` requests at a budget or limit stop. The two classes
  are built in here; `story:budget-and-limit-stops-raise-decision-requests` of
  `epic:tree-daemon-under-enforced-limits` raises them.
- Revoking a Canon record when a Decided request is superseded. The record stays; Canon sets it
  aside when its grounds move (`story:decided-records-carry-grounds-to-canon`).
- Learning from decided requests (`beyond10x/decider`), and mirroring requests into AEP as
  `approval-record` or `decision-blocker`.
- The KPIs R18 to R25 of `autonomy-levels.md` § 5.
- Writing a question's answer into an application file. The host writes it and records its
  revision with `decisions record` (`story:question-and-outcome-decisions-hold-runs`).
- A review independence demand beyond `different_principal`, and model or adapter members on
  Canon's evidence (`story:non-independent-review-becomes-decision-request`).
- Changes to Canon, and changes to any artifact of `epic:protocol-tree-on-bindings`.
- Raising `ess/intake/system.yaml` or `ess/commission/system.yaml`, moving the ESS toolchain, and
  `ordered:` on any enum but `Strength` and `AutonomyLevel` (stories 0 and 0b raise only
  `ess/system.yaml`; session `sdc` decided on 2026-11-21 that story 0b raises it to `ess/37` and
  the copy of the decision domain keeps `ordered: true` on those two).
- Resuming a question or outcome hold from the command line, and a general rebuild of a commission
  from its resume context. Session `sdc` holds no questions in slice runs driven through the
  binary (`.engineering/drafts/e3-sdc-answers-11.md`, (a)), so that is a further story, not drafted.

SDC's contract extension of 2026-11-21 is no longer out of scope: story 23 carries it. The free
answer is keyed on the question kind, as the contract keys it (`when_subject: unblocks.kind !=
question`, `~/beyond10x/sdc/ess/domains/decision.yaml:493`, `:571` at `00e3557`), not on the classes
scope-changing answer and finding named earlier.

## Upstream

No Canon release carries the decision side yet. Session `canon` reported on 2026-11-21 three
stories, accepted on its local wave branch (commit `c295fae`, not pushed), none released:

- `story:authority-revision-binding` (canon wave 3, next to start): `principal` and
  `case_revision` on `canon-authority/1` decisions.
- `story:pending-decisions` (canon wave 5): `canon evaluate` writes a section `pending_decisions`
  in `canon-decision/1`. `authority` entries `{action, capability, scope?, current:
  none|require_approval, case_revision?, options: [granted, denied]}`, one per requirement that
  makes an action approval-required; `decisions` entries `{outcome, decision, case_revision?,
  options: [taken]}`, one per outcome blocked on a missing explicit decision. It carries no
  principal, holder, routing, expiry or recommendation, and no subject artifact of the action
  (`protocol/1` declares none).
- `story:decision-grounds` (canon wave 6): `ground_revisions` and `ground_evidence` on the entries
  of `canon-authority/1` and `canon-decisions/1`; evaluation sets an entry aside when a ground no
  longer holds and lists `stale_grounds` on its reason. Canon judges grounds only on a decided
  entry; a pending recommendation is checked by Loom with the same two rules. An evidence ground
  stops holding when its record expires by `max_age` at the evaluation instant, with the case
  revision unchanged (session `canon`, 2026-11-21); the key names follow Canon's release.

Both later stories depend on `story:authority-revision-binding` and on `story:missing-evidence`
(canon wave 4). `upstream-blocker:canon-decision-side-unreleased` records this and blocks stories
17, 18 and 19. Consequences for the stories that do not wait:

- Until Canon ships `pending_decisions`, a request is built from Loom's own stop and
  `CanonGovernor::decision_document` (`AGENTS.md` § Governor, `:484-492`), or from fixtures.
- `subject_artifact` never comes from Canon: the host names it, and in a tree the bindings file
  maps the action to its subject.
- Loom's grounding check applies Canon's two rules to a pending recommendation itself: the cited
  revision is still current, and the cited evidence record is still present. Until story 19, Loom
  also re-checks a decided approval's grounds before it allows and before it hands the approval to
  Canon, and hands Canon its approvals in today's `{capability, decision}` shape at the current
  case revision (stories 15 and 16). In a Loom-held case an `artifact_revision` ground cannot move
  without the case revision, so the grounds this catches are `path_line`, `aep_artifact` and
  `sdc_decisions` citations; Canon's entries cannot carry those, so Loom keeps checking them after
  story 19 too (accepted by session `sdc` on 2026-11-21).
- Loom pins Canon at `0.8.0` (`crates/loom-governor/Cargo.toml:15`); `story:canon-0-9-0-pin` moves
  it to `0.9.0`. It is unblocked, since engineering-protocols `0.16.0` pins Canon `0.9.0`, and not
  yet delivered.

## Acceptance

- A fixture run stopped on an approval at A0 opens one request addressed to the host's person;
  `b10x-loom decisions list --principal` shows it; `decisions decide` on the granting option
  writes the record, records it on the request, and `b10x-loom resume` performs the held action
  once.
- The same approval in a class whose holder is `decider` at the run's level (`context` at A2),
  with a recorded decider reply whose grounds all resolve, is decided by `decider:<kind>` with no
  person, and the run goes on.
- The same approval at A5 with a recorded decider reply that escalates records one
  `escalation_suppressed` with class, subject at revision, reasons and `best`, and is decided with
  `best`.
- A decider reply citing a revision that is no longer current leaves the request with no
  recommendation and `no_recommendation_reason` naming the citation.
- A slice run whose class the classes document gives `expires_after` opens a request with
  `expires_at`; a `decisions tick --at` given that printed instant decides a request with a strong
  default by `silence`, after which `resume` performs the held action once, and expires one
  without, whose run stays held.
- A crash injected after `Decide` and before `RecordProduced` leaves a Decided request without a
  record; the next attempt writes the record once and the run resumes once.
- In a library test with a fixture rule handed in through the SDK, the same approval under
  `rule_and_decider` with `escalate_when: [{decider: escalate}]` and a recorded decider reply that
  escalates goes to a person; under `rule_and_decider_final` the slice run ends
  `stopped: Refused` with one `escalation_suppressed` of cause `condition` and the request left
  Open. On the command line, with no rule, the same approval at A5 ends `stopped: Refused` with
  exit 3 and cause `failure`.
- A granted approval whose `path_line` ground changed while the case revision stayed is not handed
  to Canon, and `resume` supersedes its request instead of performing the action.
- A fixture outcome stop decided "leave it untaken" writes no Canon record; `resume` releases the
  hold and the run ends `NoAdmissibleAction` with nothing performed.
- A fixture tree whose root waits on an approval exits 3 naming a request id; deciding it with
  `decisions decide … --tree` and running `tree run` again continues; a root outcome under
  `decider_final` with a failed decider exits 4 naming the root and the request; `tree verify`
  exits 0.
- After a recorded approval, Canon's own `canon-decision/1` for the case lists the action
  admissible (stories 15 and 16).
- A fixture tree whose review is recorded by the implementing producer opens a `code_review`
  request instead of stopping; decided by `decider:stub`, the review record's producer is
  `decider:stub` and the claim holds (story 20).
- An eval run with an override to `decider` is refused `EvalOverride` (story 3).
- `ess/system.yaml` declares `format: ess/37`, `task drift` exits 0, and `CHANGELOG.md` names the
  raise to `ess/31` (story 0) and the raise to `ess/37` (story 0b) each in one `### Changed`
  bullet and in no `**Breaking:**` bullet.
- At an approval hold of a `software-change@1` run, `decisions decide DR-… A --as timo --state DIR`
  performs the held request once and prints the resumed run's last line `stopped: …` (story 21).
- At an outcome hold of a slice run, `resume_decided_request` with a scripted executor brings the
  run to `stopped: Completed` (story 21).
- A goals ground read from a fixture resolved goals document named in the engine configuration is
  verified by `tree run`, and one whose entry digest differs is refused `digest_changed`
  (story 24).
- A goals document whose revision is not the root case's `goals` revision is refused
  `invalid_goals` (story 24).
- On a tree served by `tree serve` that waits on `DR-…`, `decisions decide DR-… A --as timo
  --tree STORE` exits 0, and the daemon brings the root to its terminal outcome with no further
  command (story 22).
- A goals ground whose entry digest is unchanged after the goals document moved is verified with
  the note `revision_moved`, and `decisions decide DR-q --answer "neither"` leaves the question
  request Decided with that answer (story 23).
- No test makes a model or network call; `task check` exits 0 on every story's commit.

## Source

Session `sdc`, 2026-11-21: the relayed request, nine answers, the second answers a to h with
`escalate_when` and the budget and limit requests, the third message with the final contract, the
fourth message on a decider's review independence, the fifth message on the four points of the
e3r3 ledger, the sixth message choosing `ess/31` for Loom's main system
(`.engineering/drafts/e3-sdc-answers-6.md`, with the probe `e3-ess-format-probe.md`), the
message deciding wave 2026-11-21-w6, which raises it to `ess/37` so that the copy of the
decision domain keeps `ordered: true` (`approval-record:loom-wave-2026-11-21-w6`), and the
accepted design calls (an Escalate moves the existing request; a Supersede of
a Decided request does not revoke the Canon record; the host hands in eligible principals; the
inbox is a store plus CLI and SDK calls). Session `canon`, 2026-11-21: the three Canon stories
above and the answer on expired evidence grounds. Documents read:
`~/beyond10x/sdc/docs/decisions.md`, `autonomy-levels.md`, `fs-changes.md`,
`sdc/tools/decision-grounds.sh`, `sdc/ess/domains/decision.yaml`,
`sdc/ess/scenarios/merge-a5-veto.yaml`.
