---
format: aep.planning-md/3
id: story:tree-waits-open-decision-requests
kind: story
status: proposed
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
- depends_on: story:decisions-resume-the-held-run
scope:
- confidence: cited
  path: crates/loom-cli/src/decisions.rs
- confidence: cited
  path: crates/loom-cli/src/lib.rs
- confidence: cited
  path: crates/loom-cli/src/main.rs
- confidence: inferred
  path: crates/loom-cli/src/tree.rs
- confidence: inferred
  path: crates/loom-cli/tests
- confidence: inferred
  path: crates/loom-cli/tests/fixtures/tree
- confidence: cited
  path: crates/loom-cli/tests/fixtures/tree/decisions
- confidence: cited
  path: crates/loom-cli/tests/tree_decisions.rs
- confidence: inferred
  path: crates/loom-runlog
- confidence: cited
  path: crates/loom-tree
- confidence: inferred
  path: crates/loom-tree/Cargo.toml
- confidence: cited
  path: crates/loom-tree/tests/tree_decisions.rs
- confidence: cited
  path: ess/domains/runlog.yaml
- confidence: cited
  path: ess/domains/tree.yaml
revision: 17
transitions:
- {from: "draft", to: "proposed", at: "2026-11-21T04:44:12Z", actor: "agent:sdc", revision: 17, executor: "agent:loom", correlation: "sdc-standing-rule-2026-11-21"}
---
## Outcome

`b10x-loom tree run` turns each wait on a decision into a decision request in the tree's store.
`tree open` takes the decision flags of `story:approval-decisions-resume-held-runs` and keeps
them, with the resolved grant table, beside the root case; `tree run` routes, at each wait:

- an action that needs a capability (`AwaitingAuthority`): an approval request whose record is
  the tree's `CaseApproval` (`story:tree-run-command`), written by `CaseApprovalWriter` with
  record id `<capability>@r<N>` at the request's decided case revision `r<N>`
  (`story:decided-record-written-then-recorded`);
- an outcome blocked on a missing explicit decision: an outcome request, written by
  `ExplicitDecisionWriter`.

This story creates `CaseApprovalWriter`: no story before it defines one. It is a Rust
implementation of the `DecisionRecordWriter` port of `story:decided-record-written-then-recorded`
in the tree's decision module of `crates/loom-tree`, as `GrantRecordWriter`
(`story:approval-decisions-resume-held-runs`) is the slice's. It writes the request's record as
the case's `CaseApproval` with the request's principal and decided case revision, idempotent by
record id. It adds no ESS noun: the record it writes is the `CaseApproval` that
`ess/domains/tree.yaml` declares (`story:tree-run-command`).

`story:tree-run-command` stops with `AwaitingPerson` in two cases, and neither opens a request:

- an action bound to a `person` executor is work, not a decision: it stays that story's wait,
  exit 3 with the `tree approve --action` command;
- an admissible writer whose binding has a `handoff`: it stays that story's wait, exit 3 with the
  `handoff` text and `tree run` again as the command, since the binding gives no options.

Only `AwaitingAuthority` and an outcome blocked on a missing explicit decision open a request.

The request's subject is the subject of the action's evidence entries in the bindings file
(`story:bindings-document`); Canon names none (session `canon`, 2026-11-21). An action whose
evidence entries name more than one subject gives the request the subject `case` at the case
revision (session `sdc`, 2026-11-21, answer f): a request has one subject
(`~/beyond10x/sdc/ess/domains/decision.yaml:231-232` at `00e3557`), and a binding may list several entries
(`story:bindings-document` § Outcome). The tree hands the router no touched paths, so a
`paths_touched` condition counts as matching (`story:escalate-when-conditions`). The router sets
`expires_at` from the classes document through the `ExpiryInstant` port
(`story:decision-routing-by-holder`), which `tree run` implements by adding the class's seconds to
the instant it routes at. No flag supplies a rule (Loom ships no rule): a `rule*` holder in a tree
has an absent rule port, with the principal `rule:none`, which routing treats as a failed rule
result, so `rule` and `rule_and_decider` escalate to the escalation person and `rule_final` and
`rule_and_decider_final` end the case's run.

