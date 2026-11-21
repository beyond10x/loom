---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-parallel-safety-r2
kind: review-result
status: active
title: Plan critic parallel-safety, round 2, on epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:approval-decisions-resume-held-runs
- reviews: story:approvals-supplied-to-canon-evaluation
- reviews: story:authority-records-bound-and-grounded-in-canon
- reviews: story:autonomy-grant-table
- reviews: story:decided-records-carry-grounds-to-canon
- reviews: story:decided-record-written-then-recorded
- reviews: story:decider-port-and-grounding-check
- reviews: story:decision-inbox-commands
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
- reviews: story:decision-routing-by-holder
- reviews: story:decision-suspension-reason
- reviews: story:escalate-when-conditions
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:llm-decider
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:requests-from-canon-pending-decisions
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:tree-waits-open-decision-requests
revision: 1
---
needs-revision

**Round-1 fixes: both hold in the stored bodies.**
- **Finding 1 (story 12 reading a `Decision` Suspension): fixed.** `silence-and-expiry-at-a-host-tick.md` has no line that reads a held `Decision` Suspension. Its Acceptance (lines 62-76) covers requests only, and Out of scope (line 82) hands the held-run case to story 14. Story 14 keeps "After `decisions tick` expires the request, `resume` is refused" (`question-and-outcome-decisions-hold-runs.md:96`) and depends on 10 and 13.
- **Finding 2 (the records port split between stories 2 and 7): fixed.** The entity story declares `EscalationSuppressedRecord` and the port's append and read (`decision-request-entity.md:27-33`, `:68-73`). The file store implements both operations (`decision-request-file-store.md:25-29`) and depends on the entity story. Routing appends through the port (`decision-routing-by-holder.md:91-96`). Stories 2 and 7 now share no unowned surface.
- **Edges from the ledger (items 12, 17 and 18): hold in `aep plan artifact graph`.** Story 15 depends on 13 and 5, and story 16 depends on 15. `approvals-supplied-to-canon-evaluation` no longer depends on `tree-waits-open-decision-requests`, so there is no cycle.
- **Same-file chains: every pair is ordered by a `depends_on` path.** The files are `ess/domains/decision.yaml`, `ess/domains/autonomy.yaml`, `ess/domains/governor.yaml`, `crates/loom-governor/src/lib.rs`, `crates/loom-decision/src/lib.rs`, `crates/loom-cli/src/lib.rs`, `ess/intake/domains/routing.yaml` and the `crates/loom-sdk` decisions module.
- **Stories 14 and 15 are the one same-file pair with no edge, and the body admits it.** Story 15 names the sharing of `crates/loom-intake-slice` and `crates/loom-cli` at `approvals-supplied-to-canon-evaluation.md:69-71` and defers to the wave planner. Ledger Open 6 repeats it. I do not raise it.

**Finding**

