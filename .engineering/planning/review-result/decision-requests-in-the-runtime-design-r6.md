---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r6
kind: review-result
status: active
title: Plan critic design, round 6, on story:decisions-resume-the-held-run, story:daemon-continues-a-decided-request and story:goals-grounds-and-free-answers of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:decisions-resume-the-held-run
- reviews: story:daemon-continues-a-decided-request
- reviews: story:goals-grounds-and-free-answers
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

story:decisions-resume-the-held-run — Ledger § Open 4 does not hold in code, so the outcome, question, 31 and 32 command-line bullets and the library `Resumed` bullet rest on a rebuild that no story writes. The story must either take on rebuilding a non-`software-change@1` commission from its context (put `intent.rs` in Files, keep the execution kind or bindings in the context, have `ResumeBuilder` supply the executor) or move those bullets to a library test that calls `resume_decision_hold`. Evidence: `resume` rebuilds through `resumed_request` (crates/loom-cli/src/main.rs:681). That function refuses a case whose inputs are not a software change's (crates/loom-intake-slice/src/intent.rs:624-625), and the fixture protocol's cases open with `CaseInputs::none()` (:902, :929). It also needs a test program (:599-602) and sets `bindings: None` (:628-629). The context keeps no bindings (:774-802), yet `run_prepared` runs any other protocol only through bindings or the clock path (:891, :922-923). Separately, a `ResumeBuilder` that builds only a model and a test runner cannot inject the scripted inner executor its library bullets need. Cites: .engineering/planning/story/decisions-resume-the-held-run.md:106-111, :234-235, :479-485, :544.

story:decisions-resume-the-held-run — `DecidedResume` has no `Stopped` variant. So the shared `resume_decided_request` cannot report a stopped run: an SDK host gets `NothingHeld`, and the CLI's `stopped: C by P (reason)` line needs a second read of `RunStopped` outside the shared function. `--commission C` naming a stopped commission skips the hold check, so it is refused `NothingSuspended` and exits 30, not the exit 0 the stop rule states. Fix: add `Stopped { commission, principal, reason }`, read `RunStopped` inside `resume_decided_request`, and say what `--commission` does on a stopped run. Cites: .engineering/planning/story/decisions-resume-the-held-run.md:90-92, :218-224, :269-270, :349-350.

story:goals-grounds-and-free-answers — The `GoalsEntries` port is declared and tested in `loom-decision` only, and no host can pass it in, so outside that unit test every goals ground is refused `invalid_goals`:
- `HeldCaseGrounds` gains no constructor for it, unlike `with_authority_records`.
- Neither `check_grounds` nor the resolver takes the instant the port answers for, and the resolver "reads no clock".
- Files name no slice, tree or CLI code that passes the port.

The `decisions show` … `revision_moved` bullet also reads a note no stored member keeps: ESS first adds it to `CitationVerdict` only. The body should name the constructor, the path the instant takes, and where the note is stored (for example `grounding_report`). Cites: .engineering/planning/story/goals-grounds-and-free-answers.md:36-41, :107-112, :158-166, :224-225; .engineering/planning/story/grounds-resolve-against-held-cases.md:53-58, :131; .engineering/planning/story/decider-port-and-grounding-check.md:59.

story:goals-grounds-and-free-answers — No edge orders this story against stories 16 and 22. Its contract type changes (`Decide.option` optional, `Decide.answer`, `Ground.supports` optional) force an edit in their `--tree` and daemon `Decide` code, whichever of the two lands second. Its Files also leave out `crates/loom-tree`. The epic records exactly this kind of forced edit as an edge (story 13 after story 10). There are two options. One is a `depends_on` edge from this story to `story:daemon-continues-a-decided-request`, which orders it after story 16 too, with `crates/loom-tree` added to Files. The other is edges from stories 16 and 22 to this story. Cites: .engineering/planning/story/goals-grounds-and-free-answers.md:107-112, :136-137; .engineering/planning/epic/decision-requests-in-the-runtime.md:173-174; aep plan artifact graph --format json.

Round-5 design findings, checked against the stored bodies:
- Story 22 per-case states: fixed. `Decide` and `Tick` are stated per case (daemon-continues-a-decided-request.md:64-71, :168-174), the edge is named (:157-159), and Out of scope now covers only the slot scheduling (:243-244). Its citations of concurrent-cases (lines 68-71, 96-98, 111-112) check out.
- `tick --resume` scope: fixed. It now resumes every run held on a request decided by `silence`, the retry path is stated, and no list of what was taken before an error is needed (decisions-resume-the-held-run.md:175-192).
- Epic Order paragraph: fixed. It names all four edges of story 22 and story 21's edge to story 12 (epic:179-186).
- Declined `NotEligible` order: the reason holds. The retry bullets use the eligible `timo`, and the contract checks eligibility at `00e3557` decision.yaml:574-603, before the request's state.

