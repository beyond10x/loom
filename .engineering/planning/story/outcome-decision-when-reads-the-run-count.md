---
format: aep.planning-md/3
id: story:outcome-decision-when-reads-the-run-count
kind: story
status: proposed
title: 'An outcome''s decision counts only once a writer''s allowance is spent: a when reads the spent claim'
summary: 'requires: {decision: d, when: {claim: spent}} over the run-count claim; before the spend d is not pending, after it it is; an early decision counts after the spend; an evidence form in when is refused; split out (D131)'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:protocol-tree-on-bindings
- depends_on: story:outcome-when-reads-the-run-count
- depends_on: story:canon-0-11-0-pin
scope:
- confidence: inferred
  path: CHANGELOG.md
- confidence: inferred
  path: crates/loom-tree/tests/main.rs
- confidence: inferred
  path: website/docs/guides/run-a-case-tree.md
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T13:10:38Z", actor: "agent:loom", revision: 6}
---
## Outcome

An outcome's decision requirement can be conditioned on an action's spent allowance:
`requires: {decision: d, when: {claim: <spent claim>}}`. The decision counts only once the
writer is spent, and before that Canon neither counts it nor lists it as pending.

This builds on two stories:

- Canon 0.11.0 (`CANON-OUTCOME-003`; Canon `CHANGELOG.md` § 0.11.0, lines 69-76), which Loom
  reaches with `story:canon-0-11-0-pin`. A `when` holds claims and case inputs only, and an
  evidence form in it is refused `evidence-in-decision-condition`.
- The spent claim of `story:outcome-when-reads-the-run-count`, which a `when` can hold because
  it is a claim and not an evidence form.

This was split from that story by decision D131. It adds no Loom code beyond what the two
stories bring. It proves that the run-count record, the claim and the condition meet in a case
that `tree run` drove.

### What the case shows

The root case's Canon decision document (`CanonGovernor::decision_document`, which `tree run`
reads at `crates/loom-tree/src/run.rs:755-764`) shows:

- **Before the writer is spent**, the `when` is not true. The outcome is blocked by the
  condition's reasons, and no decision `d` is under `pending_decisions`.
- **Once the spent record arrives**, the `when` is true, and `d` is listed under
  `pending_decisions`.
- **A decision recorded early** is neither counted nor refused while `when` is not true (Canon's
  rule). It applies only at the case revision it was taken at (Canon
  `crates/canon/src/eval/decisions.rs:33-37` at `0.11.0`). Loom records it at the case's
  revision (`crates/loom-governor/src/lib.rs:1116-1121`), and the revision moves only when an
  artifact moves (`lib.rs:1784-1792`), not when a record arrives. So when the spending run
  changes no artifact, the early decision counts once the writer is spent, without being given
  again. When the spending run does change an artifact, it must be given again at the new
  revision.

Printing `d` as a wait of `tree run` is `story:tree-waits-open-decision-requests` (proposed).
This story does not need it.

## Decisions

- **D1. What the tests read.** Default **a**.
  - **a.** Canon's decision document of the root case, through a governor the test opens over the
    tree's store, as `crates/loom-tree/src/close.rs:58-67` opens one. That needs no story but
    the two depended on.
  - **b.** The decision request `tree run` prints. That adds a depends_on on
    `story:tree-waits-open-decision-requests`, which is proposed and larger.

## ESS first

No new noun. The decision requirement is Canon's.

## Acceptance

Bounds:

- No test runs longer than 60 s, and the test step ends within 120 s.
- No test makes a network request or a model call, and no test reads a credential file.
- Fixtures are POSIX sh, never Python.
- A slot job ends within 10 minutes, or names its time with `slot --timeout` (at most 30
  minutes).

All tests go in the `runs_spent` module of `crates/loom-tree/tests/main.rs`, on the fixture of
`story:outcome-when-reads-the-run-count`'s test 4. In that fixture, `work.do` has `max_runs: 2`
and `do.sh` exits 1, and `given_up` becomes
`{requires: {decision: give_up, when: {claim: work.spent}}}`.

1. `a_when_reads_the_spent_claim`.
   - A first `run_tree` with `max_steps: Some(1)` takes `work.do` once. The root's decision
     document then has no `give_up` under `pending_decisions`, and `given_up` is blocked by
     `work.spent`.
   - A second `run_tree` takes `work.do` again, and the spent record is written. The document
     then lists `give_up` under `pending_decisions`.
   - Failing first: before `story:canon-0-11-0-pin`, the protocol is refused at registration
     (`when` is an unknown field).
2. `an_early_decision_counts_once_the_writer_is_spent`.
   - The fixture's `do.sh` exits 1 and changes no file, so no step moves the case revision.
   - After the first run, the test records `give_up` with `CanonGovernor::decide`
     (`crates/loom-governor/src/lib.rs:1101`). The document still lists no pending `give_up`, and
     `given_up` is not legitimate.
   - The second `run_tree` takes `work.do` again and writes the spent record at the same case
     revision. With the early decision held, `given_up` is then the one legitimate outcome, and
     that same run ends `Terminal` with `given_up` (`crates/loom-tree/src/run.rs:456-475`),
     decided by the decision recorded before the spend. No decision is given again.
   - Failing first: as test 1.
3. `an_evidence_form_in_when_is_refused`.
   - A protocol whose `when` holds `{evidence: {kind: spent, subject: work, result: spent}}` is
     refused at registration, naming `evidence-in-decision-condition`.
   - This guards the claim form the story relies on.
   - Failing first: as test 1, refused for `when` itself.

**Gate.** `task check` exits 0.

## Out of scope

- The spent claim and its record (`story:outcome-when-reads-the-run-count`).
- The Canon move (`story:canon-0-11-0-pin`).
- Printing the conditioned decision as a wait (`story:tree-waits-open-decision-requests`).
- Runbook's protocol change to use `when`.

## Scope

Derived 2026-11-22 from the plan tree at `763c6ecf`; Canon was read at tag `0.11.0`, read-only.
**Cited** means read at the line named; **inferred** means a reading that could be wrong.

- `crates/loom-tree/tests/main.rs` (the `runs_spent` module) — inferred: the module is created by
  `story:outcome-when-reads-the-run-count`.
- `crates/loom-tree/src/close.rs` (`:58-67`) and `crates/loom-governor/src/lib.rs` (`:1101`),
  read and not changed — cited.
- `website/docs/guides/run-a-case-tree.md` (a paragraph on conditioned decisions) and
  `CHANGELOG.md` — inferred.

## Edges

- `depends_on story:outcome-when-reads-the-run-count`: the spent claim and its fixture.
- `depends_on story:canon-0-11-0-pin`: Canon 0.11.0, which reads `when`.
- **Order:** after both; it is the last of this set.
- **Would collide with**, no edge:
  - `story:outcome-when-reads-the-run-count` on the `runs_spent` module of
    `crates/loom-tree/tests/main.rs`. It merges first.
  - `story:tree-waits-open-decision-requests` (proposed) on the guide's section on decisions.