`tree run` calls `complete_pending` (`story:decided-record-written-then-recorded`) before it routes
any stop, so a record a crash left written but unrecorded is recorded before routing reads the
request.

A decider or rule holder decides in the same `tree run`, which goes on once the record is written.
Where the request waits on a person, `tree run` exits 3 and prints, per request, the case, the
request id, the question, the recommendation or the reason for none, and the exact
`b10x-loom decisions decide <ID> <OPTION> --as <PRINCIPAL> --tree <STORE>` command. `tree approve`
applies `Decide` with principal kind `person` to the request its case and capability name, then
completes it, so `story:tree-run-command`'s use of it is kept.

The next `tree run` continues past a wait only when its request carries a `produced_record`
whose grounds still resolve; a ground that no longer resolves supersedes the request and opens a
new one, as `story:approval-decisions-resume-held-runs` does for the slice. `tree run` builds its
`HeldCaseGrounds` with the tree's store directory as the case directory of `GroundRoots`, the root
of `path_line` citations (`story:grounds-resolve-against-held-cases`). In a
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
silence `decided` lines. For `--tree` the tree implements the tick's `CaseRevisions` port
(`story:silence-and-expiry-at-a-host-tick`) from the governor of the request's case. `--tree`
names the one store a command acts on, so it conflicts with `--store` and `--state`: given with
either, the command line is not valid (exit 2). `--at` keeps its meaning on `tick`.

Under `--tree`, `decisions decide` and `take` resume nothing by themselves. After their output
they print two lines (`story:decisions-resume-the-held-run`): `unblocked: <case> (DR-…)` and
`continue: b10x-loom tree run <STORE>`, with `<STORE>` the path exactly as given to `--tree`.

`decisions … --tree` writes the tree's store, so it takes the store's one-writer lock as the
`tree` commands do (`story:tree-run-command`). On a store a daemon serves, it is therefore refused
`StoreServed`, naming the daemon's socket, as `story:tree-daemon` refuses any direct command that
finds the lock held by a daemon (its § Outcome). It exits 37, a status of `decisions` clear of
the inbox's and of 30 to 36, and changes nothing. A store held by another direct command is
refused `StoreInUse` (`story:tree-run-command`), with the same exit 37. This holds until
`story:daemon-continues-a-decided-request` sends the decision through the daemon. This story tests
only `StoreInUse`. `StoreServed` needs `b10x-loom tree serve`, which `story:tree-daemon` brings,
and nothing orders this story after that one. A served store still refuses a decision with exit 37
once `story:daemon-continues-a-decided-request` lands, when nothing listens on the socket. That
story depends on both and carries the bullet (its § Acceptance, `--socket` naming a path nothing
listens on).

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

The tree's decision code, `CaseApprovalWriter` with it, goes in its own module of
`crates/loom-tree`, and `--tree` in the inbox's module of `crates/loom-cli`. The bullets that run
the `b10x-loom` binary are in `crates/loom-cli/tests/tree_decisions.rs`, beside
`story:tree-run-command`'s `crates/loom-cli/tests/tree_run.rs`, over its fixture tree
`crates/loom-cli/tests/fixtures/tree/`; `crates/loom-tree/tests/tree_decisions.rs` holds the
library bullet for `CaseApprovalWriter`.

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
- `story:decisions-resume-the-held-run`: it says what `decide` and `take` print under `--tree`,
  and both change `crates/loom-cli/src/decisions.rs`, `lib.rs` and `main.rs`.
- `story:approval-decisions-resume-held-runs` (the decision flags `tree open` takes, and
  `GrantRecordWriter`, the pattern `CaseApprovalWriter` follows): indirect, through
  `story:approvals-supplied-to-canon-evaluation` and `story:decisions-resume-the-held-run`.

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
from its side. The same holds, with no edge either way, for `epic:concurrent-and-standing-case-trees`:
`story:child-case-own-workspace` changes `crates/loom-tree` and `ess/domains/tree.yaml`;
`story:run-caps-per-arrival`, `story:standing-tree-wakes-at-a-host-instant` and
`story:tree-usage-and-model-budget` change `crates/loom-tree`, `ess/domains/tree.yaml`,
`ess/domains/runlog.yaml` and `crates/loom-cli/src/lib.rs`; `story:standing-tree-rests-quiescent`
changes `crates/loom-tree`, `ess/domains/tree.yaml` and `lib.rs`. And for
`story:agent-selects-the-connectors-relay` and `story:secrets-subcommand`, which change
`crates/loom-cli/src/lib.rs` and `main.rs`. `story:budget-and-limit-stops-raise-decision-requests` and
`story:run-log-references-exec-file-changes` come after this story through their edges.

