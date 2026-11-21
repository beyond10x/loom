---
format: aep.planning-md/3
id: review-result:decision-requests-in-the-runtime-parallel-safety-r1
kind: review-result
status: active
title: Plan critic parallel-safety, round 1, on epic:decision-requests-in-the-runtime
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

Two findings. Neither is a file collision. Both are cases where one story reads something another story creates and no `depends_on` path joins them.

**Findings**

story:silence-and-expiry-at-a-host-tick — its acceptance reads a held `Decision` Suspension, which only story:decision-suspension-reason creates (the `SuspensionReason::Decision` variant and a Pending Suspension with no action request). The ancestors of story 12 are 11, 9, 8, 7, 6, 5, 4, 3, 2 and 1, and story 10 is not among them. One wave can hold both, and story 12's implementor would stub a Suspension kind that does not exist yet. The missing edge is `depends_on story:decision-suspension-reason`, or dropping the line, which story 14 already covers with "After `decisions tick` expires the request, `resume` is refused and the run stays held". The surface is cited. — .engineering/planning/story/silence-and-expiry-at-a-host-tick.md:59-60, .engineering/planning/story/decision-suspension-reason.md:17-22

story:decision-routing-by-holder — its body keeps the `EscalationSuppressed` record "beside the request in the store", and its acceptance reads it from the in-memory store. Story 1's `DecisionStore` port has no operation for records beside a request. It lists only read, write-if-unchanged, list and find-open-for-key. Story 2's `FileDecisionStore` implements that port and says it stores "the records a later story appends about it" in `StoredDecisionRequest`, a type declared in `ess/domains/governor.yaml`. Stories 2 and 7 are unordered: 2 depends only on 1, and 7 reaches 1 through 4 and 3. So whichever story adds the method to the port breaks the other's build, and 2 shares no scoped path with 7. The remedies are:
- Add `depends_on story:decision-request-file-store` to 7, so one story owns the record shape.
- Declare the records operation in story 1's port, so 2 and 7 both build on it.

The surface is cited in the three bodies. The port-gap reading is my inference from the port's listed operations. — .engineering/planning/story/decision-routing-by-holder.md:79-82, .engineering/planning/story/decision-request-file-store.md:25-26, .engineering/planning/story/decision-request-entity.md:27-29

**Pairs checked and not findings**
- **Within the set:** every pair on one file or domain is ordered by a `depends_on` path. The files and domains checked were:
  - `ess/domains/decision.yaml`: 1, 4, 7, 8, 9, 12.
  - `ess/domains/autonomy.yaml`: 3, 8.
  - `ess/domains/governor.yaml` and `crates/loom-governor/src/lib.rs`: 2, 5, 9, then 18, 16, 17, 19.
  - `crates/loom-decision/src/lib.rs`: 1, 3, 4, 7, 8, 9, 12.
  - `crates/loom-cli/src/lib.rs`: 11, 12, 13, 14, 15, 18.
  - `crates/loom-sdk` decisions module: 11, 12.
  - `ess/intake/domains/routing.yaml`: 13, 14, 19.
  - `crates/loom-tree`, `ess/domains/tree.yaml` and `ess/domains/runlog.yaml`: 15, 16, 18, 19.
  - `ess/system.yaml` and `ess/ess-inputs.yaml`: 1, 3.
  - `ModelDecider` in `crates/loom-executor`: 6, 8.