Checks with no defect found:
- Exit statuses 36 and 37 clash with no status in `lib.rs` or any story. Story 16's 37 says it is clear of 30 to 36. `stop`'s 10, 11, 20, 2 and 1 are `resume`'s numbers for the same causes.
- The `Command` enum has no top-level `Stop` (lib.rs:221-242); `tree stop` is a subcommand of `tree`, so it does not clash.
- A stop releases the Suspension and drops the context (story 21, lines 258-260).
- Story 23 at `00e3557`: every member it names exists with that spelling (decision.yaml:116, :146-147, :166, :204, :245, :255, :276, :335, :340, :466, :482-495, :549-550, :560-573, :609, :678, :860, :869-870, :912-976).
- Free answer key: the contract keys it on the question kind (`unblocks.kind != question`, :493, :571). `scope_changing_answer` (:178) is only a decision class, and no guard reads it. So story 23 follows the contract; approvals and outcomes stay option-only.
- Story 23 has seven `depends_on` edges, matching the epic's list (stories 2, 5, 6, 11, 12, 14 and 21). It holds none of stories 1 to 14, and no story depends on it.
- Each refusal cause and the `revision_moved` note has a library bullet.
- Five moved citations checked at `00e3557`: :465, :496-515, :708-718, :746-749, :785-788. All match.

What I read: 3 stories in full, plus:
- the round-5 design verdict, ledger e3r10 and the epic's Order and Not in scope;
- 4 neighbouring stories: concurrent-cases, grounds-resolve, decider-port, question-and-outcome;
- the code: intent.rs:80-200, :260-290, :340-380, :440-960; main.rs:605-690; lib.rs:1-60 and :221-242; case.rs:166;
- the contract at `00e3557`.

Commands: `aep plan artifact relations`, `aep plan artifact validate` (valid), and `aep plan artifact graph --format json`. I walked 358 `depends_on` edges across the whole store, not just the set, and found no cycle. I traced 29 acceptance reads to a producer (story 21: 13, story 22: 7, story 23: 9). Edges order 27 of them. The other 2 have no producer at all: the context rebuild (first finding) and the goals port and note (third finding).

What I could not establish (outside my lane except where noted):
- Whether `run_prepared_intent_with_state` can hold a commission of the fixture protocol at all. It refuses through `clock_compatible` unless bindings are given (intent.rs:891-923). Acceptance lane.
- `take` and `record` after `stop` have no bullet. Acceptance lane.
- The epic's Not in scope (epic:296-299) still calls the contract extension a later story keyed on classes, which contradicts story 23. Scope lane.
- With no port shipped, SDC through the CLI can never use a goals ground (§ Open 1). Scope lane, for session `sdc` to settle.

```findings
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 106
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the rebuild of a Decision-held commission from its resume context does not hold in code: resume calls resumed_request (main.rs:681), which refuses a case whose inputs are not a software change's (intent.rs:624-625), needs a test program and drops bindings (:628-629), and the context keeps no execution kind (:774-802); no story writes that rebuild, so the story must own it (intent.rs in Files, the context keeping execution kind or bindings, ResumeBuilder supplying the executor) or move the outcome, question, 31 and 32 bullets to a library test of resume_decision_hold"
- file: .engineering/planning/story/decisions-resume-the-held-run.md
  line: 222
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "DecidedResume has no Stopped variant, so resume_decided_request reports a stopped run as NothingHeld and the CLI's stopped: line needs a second read of RunStopped outside the shared function, and --commission on a stopped run gives 30, against the stop rule's exit 0; add Stopped and read RunStopped inside resume_decided_request"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 36
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "GoalsEntries is declared and tested in loom-decision only: HeldCaseGrounds gains no constructor for it, check_grounds and the resolver take no instant, and Files name no slice, tree or CLI code that passes it, so every goals ground outside the unit test is invalid_goals, and the decisions show revision_moved bullet reads a note no stored member keeps"
- file: .engineering/planning/story/goals-grounds-and-free-answers.md
  line: 136
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "no edge orders this story against stories 16 and 22, though its type changes to Decide (option optional, answer) and Ground.supports force an edit in their --tree and daemon Decide code whichever lands second, and its Files omit crates/loom-tree; add a depends_on edge from it to story:daemon-continues-a-decided-request with loom-tree in Files, or from stories 16 and 22 to it"
```
