---
format: aep.planning-md/3
id: story:governor-keeps-a-terminated-case-terminated
kind: story
status: proposed
title: A terminated case answers its stored outcome and a blocked frontier at any later time, and refuses writes
summary: 'termination judged once in terminate; snapshot passes none to Canon; completion stored outcome; writes refused Terminated (D1 a: update_revision, record_step, decide, receive); open_children window out of scope; security-relevant'
tags:
- sdc
relations:
- serves: vision:O1
- decomposes: epic:decision-requests-in-the-runtime
- depends_on: story:child-case-runtime
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-governor/src/children.rs
- confidence: cited
  path: crates/loom-governor/src/lib.rs
- confidence: cited
  path: crates/loom-governor/tests/child_cases.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T15:08:36Z", actor: "agent:loom", revision: 8}
---
## Outcome

A terminated case stays terminated. Cites are at `23f1f4b3`.

- The governor judges the termination once, in `terminate`.
- From then on:
  - `completion` reports the stored outcome;
  - `frontier` blocks every action;
  - neither asks Canon to judge the termination again.
- A public write to a terminated case is refused with a named error.

**Today.** The contract in `AGENTS.md` § Governor (`:766-768`) reads: "A terminated case is
evaluated with its termination, its frontier blocks every action ... and `completion` reports
the stored outcome." The code holds that only as long as Canon still judges the termination
legitimate:

