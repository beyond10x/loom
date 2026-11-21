---
format: aep.planning-md/3
id: story:tree-waits-open-decision-requests
kind: story
status: draft
title: Each decision wait of tree run is a decision request, decisions --tree acts on the tree's store, and the run log records the decision events
summary: Approvals and outcome decisions routed by holder; a person action stays tree approve --action; a final holder's end stops the case with exit 4; the tree's approvals reach Canon
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:question-and-outcome-decisions-hold-runs
- depends_on: story:tree-run-command
- depends_on: story:hash-chained-run-log
- depends_on: story:agent-node-run-records
- depends_on: story:approvals-supplied-to-canon-evaluation
revision: 6
---
## Outcome

`b10x-loom tree run` turns each wait on a decision into a decision request in the tree's store.
`tree open` takes the decision flags of `story:approval-decisions-resume-held-runs` and keeps
them, with the resolved grant table, beside the root case; `tree run` routes, at each wait:

- an action that needs a capability (`AwaitingAuthority`): an approval request whose record is
  the tree's `CaseApproval` (`story:tree-run-command`), written by `CaseApprovalWriter` with
  record id `<capability>@<case_revision>` at the request's decided case revision;
- an outcome blocked on a missing explicit decision: an outcome request, written by
  `ExplicitDecisionWriter`.

An action bound to a `person` executor (`AwaitingPerson`) is work, not a decision: it stays the
wait of `story:tree-run-command`, recorded with `tree approve --action`, and opens no request.

The request's subject is the subject of the action's evidence entries in the bindings file
(`story:bindings-document`); Canon names none (session `canon`, 2026-11-21). An action whose
evidence entries name more than one subject gives the request the subject `case` at the case
revision (session `sdc`, 2026-11-21, answer f): a request has one subject
(`~/beyond10x/sdc/ess/domains/decision.yaml:229-230`), and a binding may list several entries
(`story:bindings-document` § Outcome). The tree hands the router no touched paths, so a
`paths_touched` condition counts as matching (`story:escalate-when-conditions`). The router sets
`expires_at` from the classes document (`story:decision-routing-by-holder`). No flag supplies a
rule (Loom ships no rule): a `rule*` holder in a tree has an absent rule port, which routing treats
as a failed rule result, so `rule` and `rule_and_decider` escalate to the escalation person and
`rule_final` and `rule_and_decider_final` end the case's run.

A decider or rule holder decides in the same `tree run`, which goes on once the record is written.
Where the request waits on a person, `tree run` exits 3 and prints, per request, the case, the
request id, the question, the recommendation or the reason for none, and the exact
`b10x-loom decisions decide <ID> <OPTION> --as <PRINCIPAL> --tree <STORE>` command. `tree approve`
applies `Decide` with principal kind `person` to the request its case and capability name, then
completes it, so `story:tree-run-command`'s use of it is kept.

The next `tree run` continues past a wait only when its request carries a `produced_record`
whose grounds still resolve; a ground that no longer resolves supersedes the request and opens a
new one, as `story:approval-decisions-resume-held-runs` does for the slice. `tree run` builds its
`HeldCaseGrounds` with the tree's store directory as the root of `path_line` citations. In a
Loom-held case an `artifact_revision` ground cannot move alone: moving the artifact raises the case
revision (`crates/loom-governor/src/lib.rs:1042`), the record of the earlier case revision no
longer applies, and the stop is routed at the new revision under a new record key, while the old
request stays Decided with its record. The grounds that go stale while the case stays are
`path_line`, `aep_artifact` and `sdc_decisions` citations, and this check is what catches them.
An outcome request decided "leave it untaken" produces no record (answer d): the next `tree run`
opens no new request for that stop at that case revision and goes on directly, and a case left
with nothing useful and no legitimate outcome ends stuck (exit 4) as `story:tree-run-command` says.
`tree status` lists, per case, the ids of its Open and Escalated requests.

On `RunEnds` (`story:decision-routing-by-holder`) the case's run ends: `tree run` stops that case
as `Stuck` with the reason `decision <id> ended the run: <cause>`, takes no further step in the
tree, and exits 4 naming the case and the request, the status `story:tree-run-command` gives a
case that cannot go on (exit 3 means a wait someone can answer, which a final holder rules out).
The request stays as the route left it: Decided on the denying option after a veto, otherwise
Open, for the reasons `story:approval-decisions-resume-held-runs` gives (`Supersede` needs a newer
request, a listed person may still decide it, and the next `tree run` routes it again by its record
key). `tree status` shows the case stopped with that reason.