## ESS first

`ess/domains/runlog.yaml`: `RunLogEvent` gains the decision events above.
`ess/domains/tree.yaml`: `TreeRun` gains the decision settings, `NextDecision` gains the request
id, `TreeStatus` gains the open requests per case, and `Stuck` gains the ending request.
`CaseApprovalWriter` adds no ESS line: it is Rust and writes the `CaseApproval` that
`ess/domains/tree.yaml` already declares. On that commit `task drift` fails,
`crates/loom-tree/tests/tree_decisions.rs` does not compile, and
`crates/loom-cli/tests/tree_decisions.rs` fails (`decisions` has no `--tree`).

## Acceptance

In `crates/loom-tree/tests/tree_decisions.rs`:

- `CaseApprovalWriter` writing one Decided approval request's record twice leaves one
  `CaseApproval` with record id `<capability>@r<N>`, the request's principal and case revision
  `r<N>`.

In `crates/loom-cli/tests/tree_decisions.rs`, which runs the `b10x-loom` binary over the
three-level fixture tree of `story:tree-run-command` (`crates/loom-cli/tests/fixtures/tree/`),
the fixture autonomy documents and principals this story adds under
`crates/loom-cli/tests/fixtures/tree/decisions/`, a recorded decider, no rule and no model call:

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
- A granted approval at case revision `r<N>` whose case then moves (`spec` revised, the case
  revision raised to `r<N+1>`): the next `tree run` opens a request with record key
  `<capability>@r<N+1>`, writes no `superseded` line, exits 3, and the old request stays Decided
  with its record.
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
- A fixture writer with a `handoff`, admissible when nothing is useful, stops `tree run` with
  exit 3 and the `handoff` text, and opens no decision request.
- After a decided approval, the case's `canon-decision/1` document in the store shows the
  capability granted.
- A child's wait names the child's case and the request is in the child's log.
- `tree status` lists the open request of the waiting case and none after it is decided.
- `decisions decide DR-… A --as timo --tree STORE` on a store no daemon serves, with `STORE`
  typed as a relative path, prints the last two lines `unblocked: <case> (DR-…)` and
  `continue: b10x-loom tree run STORE`, with `STORE` exactly as typed.
- With the store's one-writer lock held by the test through the lock `story:tree-run-command`
  takes, `decisions decide DR-… A --as timo --tree STORE` exits 37. Standard error names
  `StoreInUse`, and the request and the case's log are byte-identical.
- `decisions list --principal timo --tree STORE --state DIR` exits 2 and changes nothing.
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
(`story:non-independent-review-becomes-decision-request`). A flag that supplies a rule. A served
tree: a decision through the daemon, and the daemon continuing the decided case
(`story:daemon-continues-a-decided-request`). Until that story lands, `decisions … --tree` on a
served store is refused `StoreServed`.

## Scope