- **Unordered but sharing nothing:** 6 (`crates/loom-executor`) with 2, 5 and 7, and 10 (Commission) with 1 to 9, 11 and 12.
- **Cross-epic ordering:** 15, 16, 17, 18 and 19 come after every story of `epic:protocol-tree-on-bindings`. Story 15 reaches all 14 of them through `tree-run-command`, `hash-chained-run-log`, `agent-node-run-records` and `canon-0-9-0-pin`. Stories 1 to 14 depend on nothing in either other epic, and the daemon-epic chain (`engine-configuration-document` to `tree-daemon`) is unordered against all 19 stories. `story:budget-and-limit-stops-raise-decision-requests` is ordered after 7, 15, `action-limits-enforced` and `limit-hits-in-run-log`.
- **Cross-epic file-sharing pairs:** none is a finding, because each shares a path a scope entry can list, and `aep plan artifact waves` keeps two stories that share a scope path out of one wave. The shared paths are:
  - `ess/commission/domains/responsibility.yaml`: 10 with `child-case-runtime` and `deterministic-default-selector` (the latter is `active`). Story 10's body says so at line 54.
  - `crates/loom-governor/src/lib.rs`: 2, 5 and 9 with `bindings-document`, `bound-program-result-table`, `bound-artifact-revisions`, `hash-chained-run-log`, `canon-0-9-0-pin`, `child-case-runtime`, `record-copy-between-cases` and `deterministic-default-selector`.
  - `crates/loom-cli/src/lib.rs`: 11 to 14 with `confinement-opens-from-engine-config`, `bound-commands-run-confined`, `tree-run-command`, `agent-node-run-records`, `engine-configuration-document` and `tree-daemon`.
  - `ess/system.yaml`, `ess/ess-inputs.yaml` and `crates/loom-executor/tests/crate_names.rs`: 1 and 3 with `bindings-document`, `hash-chained-run-log` and `tree-run-command`.
  - `ess/domains/runlog.yaml`, `ess/domains/tree.yaml`, `crates/loom-tree` and `docs/contracts/run-log-v1.json`: 15 to 19 with `limit-hits-in-run-log`, `action-limits-enforced`, `executable-allow-list`, `named-egress-per-action` and `tree-daemon`.
  - `Cargo.lock` and the Canon pin manifests: 16 with `substrate-daemon-runner`, `canon-0-9-0-pin` and the other pin stories.
- **Cross-epic reader beside producer:** none found. Every story of the set that reads state from another epic is ordered after its producer.

**What you read**
- 21 artifacts: the epic, 19 stories and the blocker, plus the frontmatter and relations of 24 stories of the two other epics.
- Commands: `aep plan artifact show` on each story, `aep plan artifact graph --format json`, `aep plan artifact waves`, `git grep` and `grep` over the story bodies for the eight listed surfaces, and `sed` over the Loom source files that story 10's type change touches.

**Counts**
- Surfaces established: 19 stories cited, 0 inferred, 0 unplaceable. The blocker is not worked, so I did not place it.
- Acceptance reads traced to a producer in the set: 20. A `depends_on` path records 18 of them, through a direct edge or a transitive one. The two it does not record are the two findings above.

**What I could not establish**
- Story 10's change to `SuspensionReason` and to the `Suspension` fields reaches beyond the two crates it names. This is inferred from `git grep` and is not a finding, because it is a scoping matter. The files affected are:
  - `crates/loom-governor/src/file_suspension.rs:277-298` (an exhaustive `match` on the reason, and encode/decode of `action_request_id` and `expected_case_revision` at lines 261-266 and 459-461).
  - `crates/loom-cli/tests/adversary_w10_u3.rs`.
  - `crates/loom-agent/tests/serve.rs`.
  - `crates/loom-commission-conformance`.

  The story-scoper must list them in story 10's scope, or `waves` will not keep story 10 apart from other stories that edit those files.
- Story 16 moves "every manifest naming `b10x-canon`". Its scope needs the manifests listed one by one, or it collides with everything.
- Out of my lane, noted only:
  - Story 15 (`tree-waits-open-decision-requests.md:46`) prints `decisions decide … --tree <STORE>`, and its acceptance runs that command. No story of the set says it adds a `--tree` flag to `decisions` (story 11 has `--store`, story 13 `--state`). This is an acceptance or scope point.
  - Story 5's acceptance "a child case named by `case` resolves against the child" does not say whether it needs the parent-child link of `story:child-case-runtime`. This is a body-clarity point.

```findings
- file: .engineering/planning/story/silence-and-expiry-at-a-host-tick.md
  line: 60
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads a held Decision Suspension that only story:decision-suspension-reason creates, on a different file, and no depends_on path orders story 12 after story 10, so one wave could hold both; the missing edge is depends_on story:decision-suspension-reason, or drop the line (story 14 already covers it). The surface is cited."
- file: .engineering/planning/story/decision-routing-by-holder.md
  line: 80
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the body keeps EscalationSuppressed beside the request in the store, but the DecisionStore port of story:decision-request-entity (entity.md:27-29) has no operation for it and story:decision-request-file-store (file-store.md:25-26) implements the port and stores those records in StoredDecisionRequest; stories 2 and 7 are unordered, so whichever adds the port method breaks the other's build. Either record an ordering edge from 7 to 2 for the shared record shape, or declare the records operation in story 1's port. The surfaces are cited in the three bodies; the port-gap reading is inferred from the port's listed operations."
```
