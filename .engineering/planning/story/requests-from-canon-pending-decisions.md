---
format: aep.planning-md/3
id: story:requests-from-canon-pending-decisions
kind: story
status: draft
title: Loom builds on the Canon release with pending_decisions and builds every decision stop from it
summary: stop_from_pending builds every decision stop from Canon 0.10.0's pending_decisions
tags:
- sdc
relations:
- decomposes: epic:decision-requests-in-the-runtime
- serves: vision:O1
- depends_on: story:tree-waits-open-decision-requests
- depends_on: story:approvals-supplied-to-canon-evaluation
- depends_on: story:canon-0-10-0-pin
scope:
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/src/pending.rs
- confidence: cited
  path: crates/loom-governor/tests/pending_decisions.rs
- confidence: inferred
  path: crates/loom-intake-slice/src/decision.rs
- confidence: cited
  path: crates/loom-intake-slice/tests/decision_authority_rule.rs
- confidence: inferred
  path: crates/loom-tree
- confidence: cited
  path: crates/loom-tree/tests/tree_decisions.rs
- confidence: cited
  path: ess/domains/governor.yaml
revision: 33
---
## Outcome

On Canon 0.10.0, the release that carries `story:pending-decisions` and that
`story:canon-0-10-0-pin` moves Loom to, Loom builds every decision stop from Canon's
`pending_decisions` section of `canon-decision/1`.

The shape below was read at Canon `49ff2ce` (branch `wave/2026-11-20`), which no tag contains; it
is read again at the release tag before this story is implemented (§ Waits on).

- Canon's type is untyped: `Decision.pending_decisions: Option<Json>`, `Json` being
  `serde_json::Value` (`crates/canon/src/model/decision.rs:70`, `:15` at `49ff2ce`). Loom does not
  read it through a Canon type. It reads the section from the text of the document
  `CanonGovernor::decision_document` returns (`crates/loom-governor/src/lib.rs:1230`;
  `AGENTS.md` § Governor) into its own ESS types (§ ESS first), whose member names are Canon's at
  `49ff2ce`.
- `CanonGovernor::pending_decisions(case, frontier)` returns the section of the document behind a
  frontier id the governor issued. Like `decision_document`, it answers `None` for an id the
  governor does not keep. For a kept document with no section it answers `Some(None)`.
- Canon writes the section only when something is pending, and each of its two lists only when
  it has an entry (`crates/canon/src/eval/pending.rs:5-7`, `:53-62` at `49ff2ce`;
  `website/docs/reference/documents.md:80` at `49ff2ce`). So the section of a case with nothing
  pending is `None`, never an empty section.
- `authority` lists entries `{action, capability, scope?, current, case_revision?, options:
  [granted, denied]}`, one per requirement of an `approval-required` action whose decision is
  `none` or `require_approval` (`pending.rs:9-14`, `:79`). It is ordered by action id, then
  capability, then scope. No scope comes first. Two scopes are compared by their key-value pairs
  in key order, pair by pair, key before value, each by code point, and a prefix comes first
  (`pending.rs:15-19`, `:65-67`, `:107`).
- `decisions` lists entries `{outcome, decision, case_revision?, options}`, one per outcome
  blocked for want of an explicit decision, in outcome-id order (`pending.rs:20-24`, `:135`).
  `options` is `[taken]` with a case revision, and `[]` with no `case_revision` when the snapshot
  has no revision (`:123`, `:129-131`).
- Loom keeps both orders. Callers build and route stops in list order.

`stop_from_pending` builds stops from the entries. It lives in its own module of
`b10x-loom-governor`, `crates/loom-governor/src/pending.rs`, with one `mod` line in `lib.rs`. It
returns `DecisionStop` (`story:decision-routing-by-holder`), declared in `ess/domains/decision.yaml`
and generated into the `loom` crate. `crates/loom-governor/Cargo.toml:18` already names that crate,
so this story adds no manifest line for the type.

The function stays in the governor and not in `b10x-loom-decision` for two reasons:

- `b10x-loom-decision` names no Canon or Commission crate (`story:decision-request-entity`,
  Acceptance).
- The governor is the only crate that evaluates with Canon (`AGENTS.md` § Governor). Its kept
  documents are what `pending_decisions` reads.