- `snapshot` hands Canon the stored termination (`crates/loom-governor/src/lib.rs:2083-2087`, "A
  terminated case is evaluated with its termination, which Canon judges again").
- `decided_at` runs `decide_case` at every evaluation with the records and the trusted time of
  that moment (`:1383-1408`).
- When Canon no longer judges the termination legitimate, `decide_case` fails, and both of these
  answer `GovernorUnavailable` instead of the stored outcome:
  - `frontier` (`:1437-1484`);
  - `completion` (`:1489-1514`).
- Two things make that happen (`crates/loom-governor/src/children.rs:652-661`, "Limitation"):
  - the trusted time passes the `max_age` of a record the termination rests on;
  - a later write changes what Canon sees. `update_revision` (`lib.rs:1069-1072` says so),
    `record_step` (`:1715`), `decide` (`:1108`) and `receive` (`:1579`) do not refuse a
    terminated case.
- No caller in this repository combines a trusted time with `terminate`: `tree run` and
  `tree close` build the governor without one (`children.rs:660-661`). A library host would.

**The review's two red cases** (w7 batch, security review lane 4), cited by content:

- A protocol `review.sec.closed@1` with one artifact `work`, a kind `verdict` with
  `max_age: 60s`, a claim `work.passed` over a passed verdict about `work`, and one outcome
  `passed` requiring it.
- A case is opened with `work` at `w1`. A trusted observation and a passed verdict are received
  at `12:00:00Z`, and the case is terminated through `passed` at `12:00:10Z`, under a trusted
  time held in a cell.
- Case 1: the cell moves to `13:00:10Z`. The store still holds the termination, but `completion`
  and `frontier` answer `GovernorUnavailable`.
- Case 2: `update_revision(case, "work", "w2")` succeeds, and then `completion` and `frontier`
  answer `GovernorUnavailable`.

**After, under D1 a.**

- **Judged once.**
  - For a terminated case, `snapshot` hands Canon no termination, so `decide_case` evaluates the
    case as an open one.
  - `frontier` then lists Canon's claims and obligations as they are now, and blocks every
    action through `children::after_termination` (`children.rs:181-194`, reason
    `{"terminated":"<outcome>"}`), as today.
  - `completion` returns the stored outcome before any Canon call.
  - Canon's judgement of the termination stays in `terminate` (`children.rs:690-700`, `judge`),
    unchanged.
- **Writes refused**, each with a new variant `Terminated { outcome }`, naming the stored
  outcome, and nothing written:
  - `update_revision` (`UpdateError`, `lib.rs:636-645`);
  - `record_step` (`StepError`, `:1641`);
  - `decide` (`DecisionError`, `:669-677`);
  - `receive`. Commission's `GovernorError` has only `UnknownCase` and `GovernorUnavailable`,
    so `receive` answers `GovernorUnavailable` and its doc comment names the termination.
- **The governor's own writes stay.**
  - `terminate` through the same outcome still completes an interrupted replacement of the
    parent's `case_outcome` record (`AGENTS.md:765-766`).
  - That write is to the parent, and goes through the store, not through a public write method.
- **Documentation.** The "Limitation" section (`children.rs:652-661`) and the comments at
  `lib.rs:121-124`, `:1069-1072`, `:1434-1436` and `:1486-1488` are rewritten to the new rule.
  `AGENTS.md:766-768` keeps its sentence, which is now true without a condition.

Security-relevant: yes. A host that combines a trusted clock with `terminate` could see a closed
case answer `GovernorUnavailable` and treat it as open. A writer could also move a terminated
case's artifacts. The unit gets a security review.

## Decisions

- **D1. Which writes refuse a terminated case.** Default **a**.
  - **a.** Every public write: `update_revision`, `record_step`, `decide`, and `receive` for a
    record of the case. Once the termination is judged once, no write can change what the case
    answers, so the refusal is what keeps the store honest.
  - **b.** Only `update_revision` and `record_step`, the two that move revisions. `decide` and
    `receive` keep today's acceptance. A record received after the end is kept and never read
    for the outcome.

## ESS first

No noun changes. `loom.governor.CaseTermination` (`ess/domains/governor.yaml:174`) already holds
the outcome. The error enums are Rust API, not declared in `ess/`.

## Acceptance

Bounds: no test over 60 s, the test step within 120 s, no network, no model call, no clock call
(the trusted time is a cell the test sets). No fixture program.

**`crates/loom-governor/tests/child_cases.rs`** (an existing binary; this adds no new one). The
fixture is the review's: `review.sec.closed@1`, `terminated_case(cell)` and `still_terminated`,
as described above, held as `const` texts.

1. `a_terminated_case_stays_complete_after_its_evidence_ages_out`.
   - The review's case 1. After the cell moves past `max_age`, `completion` is `Complete` with
     `passed`, and every frontier action is `Blocked` with `{"terminated":"passed"}`.
   - Failing first: both answer `GovernorUnavailable`.
2. `a_revision_move_on_a_terminated_case_is_refused`.
   - The review's case 2, with D1's answer: `update_revision` returns `Terminated { outcome:
     "passed" }`.
   - The case's revisions and its revision number are as before, and `still_terminated` holds.
   - Failing first: the move succeeds and the case answers `GovernorUnavailable`.
3. `a_step_a_decision_and_a_record_on_a_terminated_case_are_refused`.
   - `record_step` → `StepError::Terminated`, and `decide` → `DecisionError::Terminated`.
   - A record submitted through the evidence port → `GovernorUnavailable`.
   - Each leaves the stored case byte-identical (its `canon-case/1` document and its records).
   - Failing first: each write succeeds.
4. `a_restart_between_the_child_terminated_and_the_outcome_written_is_completed_once`
   (`child_cases.rs:764-782`), an existing test, passes unchanged.
   - It makes the parent's record stale before the second `terminate`. A `Faulty` store fails
     the parent's next update (`fail_next_update_of`), so the first `terminate` stores the
     child's termination and leaves the parent's `case_outcome` record at `open`.
   - A governor over the same directory then terminates the child again through `passed`, and
     the parent's one record reads `passed`.
   - Failing first: none. It is a guard that the write refusal does not reach `terminate`'s
     own write to the parent. It catches the planted defect of refusing `terminate` on a case
     already terminated through the same outcome.

**Gate.** `task check` exits 0.

## Out of scope

- **The `open_children` window** (`children.rs:390-400`, documented by correction CE). The
  children are written first and the parent last, so a stop between them leaves a child whose
  parent does not name it.
  - Closing it needs a different change: the write order, or one write. It does not need the
    termination change here.
  - It is named for a story of its own.
- Canon's own rule for a termination (Canon judges it in `terminate`, as today).

## Scope

Derived 2026-11-22 from the plan tree at `23f1f4b3`. **Cited** means read at the line named;
**inferred** means a reading that could be wrong.

- `crates/loom-governor/src/lib.rs` (`:121-124`, `:636-677`, `:1069-1108`, `:1383-1408`,
  `:1434-1514`, `:1579`, `:1641`, `:1715`, `:2064-2087`) — cited.
- `crates/loom-governor/src/children.rs` (`:181-194`, `:390-400`, `:640-700`) — cited.
- `crates/loom-governor/tests/child_cases.rs` (`:382`, `:688-691`, `:764-782`) — cited.
- `AGENTS.md` (§ Governor, `:758-772`) and `CHANGELOG.md` — cited.
- `crates/loom-tree/src/close.rs`, `crates/loom-tree/src/run.rs`: callers of `terminate`, read
  and not changed — inferred.

## Edges

- `depends_on story:child-case-runtime` (active): `terminate`, `CaseTermination` and
  `after_termination` are its.
- `AGENTS.md` and `CHANGELOG.md` are shared with most stories. They are textual merges and are
  not listed below. In § Governor, `canon-0-11-0-pin` edits `:717-735` and this story the
  paragraph at `:758-772`.
- **Would collide with**, no edge. Each entry names the shared file and the merge order:
  - `crates/loom-governor/src/lib.rs`:
    - `canon-0-10-0-pin` and `canon-0-11-0-pin` (active) edit the `snapshot` literal
      (`:2064-2087`); the 0.11.0 pin adds `descends_from` to `CaseArtifact` in the same function.
      This story merges after the pin;
    - `outcome-when-reads-the-run-count` and `outcome-decision-when-reads-the-run-count`
      (active) edit `decide` (`:1108`). The new refusal comes before Canon's check there.
      Textual merge, no order;
    - active, other regions: `decision-request-file-store`, `record-copy-between-cases`,
      `bound-artifact-revisions`, `bound-program-contract`, `bound-program-retry-records`,
      `concurrent-confined-runs`, `engineering-protocols-dependency-removed`,
      `open-children-from-arguments-from` and `tree-run-command`. Textual merge, no order;
    - `silence-and-expiry-at-a-host-tick`, `standing-tree-rests-quiescent` and
      `standing-tree-wakes-at-a-host-instant` (proposed) bring the trusted time this story makes
      safe. They merge after this story;
    - proposed, other regions: `approval-decisions-resume-held-runs`,
      `approvals-supplied-to-canon-evaluation`, `child-case-own-workspace`,
      `decided-record-written-then-recorded`, `decision-inbox-commands`,
      `decision-routing-by-holder`, `ess-rules-of-the-runtime`,
      `gate-run-collects-a-gates-document`, `gates-document-evaluated-in-the-governor`,
      `goals-entries-from-a-resolved-document`, `grounds-resolve-against-held-cases`,
      `question-and-outcome-decisions-hold-runs`, `run-caps-per-arrival` and
      `tree-waits-open-decision-requests`. Textual merge, no order;
    - `requests-from-canon-pending-decisions` (draft) reads Canon's decision of a case, which
      for a terminated case is now taken without its termination. It merges after this story;
    - `selector-stop-names-set-aside-record` (draft), another region. Textual merge, no order.
  - `crates/loom-governor/src/children.rs`: `bound-program-contract`, `canon-0-11-0-pin`,
    `child-record-in-the-run-log` and `open-children-from-arguments-from` (active), and
    `ess-rules-of-the-runtime` (proposed). Textual merge, no order.
  - `crates/loom-governor/tests/child_cases.rs`: `one-test-binary-per-crate` (active) turns the
    file into a module of `tests/main.rs`. The file keeps its path and its test names. Textual
    merge, no order.