Derived 2026-11-21 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-tree`, a decision module plus `crates/loom-tree/tests/tree_decisions.rs` — cited (story § Outcome last paragraph, § ESS first). Planned by `story:tree-run-command`, absent at HEAD.
- **Files:**
  - `crates/loom-cli/src/decisions.rs` (`--tree`, writers, the `unblocked:`/`continue:` lines) — cited (§ Outcome last paragraph). Planned by `story:decision-inbox-commands`, absent at HEAD.
  - `crates/loom-cli/src/lib.rs`, `crates/loom-cli/src/main.rs` (flags, exit 37 in the `decisions` help, `tree open` decision flags) — cited (§ Waits on); both exist at HEAD.
  - `ess/domains/runlog.yaml` (decision events) — cited (§ ESS first). Planned by `story:hash-chained-run-log`, absent at HEAD.
  - `ess/domains/tree.yaml` (`TreeRun`, `NextDecision`, `TreeStatus`, `Stuck`) — cited (§ ESS first). Planned by `story:tree-run-command`, absent at HEAD.
  - `docs/contracts/run-log-v1.json` (regenerated) — cited (§ Acceptance). Planned by `story:hash-chained-run-log`, absent at HEAD.
- **Symbols:** `CaseApproval`, `AuthorityRecords`, `HeldCaseGrounds`, `GroundRoots`, `ExplicitDecisionWriter`, `CaseApprovalWriter`, `complete_pending`, `CaseRevisions`, `ExpiryInstant`, `RunEnds`, `StoreInUse`, `StoreServed` — cited (story text). None is in the code at HEAD.
- **Also likely:**
  - `crates/loom-cli/src/tree.rs`, the `tree status` and exit-4 output — inferred. `story:tree-run-command` plans it, itself inferred.
  - `crates/loom-cli/tests/` for the `decisions --tree` and exit-37 bullets — inferred. The story names no CLI test file (see defect 3).
  - `crates/loom-cli/tests/fixtures/tree/` extended with autonomy and classes documents — inferred.
  - `crates/loom-runlog` and `crates/loom-tree/Cargo.toml` — inferred. The decision lines are written through the run-log crate.
  - The generated `website/docs/reference/cli.md` and `generated/rust/loom/` — inferred, house rule.
- **Documents:** none required (a guide page is not in the acceptance) — cited (§ Acceptance)
- **Confidence:** medium — the story names its surfaces, but every one except `lib.rs` and `main.rs` is planned and not in the code, and the CLI test file is unnamed
- **Would collide with:**
  - Ordered before it by `depends_on` (a path orders them): `tree-run-command`, `hash-chained-run-log`, `agent-node-run-records` (`runlog.yaml`, `tree.yaml`), `approvals-supplied-to-canon-evaluation`, `question-and-outcome-decisions-hold-runs`, `decisions-resume-the-held-run`, `decision-inbox-commands`, `silence-and-expiry-at-a-host-tick`, `approval-decisions-resume-held-runs`, `bound-commands-run-confined`, `confinement-opens-from-engine-config`, `decision-suspension-reason`. All touch `lib.rs`, `main.rs`, `decisions.rs`, `runlog.yaml`, `tree.yaml` or `crates/loom-tree`.
  - Ordered after it (a path orders them): `daemon-continues-a-decided-request`, `non-independent-review-becomes-decision-request`, `goals-entries-from-a-resolved-document`, `goals-grounds-and-free-answers`, `flow-limits-recorded-as-events`, `concurrent-cases-in-the-tree-daemon`, `capacity-given-to-an-admission-program`, `budget-and-limit-stops-raise-decision-requests`, `run-log-references-exec-file-changes`.
  - **No `depends_on` path, so a real conflict:**
    - `child-case-own-workspace`: `crates/loom-tree`, `tree.yaml`
    - `run-caps-per-arrival`: `crates/loom-tree`, `tree.yaml`, `runlog.yaml`, `lib.rs`
    - `standing-tree-rests-quiescent`: `crates/loom-tree`, `tree.yaml`, `lib.rs`
    - `standing-tree-wakes-at-a-host-instant`: `crates/loom-tree`, `tree.yaml`, `runlog.yaml`, `lib.rs`, `tree.rs`
    - `tree-usage-and-model-budget`: `crates/loom-tree`, `tree.yaml`, `runlog.yaml`, `run-log-v1.json`, `lib.rs`, `tree.rs`
    - `agent-selects-the-connectors-relay` and `secrets-subcommand`: `lib.rs`, `main.rs`
    - `mantle-placement`: `crates/loom-cli`, inferred
  - The story's "Shared with" stories have no scope entry at these paths in the store, so only its prose orders them: `limit-hits-in-run-log`, `engine-configuration-document`, `action-limits-enforced`, `tree-daemon`, `confinement-level-required`, `executable-allow-list`, `named-egress-per-action`, `confinement-level-on-oci-and-pod`, `ingress-routes-per-action`.
- **Safety fact:** exit 37 is free in `decisions`. The stories that use 30 to 36 are `decisions-resume-the-held-run` (30 to 35, 32 unused) and `daemon-continues-a-decided-request` (36, 37). Step 2, unproven.