story:decision-suspension-reason — its body says "Waits on: Nothing in this epic" and names only Commission and testkit surfaces, but making `Suspension.action_request_id` and `expected_case_revision` optional (`:61-66`) breaks production code in `crates/loom-intake-slice/src/grant.rs` (`held.data.action_request_id.0.0` at `:293`, `:315`, `:402`; inferred from `git grep`, it reads the generated `SuspensionData`, so the story's own commit must edit it). Story 15 edits "the grant module of `crates/loom-intake-slice`" (cited), and story 13's refusal of `approve` on a decision-held commission is the function at `grant.rs:285-316` (inferred). Neither 13 nor 15 is ordered against 10. Ledger Open 6 lists the other files story 10 reaches (`file_suspension.rs`, `adversary_w10_u3.rs`, `serve.rs`, `loom-commission-conformance`) and omits this production file. Round 1 left the width of story 10's reach as a scoping note. It is now a named collision with two set stories, and the "Waits on" line is wrong for it. Two remedies, unchosen:
- An ordering edge from 13 and 15 to 10 (or 10 to 13), recording `grant.rs` as the reason.
- Keeping the existing fields required, so the type change does not reach `loom-intake-slice`. For example, a separate member for the decision hold.
— `.engineering/planning/story/decision-suspension-reason.md:54`, `.engineering/planning/story/decision-suspension-reason.md:61`, `crates/loom-intake-slice/src/grant.rs:293`, `.engineering/planning/story/approvals-supplied-to-canon-evaluation.md:45`

**What I read**
- **Artifacts read:** 21. These are the epic, the 19 story bodies in full, and the blocker. I also read `e3-round-1-parallel-safety.md` and `e3r2-changes.md`, which were missing at first and are now restored. I read the epic's inline copy of the answers and `e3-sdc-answers-4.md`. I did not open `e3-sdc-answers.md`, `-2.md` or `-3.md`, because the epic quotes all three and none touches my lane.
- **Commands:** `aep plan artifact show` on the epic and one story, `aep plan artifact graph --format json` through `jq` for the 19 stories' `depends_on` edges, and `git grep` over `crates/`, `ess/` and `generated/` for the symbols story 10 changes.
- **Cross-epic read:** `sed` and `grep` over the frontmatter and Waits-on of the tree-daemon chain stories and the bindings and tree stories.

**Counts**
- **Surfaces established:** 19 stories cited, 0 inferred, 0 unplaced. The blocker is not worked, so I did not place it. Story 10's reach into `loom-intake-slice` is inferred, and it is the only inferred surface.
- **Acceptance reads traced to a producer in the set:** 28. `depends_on` records all 28, directly or transitively. The two unrecorded reads of round 1 are gone.

**What I could not establish**
- **Cross-epic file sharing:** I did not re-raise the pairs round 1 judged to be scope-path matters. Those are `responsibility.yaml`, `crates/loom-governor/src/lib.rs`, `crates/loom-cli/src/lib.rs`, `ess/system.yaml` with `ess/ess-inputs.yaml`, the `runlog.yaml` and `tree.yaml` paths with `crates/loom-tree`, and the Canon pin manifests. Story 16 against the tree-daemon chain is still named only from the other side, at `limit-hits-in-run-log.md:79-82`. A reader will not find the admission in story 16's body.
- **Hidden reach of story 10:** which of its other forced edits (`file_suspension.rs`, the test files, `loom-commission-conformance`) land with stories outside this set, I cannot tell. The scoper must list them.
- **Out of my lane (acceptance or design), not setting the verdict:**
  - Story 15 says Commission then reads an approved action as `Admitted` and asks the authority provider only about actions Canon still reports approval-required (`approvals-supplied-to-canon-evaluation.md:47-49`). Story 13's acceptance relies on `DecisionAuthority` re-checking a moved ground before a resume (`approval-decisions-resume-held-runs.md:135-137`). After story 15 lands, that check may never run for an approved action.
  - Ledger Open 2 lists stale SDC citations that the revision did not rewrite.
  - Story 14's "a host writer records an answer artifact revision and the request's record" names no story that builds that writer.

```findings
- file: .engineering/planning/story/decision-suspension-reason.md
  line: 54
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body says it waits on nothing and names only Commission and testkit surfaces, but making Suspension.action_request_id and expected_case_revision optional breaks production code in crates/loom-intake-slice/src/grant.rs (held.data.action_request_id at lines 293, 315, 402; inferred from git grep, not cited in any body), which story:approvals-supplied-to-canon-evaluation edits ('the grant module of crates/loom-intake-slice', approvals-supplied-to-canon-evaluation.md:45, cited) and story:approval-decisions-resume-held-runs reaches through its approve refusal (inferred); no depends_on edge orders story 10 against either, and ledger Open 6 omits this file. Either record an ordering edge naming grant.rs as the shared file, or keep the existing Suspension fields required so the type change does not reach loom-intake-slice."
```
