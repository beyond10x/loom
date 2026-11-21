---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-design-r4
kind: review-result
status: active
title: Plan critic design, confirming pass (round 4), on stories 1 to 14 and story:ess-format-31 of epic:decision-requests-in-the-runtime
relations:
- reviews: epic:decision-requests-in-the-runtime
- reviews: story:ess-format-31
- reviews: story:decision-request-entity
- reviews: story:decision-request-file-store
- reviews: story:autonomy-grant-table
- reviews: story:decider-port-and-grounding-check
- reviews: story:grounds-resolve-against-held-cases
- reviews: story:llm-decider
- reviews: story:decision-routing-by-holder
- reviews: story:escalate-when-conditions
- reviews: story:decided-record-written-then-recorded
- reviews: story:decision-suspension-reason
- reviews: story:decision-inbox-commands
- reviews: story:question-and-outcome-decisions-hold-runs
- reviews: story:silence-and-expiry-at-a-host-tick
- reviews: story:approval-decisions-resume-held-runs
revision: 1
---
needs-revision

story:decision-inbox-commands — `decide` and `take` read the case revision inline from the `--store` governor, and the SDK's `decide` and `take` say nowhere where the revision comes from. One story later, story:silence-and-expiry-at-a-host-tick declares the `CaseRevisions` port for the same read, so every host (`--state`, `--tree`) has to wire that read twice; `CaseRevisions` should be declared in this story and taken by `decide`, `take` and `tick` alike — .engineering/planning/story/decision-inbox-commands.md:53-57, :92-95; .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:36-38, :102

story:approval-decisions-resume-held-runs — the story adds `--state` to every `decisions` command, `tick` included, and its acceptance runs `decisions tick --state`, but it names only the writer (`GrantRecordWriter`) and no `CaseRevisions` for the state directory, although story:silence-and-expiry-at-a-host-tick gives that port to this story — .engineering/planning/story/approval-decisions-resume-held-runs.md:124-127, :214-219; .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:67-69

story:question-and-outcome-decisions-hold-runs — `DecisionHoldingExecutor` calls `route_stop` for outcome and question stops but names no `ExpiryInstant`, while story:decision-routing-by-holder lists this story as one of the three hosts that implement that port; the body should say it passes the slice implementation from story:approval-decisions-resume-held-runs — .engineering/planning/story/question-and-outcome-decisions-hold-runs.md:44-61; .engineering/planning/story/decision-routing-by-holder.md:40-42

story:decision-suspension-reason — its Out of scope gives "a command that calls `resume_after_decision`, and its exit statuses" to story:question-and-outcome-decisions-hold-runs, but that story now wires no command and gives those refusals no exit status, so the surface has no owner in the set and the pointer should say so — .engineering/planning/story/decision-suspension-reason.md:280-282; .engineering/planning/story/question-and-outcome-decisions-hold-runs.md:142-153

**The new seams you asked about:**
- `CanonGovernor::decide_at_revision`, `CanonGovernor::explicit_decisions` and `DecisionError::CaseMoved` have one owner: story:decided-record-written-then-recorded. The question story names `decide_at_revision` only as the model for its own guard.
- `CanonGovernor::update_revision_at`, `DecisionHoldingExecutor`, `resume_decision_hold` and `ArtifactRevisionWriter` have one owner: story:question-and-outcome-decisions-hold-runs. story:requests-from-canon-pending-decisions reuses the executor and is ordered after it.
- `decider:none` and `rule:none` are owned by routing; the approval and tree stories only use them.
- `ExpiryInstant` is declared by routing and implemented by approval (the slice) and tree-waits; the gap is finding 3.
- `CaseRevisions` is declared by silence and implemented for `--store` (silence) and `--tree` (tree-waits); findings 1 and 2 cover the rest.
- No two stories claim the same surface. The seams are not split between stories: `decide_at_revision` and `update_revision_at` are two different guarded writes, each with its own caller.

**Edges:**
- All five ledger edges are in the graph: two from `story:ess-format-31` (decomposes, serves) and three depends_on (entity on ess-format-31, inbox on suspension, approvals-supplied on question).
- Each new depends_on edge has its reason written in a body: the `ess/31` read, `crates/loom-cli/src/lib.rs`, and `main.rs`/`lib.rs`.
- No cycle.

`story:ess-format-31` holds its shape. It has no outgoing edge. Its behaviour change comes with the format raise and cannot be split off. Its only reader in the set is the entity story, and the edge records that.

**What I read:** 15 artifacts, whole bodies.
- I read the ledger `e3r5-changes.md`, `e3-round-3-design.md` and the "Story defects" of six scope reports: inbox, silence, suspension, question, approval and decided-record.
- I checked the cited governor lines at `crates/loom-governor/src/lib.rs` (`:193`, `:205`, `:629`, `:1020`, `:1042`, `:1055`, `:1068-1089`, `:1097`, `:1166`, `:1199`, `:1225`, `:1294`) and `tests/explicit_decisions.rs:174`, `:183`.
- Commands: `aep plan artifact relations`, `aep plan artifact graph --format json`, `aep plan artifact validate` (it says `valid`; its warnings are about other artifacts).
- I walked 417 `depends_on`/`blocks` edges across the whole store, outside the set too, and found no cycle. The set has 21 depends_on edges among its own stories.
- I traced 36 acceptance reads to a producer in the set. Edges record all 36, directly or through a chain.

**What I could not establish (all out of my lane):**
- Parallel safety: `story:ess-format-31` has no `scope:` block. `validate` does not flag it because it is a draft.
- ESS rule: the silence story declares `CaseRevisions` in `ess/domains/decision.yaml`, but routing's ESS first does not declare `ExpiryInstant`. Whether a host port trait needs an ESS home is not settled in either direction.
- Acceptance: two bodies still give `ess/25` as the reason, though the claims still hold at `ess/31`: decider-port-and-grounding-check.md:65-66 and autonomy-grant-table.md:60-61.
- Acceptance: `update_revision_at` names no refusal type of its own (question-and-outcome-decisions-hold-runs.md:109-116).
- Acceptance: approval's replacement of `approve`'s answer on a `Decision` hold has no acceptance bullet that builds such a hold (approval-decisions-resume-held-runs.md:134-141, :229-230).

```findings
- file: .engineering/planning/story/decision-inbox-commands.md
  line: 53
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "decide and take read the case revision inline from the --store governor, and the SDK's decide and take name no revision source, while story:silence-and-expiry-at-a-host-tick declares the CaseRevisions port for the same read one story later, so each host wires that read twice; CaseRevisions belongs in this story and should be taken by decide, take and tick alike"
- file: .engineering/planning/story/approval-decisions-resume-held-runs.md
  line: 124
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the story adds --state to every decisions command, tick included, and its acceptance runs decisions tick --state, but it names only the writer and no CaseRevisions for the state directory, which story:silence-and-expiry-at-a-host-tick assigns to it"
- file: .engineering/planning/story/question-and-outcome-decisions-hold-runs.md
  line: 44
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "DecisionHoldingExecutor calls route_stop for outcome and question stops but names no ExpiryInstant, while story:decision-routing-by-holder lists this story as one of the three hosts that implement that port; the body should say it passes the slice implementation from story:approval-decisions-resume-held-runs"
- file: .engineering/planning/story/decision-suspension-reason.md
  line: 281
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Out of scope gives a command that calls resume_after_decision, and its exit statuses, to story:question-and-outcome-decisions-hold-runs, which now wires no command and gives those refusals no exit status, so the surface has no owner in the set and the pointer should say so"
```