`b10x-loom decisions` gains `--tree STORE`: the inbox commands of `story:decision-inbox-commands`
and `decisions tick` act on the tree's decision store, with `CaseApprovalWriter` and
`ExplicitDecisionWriter` as their writers, and append the decision events below to the run log of
the request's case. So `decisions tick --at INSTANT --tree STORE` writes the `expired` and the
silence `decided` lines.

The tree's `CaseApproval` implements the `AuthorityRecords` port of
`story:approvals-supplied-to-canon-evaluation`, and `tree run` builds its governor and its
`HeldCaseGrounds` with it, so Canon's evaluation of a case sees the tree's approvals. As in the
slice, the port reports a `CaseApproval` only while the grounds of the Decided request that
produced it still resolve (with `canon_authority` citations resolved against the unfiltered
approvals); a record with a moved ground is withheld, Canon reports the action approval-required
again, and `tree run` supersedes the request and opens a new one.

Every decision event is a line in the case's run log: `decision requested`, `recommendation
recorded` or `withheld`, `escalated`, `escalation suppressed` (class, subject at revision, reasons,
`best`, cause), `decided`, `record produced`, `expired`, `superseded`; and `autonomy resolved` once
per tree in the root's log.

The tree's decision code goes in its own module of `crates/loom-tree`, and `--tree` in the
inbox's module of `crates/loom-cli`.

## Why

SDC runs its protocols on `tree run` (`epic:protocol-tree-on-bindings`), which stops "at the
next decision a person owes, naming the command that records it"
(`story:tree-run-command` § Outcome). This story makes that stop a decision request, so a tree is
held and continued by the autonomy policy, the decider and the inbox of this epic. The run log is
where `escalation_suppressed` is recorded for a live view
(`~/beyond10x/sdc/docs/autonomy-levels.md:460-462`). Canon must see the tree's approvals too
(session `sdc`, 2026-11-21, answer h). A `person` action produces evidence of a person's work; a
decider or rule holder must not produce it, so it is no decision request.

## Waits on

- `story:tree-run-command`: `tree run`, `tree approve`, `CaseApproval` and `crates/loom-tree`.
- `story:hash-chained-run-log`: the run log this story writes decision events into.
- `story:agent-node-run-records`: it adds `RunLogEvent` variants to `ess/domains/runlog.yaml` and
  a member to `ess/domains/tree.yaml`, which this story also changes.
- `story:approvals-supplied-to-canon-evaluation`: the `AuthorityRecords` port, the governor's
  supply and its grounds filter, and both change `crates/loom-cli/src/lib.rs`.
- `story:question-and-outcome-decisions-hold-runs`: both change `crates/loom-cli/src/lib.rs`.
- `story:decision-routing-by-holder`, `story:escalate-when-conditions`,
  `story:decided-record-written-then-recorded`, `story:decision-inbox-commands` and
  `story:silence-and-expiry-at-a-host-tick` through them.

It does not depend on `story:bound-agent-node`: the decider is not an agent node and works on no
workspace.

Shared with `epic:tree-daemon-under-enforced-limits`, with no edge either way:
`story:limit-hits-in-run-log` and `story:engine-configuration-document` also change
`ess/domains/runlog.yaml`, `ess/domains/tree.yaml` and `crates/loom-tree`;
`story:action-limits-enforced` and `story:tree-daemon` change `ess/domains/tree.yaml` and
`crates/loom-tree`; `story:confinement-level-required`, `story:executable-allow-list`,
`story:named-egress-per-action`, `story:confinement-level-on-oci-and-pod` and
`story:ingress-routes-per-action` change `crates/loom-tree`. Of this story and any of those, the
second to land rebases onto the first in each shared file, as `story:limit-hits-in-run-log` says
from its side. `story:budget-and-limit-stops-raise-decision-requests` and
`story:run-log-references-exec-file-changes` come after this story through their edges.

## ESS first

`ess/domains/runlog.yaml`: `RunLogEvent` gains the decision events above.
`ess/domains/tree.yaml`: `TreeRun` gains the decision settings, `NextDecision` gains the request
id, `TreeStatus` gains the open requests per case, and `Stuck` gains the ending request. On that
commit `task drift` fails, and `crates/loom-tree/tests/tree_decisions.rs` does not compile.

## Acceptance

With the three-level fixture tree of `story:tree-run-command`, fixture autonomy documents and
principals, a recorded decider, no rule and no model call:

- At `A0`, `tree run` exits 3 naming `DR-…` and printing its `decisions decide … --tree STORE`
  command; running it and `tree run` again continues; after the last decision the root ends
  through its one legitimate outcome and `tree run` exits 0.
- `decisions list --principal timo --tree STORE` lists the waiting request, and `decisions decide`
  with `--tree` writes the case's `CaseApproval` and one `decided` and one `record produced` line
  in that case's log.
- At `A3`, the recorded decider decides the `roadmap`-class approval with grounds that resolve,
  and `tree run` does not stop there.
- At `A3` with a classes document that puts a child's approval in `rollback` (holder `rule`),
  `tree run` writes one `escalated` line whose reason is `no rule configured`, exits 3 naming the
  request, and the request is Escalated to the escalation person.
- At `A5`, a recorded `Escalate` with `best` writes one `escalation suppressed` line with cause
  `escalate` and the tree goes on with `best`.
- At `A5` on a root outcome with a decider failure, `tree run` exits 4 naming the root and the
  request, the request is Open, the root's log has one `escalation suppressed` line with cause
  `failure`, `tree status` shows the root stopped with that reason, and a second `tree run` routes
  the same request again and opens none.
- `tree approve` on the waiting case and capability gives the same store and log bytes as the
  printed `decisions decide` command.
- A request Decided without a record (crash injected) holds the tree at the next `tree run`, which
  first completes it, writes one `record produced` line, and continues.
- At `A1` an approval taken on a strong recommendation whose `path_line` ground cites lines 1-3 of
  `notes.md` in the store directory: after those lines are rewritten, with the case revision
  unchanged, the case's `canon-decision/1` shows the capability approval-required with no
  authority entry handed over, and the next `tree run` writes one `superseded` line, opens a new
  request naming the old one in `supersedes`, and exits 3.
- A granted approval whose case then moves (`spec` revised, the case revision raised): the next
  `tree run` opens a request with record key `<capability>@<new revision>`, writes no
  `superseded` line, exits 3, and the old request stays Decided with its record.
- A recorded decider reply citing a moved revision at `A1` writes one `withheld` line with the
  reason, and the request waits on the person with no recommendation.
- With the classes document giving `roadmap` `expires_after: 3600` and `--at 12:00`, the request
  has `expires_at` 13:00; `decisions tick --at 13:00 --tree STORE` expires it and writes one
  `expired` line in the case's log; `tree run` then routes the stop again.
- The root's log has exactly one `autonomy resolved` line, written by `tree open`, and no child's
  log has one.
- A root outcome request decided "leave it untaken" writes no `record produced` line, and the next
  `tree run` opens no request for it and exits 4 naming the root.
- An action whose two evidence entries name `spec` and `plan` opens a request with subject `case`.
- An action bound to a `person` executor stops `tree run` with exit 3 and the `tree approve
  --action` command, as `story:tree-run-command` says, and opens no decision request.
- After a decided approval, the case's `canon-decision/1` document in the store shows the
  capability granted.
- A child's wait names the child's case and the request is in the child's log.
- `tree status` lists the open request of the waiting case and none after it is decided.
- `tree verify` exits 0 after the run.
- `docs/contracts/run-log-v1.json` is regenerated and names the decision events.
- `task check` exits 0.

## Out of scope

Handoffs: a writer's `handoff` stays the printed text of `story:tree-run-command`, since the
binding gives no options. An action bound to a `person` executor: it stays the wait of
`story:tree-run-command` (`tree approve --action`). Opening `budget_overrun` and `limit_hit`
requests at a budget or limit stop (`story:budget-and-limit-stops-raise-decision-requests`).
Recording the decider's model call in the run log. A daemon that ticks expiry. Computing touched
paths. A stop for a review recorded by a producer that is not independent
(`story:non-independent-review-becomes-decision-request`). A flag that supplies a rule.