The governor's path dependency on `b10x-loom-decision`, `b10x-loom-decision = { path =
"../loom-decision" }`, belongs to `story:decision-request-file-store`, which this story is ordered
after. This story's test uses `route_stop` and the in-memory store through it.

What `stop_from_pending` gives for each kind of entry:

- **`authority` entry:** an approval stop.
  - Its action is the entry's action, and it carries the entry's `capability` and `scope`.
  - Options are `A` granted and `B` denied, both producing a record.
  - The subject is given by the caller, because Canon names no subject artifact of an action. The
    caller maps the action to its subject through the bindings file (session `canon`,
    2026-11-21).
  - `current` is `none` or `require_approval` (`pending.rs:79`). Both build the same stop.
    `current` is carried on the stop only for the inbox text: the stop's question is the caller's
    question, as before this story, and for `require_approval` it ends with ` (current:
    require_approval)`. `route_stop` does not read `current`.
- **`decisions` entry with `options: [taken]`:** an outcome stop whose action is the outcome
  (`story:decision-routing-by-holder`). `A` take produces the explicit decision record, and `B`
  leave it untaken produces none (session `sdc`, 2026-11-21, answer d).
- **Entry with no `case_revision`:** refused by name, and no stop is built from it. This covers an
  `authority` entry without one and a `decisions` entry without one, which Canon writes with
  `options: []`. The refusal is `PendingStopRefusal::NoCaseRevision`, naming the list (`authority`
  or `decisions`) and the entry's action or outcome. It is declared in `ess/domains/governor.yaml`
  (§ ESS first). Without `case_revision` there is no `r<N>` for a `record_key`, and a `decisions`
  entry has no option for a request, which needs two. Loom always evaluates a case it holds with
  its revision `r<N>` (`crates/loom-governor/src/lib.rs:1770`), so only a document Loom did not
  evaluate so, such as one `evaluate` returns for a caller's snapshot without `revision`, has such
  an entry.

Canon lists each requirement once, by capability and scope (`crates/canon/src/eval/actions.rs:128-129`
at `49ff2ce`, a `BTreeSet`), and keys an entry by action, capability and scope (`pending.rs:65-67`).
So two requirements that name one capability give two entries only when their scopes differ.

A grant covers a requirement only when every scope key the grant names has the same value in the
requirement, and coverage never widens inwards (`actions.rs:59-64`). So two scopes are two
pending decisions.

Each entry is one stop, and each stop one request. The router keys requests by `record_key` and
keeps `question_key` `<case>|<action>|<question id or empty>` (`story:decision-routing-by-holder`).
The `record_key` of an approval stop is built by program from the entry's fields, in this order:
the capability, then the scope's pairs in key order when the entry has a scope, then `r<N>`.
`story:decision-routing-by-holder` states the form once, and this story reads that sentence. In
short:

- `<capability>@r<N>` with no scope, as before.
- `<capability>[<key>=<value>,…]@r<N>` with a scope, keys compared by code point.
- In the capability, each key and each value, the characters `%`, `[`, `]`, `=`, `,` and `@` are
  written `%25`, `%5B`, `%5D`, `%3D`, `%2C` and `%40`.
- An empty scope is no scope.

The consequences for requests:

- Two entries of one action with two capabilities give two requests under one `question_key`.
- Two entries of one capability with two scopes give two requests under one `question_key`.
- Routing an entry again at its revision finds the open request for its `record_key` and reuses it.
- A `require_approval` entry whose `record_key` already has a request is decided by the router's
  reuse rule (`story:decision-routing-by-holder`, Outcome, the two bullets after "Store lookups"):
  - An Open or Escalated request is reused, and no request is opened.
  - A Decided request that is not superseded is answered `Decided`, and no request is opened.
  - Canon's precedence can leave such an entry pending after a grant was recorded: a `granted`
    entry decides a `require_approval` requirement only when no `require_approval` entry still
    covers it (`pending.rs:29-30`; `actions.rs:64-65`).

A caller that looks a request up by action and capability looks it up by the stop's `record_key`.
`DecisionAuthority` (`story:approval-decisions-resume-held-runs`) is one.

Today's writers keep no scope. `GrantRecordWriter` (`story:approval-decisions-resume-held-runs`)
writes `intake.routing.Grant`, and `CaseApprovalWriter` (`story:tree-waits-open-decision-requests`)
writes the tree's `CaseApproval`. An unscoped grant would cover every scope of its capability
(`actions.rs:60-62`) and widen the authority asked for (`AGENTS.md` § Rules). So until a story
gives the writers and `AuthorityRecords` a scope, both writers refuse a `record_key` with a scope
part `WriterFailed`, naming it (`story:decided-record-written-then-recorded`), and write nothing.

Three callers build their stops with `stop_from_pending`:

- `DecisionHoldingExecutor` (`story:question-and-outcome-decisions-hold-runs`).
- `DecisionAuthority` (`story:approval-decisions-resume-held-runs`).
- `tree run` (`story:tree-waits-open-decision-requests`).

The interim reading of a blocked outcome's reason `{"decision": <name>, "present": false}` that
those stories add is removed.

## Why

§ Ownership gives the pending decision, "a fact derived from a case", to Canon
(`~/beyond10x/sdc/docs/decisions.md:286-288` at `082c863`). Until Canon ships it, this epic derives
stops from Loom's own stop and the decision document. This story replaces that derivation with
Canon's, so Loom and Canon cannot disagree about what is pending.

Loom pins Canon 0.10.0 (`story:canon-0-10-0-pin`), which carries `pending_decisions`
(`crates/canon/src/model/decision.rs:70` at `0.10.0`).

## Waits on

- `story:canon-0-10-0-pin`: it moves every manifest naming `b10x-canon` to tag `0.10.0`, which
  carries `pending_decisions`. This story moves no manifest.
  - The shape this story states was read at Canon `49ff2ce`, an ancestor of tag `0.10.0`.
  - Read again at `0.10.0` (`git -C ~/beyond10x/canon diff 49ff2ce 0.10.0`): the `authority`
    entries are unchanged; a `decisions` entry is also written for a reason that carries
    `stale_grounds` beside `decision` and `present: false`, and the entry does not copy it
    (`crates/canon/src/eval/pending.rs:111-128` at `0.10.0`). Lines of `pending.rs` after `:107`
    move by three and lines of `crates/canon/src/eval/actions.rs` move; the body's line numbers
    are those of `49ff2ce`, and the implementor cites `0.10.0`.
- `story:tree-waits-open-decision-requests`: one of the three callers, and both change
  `crates/loom-tree`. The slice callers come before it in the epic's order.
- `story:approvals-supplied-to-canon-evaluation`: both change `crates/loom-governor/src/lib.rs`
  and `ess/domains/governor.yaml`. That story is not held by Canon, so it goes first.

## ESS first

`ess/domains/governor.yaml` gains the section as Loom reads it, with Canon's member names at
`49ff2ce`:

- `PendingCurrent`: `none` or `require_approval`.
- `PendingAuthority`: `action`, `capability`, optional `scope` (a map of key to value),
  `current`, optional `case_revision`, `options`.
- `PendingExplicitDecision`: `outcome`, `decision`, optional `case_revision`, `options`.
- `PendingDecisions`: optional `authority` and optional `decisions`, lists of the two.
- The refusal `PendingStopRefusal`, a union with one variant, `NoCaseRevision` (list, entry).

On that commit `task drift` fails, and `crates/loom-governor/tests/pending_decisions.rs` does not
compile.

## Acceptance

The bullets about requests run in `crates/loom-governor/tests/pending_decisions.rs`, which routes
stops with `route_stop` over the in-memory store at a level whose holder is `person`. Each uses a
fixture protocol and case at `r3`, unless the bullet names another fixture, and no model call.

- For a case with one approval-required action and one outcome blocked on a missing decision,
  `pending_decisions` has one entry in each list, and `stop_from_pending` gives:
  - an approval stop with options `granted` and `denied`, both producing a record;
  - an outcome stop with options take, producing a record, and leave untaken, producing none.
- For a case with nothing pending, `pending_decisions` answers `Some(None)`, and the document
  `decision_document` returns has no `pending_decisions` member.
- `pending_decisions` with a frontier id the governor did not issue answers `None`.
- An action `merge` with the requirements `release.approve`, `change.merge {cluster: b}`,
  `change.merge` and `change.merge {cluster: a}` gives `authority` entries in the order
  `change.merge`, `change.merge {cluster: a}`, `change.merge {cluster: b}`, `release.approve`.
- An action `merge` that requires `change.merge` and `release.approve` gives two `authority`
  entries and two requests with the record keys `change.merge@r3` and `release.approve@r3`, both
  under the `question_key` `<case>|merge|`.
- An action `merge` that names `change.merge` twice with no scope gives one `authority` entry and
  one request, `change.merge@r3`.
- An action `merge` that requires `change.merge` at scope `{namespace: foo, cluster: prod-eu}`
  gives one entry carrying that scope, and one request with the record key
  `change.merge[cluster=prod-eu,namespace=foo]@r3`.
- An action `merge` that requires `change.merge` at scopes `{cluster: a}` and `{cluster: b}` gives
  two entries and two requests, `change.merge[cluster=a]@r3` and `change.merge[cluster=b]@r3`, both
  under the `question_key` `<case>|merge|`. The store holds two requests.
- Two `authority` entries equal but for `current` (`none`, `require_approval`) give two stops that
  are equal in every member but the question. The second question ends with ` (current:
  require_approval)`.
- With a Decided request for `change.merge@r3` in the store, a `require_approval` entry for
  `change.merge` at `r3` routes to `Decided` with that request. The store is unchanged.
- An `authority` entry read from the `canon` document that `evaluate` returns for a snapshot
  without `revision` is refused `NoCaseRevision`, naming `authority` and `merge`. No stop is built.
- A `decisions` entry from the same document, with `options: []`, is refused `NoCaseRevision`,
  naming `decisions` and the outcome `close`. No stop is built.
- `GrantRecordWriter`, given a request Decided `A` whose `record_key` is
  `change.merge[cluster=prod-eu]@r3`, answers `WriterFailed` naming it. The slice's grant store is
  unchanged (`crates/loom-intake-slice/tests/decision_authority_rule.rs`).
- `CaseApprovalWriter`, given the same request, answers `WriterFailed` naming it. The tree's store
  holds no `CaseApproval` (`crates/loom-tree/tests/tree_decisions.rs`).
- The slice and tree runs of the three caller stories give the same requests as before this story,
  by id, question, options and subject. Their tests pass with their request assertions unchanged:
  - `story:approval-decisions-resume-held-runs`: `crates/loom-cli/tests/approval_decisions.rs`.
  - `story:question-and-outcome-decisions-hold-runs`:
    `crates/loom-intake-slice/tests/decision_hold.rs` and
    `crates/loom-cli/tests/question_outcome_decisions.rs`.
  - `story:tree-waits-open-decision-requests`: `crates/loom-cli/tests/tree_decisions.rs`.
- `rg -l '"present"' crates/*/src` lists no file outside `crates/loom-governor/src`. The check can
  fail before this story: `story:question-and-outcome-decisions-hold-runs` and
  `story:tree-waits-open-decision-requests` add the interim reading, and both are ordered before
  this story by `depends_on`. It is checked once they have landed.
- `task check` exits 0.

## Out of scope

- Grounds on decided records (`story:decided-records-carry-grounds-to-canon`).
- Supplying approvals to Canon (`story:approvals-supplied-to-canon-evaluation`).
- Mapping an action to its subject in Canon.
- Writing a scoped approval's record with its scope, and supplying that scope to Canon. Neither
  `intake.routing.Grant`, the tree's `CaseApproval` nor `AuthorityRecords` has a scope.

## Scope

Derived 2026-11-21 by `story-scoper`, corrected by the reviser where the body moved a file. Every
line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/loom-governor` (`stop_from_pending`, `CanonGovernor::pending_decisions`, `ess/domains/governor.yaml`) - cited, story Outcome and "ESS first"
- **Files (governor):** `crates/loom-governor/src/lib.rs` (`decision_document` :1230, `decide_case` :2064; `pending_decisions` goes beside the first; one `mod` line) - cited
- **Files (governor, new):** `crates/loom-governor/tests/pending_decisions.rs` - cited ("ESS first": does not compile on the spec commit)
- **Files (governor, new module):** `crates/loom-governor/src/pending.rs`, the home of `stop_from_pending` - cited (Outcome)
- **Files (ESS):** `ess/domains/governor.yaml` (gains `PendingCurrent`, `PendingAuthority`, `PendingExplicitDecision`, `PendingDecisions`, `PendingStopRefusal`) - cited
- **Files (generated):** `generated/rust/loom/src/governor.rs`, `website/docs/reference/ess/loom-governor.md`, `website/data/ess/loom-governor.domain-graph.json` - inferred, regenerated from `ess/domains/governor.yaml` (`AGENTS.md` § Generated files)
- **Files (pin):** none. `story:canon-0-10-0-pin` moves the manifests, `Cargo.lock` and
  the pin documents - cited (§ Waits on)
- **Files (callers):** the slice's decision module (`DecisionAuthority`, `GrantRecordWriter`; `crates/loom-intake-slice/src/decision.rs`, named by `story:approval-decisions-resume-held-runs`) and the hold module of `DecisionHoldingExecutor` (name unknown, `story:question-and-outcome-decisions-hold-runs`); the decision module of `crates/loom-tree` (`CaseApprovalWriter`, `story:tree-waits-open-decision-requests`) - cited that all three change, location inferred, none exists in the tree (`crates/loom-tree` is absent at `8f70f7b2`)
- **Files (caller tests):** `crates/loom-intake-slice/tests/decision_authority_rule.rs`, `crates/loom-tree/tests/tree_decisions.rs` (the scoped `WriterFailed` bullets); `crates/loom-cli/tests/{approval_decisions,question_outcome_decisions,tree_decisions}.rs` and `crates/loom-intake-slice/tests/decision_hold.rs` (run, unchanged in their request assertions) - cited (Acceptance), none exists at `8f70f7b2`
- **Stop type:** `DecisionStop` of `ess/domains/decision.yaml` (`story:decision-routing-by-holder`), generated into the `loom` crate that `crates/loom-governor/Cargo.toml:18` names; no manifest line for it. The governor's `b10x-loom-decision` path dependency is `story:decision-request-file-store`'s - cited (Outcome)
- **Symbols:** `CanonGovernor::pending_decisions`, `stop_from_pending`, `PendingCurrent`, `PendingAuthority`, `PendingExplicitDecision`, `PendingDecisions`, `PendingStopRefusal::NoCaseRevision`, `DecisionHoldingExecutor`, `DecisionAuthority`, `GrantRecordWriter`, `CaseApprovalWriter` - cited; none exists in the tree
- **Canon type Loom reads through:** none typed. `Decision.pending_decisions: Option<Json>` (`crates/canon/src/model/decision.rs:70` at `49ff2ce`); Loom parses `decision_document`'s text - cited
- **Documents:** `AGENTS.md` § Governor (house rule, not listed in paths) - cited
- **Confidence:** medium - the governor files, the ESS file and the test file are named; every caller module is planned by an undelivered story
- **Would collide with** (by file; `T` = this story; ordered = a `depends_on` path exists, from `aep plan artifact graph --format json`):
  - `crates/loom-governor/src/lib.rs` and `ess/domains/governor.yaml`: `story:approvals-supplied-to-canon-evaluation` (direct `depends_on`), `story:grounds-resolve-against-held-cases`, `story:decision-request-file-store`, `story:bound-artifact-revisions`, `story:child-case-runtime`, `story:record-copy-between-cases` (all ordered before T by path); `story:decided-record-written-then-recorded`, `story:silence-and-expiry-at-a-host-tick`, `story:decision-inbox-commands`, `story:question-and-outcome-decisions-hold-runs`, `story:bound-program-result-table` (lib.rs only; ordered before T). `story:hash-chained-run-log` is delivered and is not a collision (ordered).
  - `crates/loom-governor/src/lib.rs` or `ess/domains/governor.yaml`, NO path either way: `story:child-case-own-workspace`, `story:goals-entries-from-a-resolved-document`, `story:run-caps-per-arrival`, `story:standing-tree-rests-quiescent`, `story:standing-tree-wakes-at-a-host-instant`, `story:gates-document-evaluated-in-the-governor`, `story:gate-run-collects-a-gates-document`, `story:frontier-names-movers`, `story:engineering-registry-calls-replaced`, `story:engineering-protocols-dependency-removed` (the last two also the engineering manifests and `crates/loom-protocols/Cargo.toml`). A coordinator decision.
  - Slice caller files (`crates/loom-intake-slice/src/run.rs` and the new decision module): `story:approval-decisions-resume-held-runs`, `story:question-and-outcome-decisions-hold-runs`, `story:tree-run-command`, `story:bound-commands-run-confined` (ordered before T); `story:protocol-argument-schema` (NO path).
  - `crates/loom-tree`: `story:tree-run-command`, `story:agent-node-run-records`, `story:tree-waits-open-decision-requests` (ordered before T); `story:capacity-given-to-an-admission-program`, `story:concurrent-cases-in-the-tree-daemon`, `story:daemon-continues-a-decided-request`, `story:flow-limits-recorded-as-events`, `story:non-independent-review-becomes-decision-request`, `story:tree-usage-and-model-budget`, `story:run-caps-per-arrival` (NO path).
  - After T: `story:decided-records-carry-grounds-to-canon` (direct `depends_on` T); `story:authority-records-bound-and-grounded-in-canon` (through it).
  - `story:decision-suspension-reason` (active, other tree): `crates/loom-governor/src/file_suspension.rs`, not a file T edits; no collision.
- **Safety fact:** `story:canon-0-10-0-pin`'s move changes no caller's parse because
  Canon writes `pending_decisions` only when something is pending (`pending.rs:62`,
  unchanged from `49ff2ce` to `0.10.0`), so a `decision_document` consumer that ignores
  it sees the old shape - inferred, not run
- **Contradicted by the code:** nothing; the pin is `story:canon-0-10-0-pin`'s
