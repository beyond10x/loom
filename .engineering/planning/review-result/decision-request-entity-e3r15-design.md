---
format: aep.planning-md/3
id: review-result:decision-request-entity-e3r15-design
kind: review-result
status: active
title: Plan critic (design) on revision e3r15 of story:decision-request-entity
relations:
- reviews: story:decision-request-entity
revision: 1
---
needs-revision

story:decision-request-entity — two sentences new in this revision call `story:ess-format-37` "(draft)", but it has been `active` in wave 2026-11-21-w6 since `f51c2141`; change both to "(active, wave 2026-11-21-w6)" — .engineering/planning/story/decision-request-entity.md:185, :308; `aep plan artifact show story:ess-format-37` (status active)

Answers to the five questions:

1. Edges. No cycle. I walked 15 `depends_on` edges to 12 reachable nodes, outside the epic too, and none leads back. `ess-format-37` depends on `ess-096-upgrade`, which depends on `ess-format-31`. So the edges to `ess-format-31` and `ess-096-upgrade` are now implied through `ess-format-37`. Keeping them costs nothing: both are implemented, and the Waits on section says why each is there (`[]` and `coalesce` need `ess/31`). Not a finding.
2. Stale sentences: none left in any draft or proposed story of the epic. The grep hits are the two edited sibling clauses, the story itself, and the epic, whose lines 219-220 and 329-331 already state the ess/37 decision. Hits about implemented stories are history (`ess-096-upgrade.md:82,106,245`). The other hits use "ordered" for collision order, not for enums. No draft (`authority-records-bound-and-grounded-in-canon`, `decided-records-carry-grounds-to-canon`, `requests-from-canon-pending-decisions`) names `ess/31` or an order.
3. No story downstream relies on a Rust order of the two enums. Levels `A0` to `A5` are only looked up as preset rows (`autonomy-grant-table.md:36-44,86-92`; `approval-decisions-resume-held-runs.md:193-262`; `non-independent-review-becomes-decision-request.md:219-253`). Strength is tested by membership (`decider-port-and-grounding-check.md:67`; `decision-inbox-commands.md:156`, "moderate" refused, "strong" required). The only threshold hit, `escalate-when-conditions.md:70`, is about cost. Nobody owns an order comparison, because none exists.
4. Wave 7. `aep plan artifact waves --status proposed`, first wave: `story:decision-request-entity`, `story:bound-program-result-table`, `story:engineering-protocols-dependency-removed`, `story:standing-tree-rests-quiescent`. None of the other three names `ess/system.yaml`, `ess/ess-inputs.yaml` or `crate_names.rs` in its scope or body. They share generated or lock files only: `bound-program-result-table` regenerates `generated/rust/loom` through `ess/domains/bindings.yaml`, and `engineering-protocols-dependency-removed` touches `Cargo.lock`. That overlap is the parallel-safety critic's question. A re-scope is needed. Run it after wave 6 closes, because `boundary-admits-declared-http-egress` (active) appends scenario lines to `ess/ess-inputs.yaml`, and before wave 7's plan is opened. Stale today:
   - `:298` points at `system.yaml:24` and `ess-inputs.yaml:17`. In the tree, `loom.runlog` sits at `:25`, and `domains/runlog.yaml` at `:19`.
   - `:301` and `:310` give `[..; 29]`. The tree has `[..; 30]` at `crate_names.rs:18`, so the count becomes 31.
   - `:309-310` still list `hash-chained-run-log`, which is implemented. The collision list is also missing `observe-executor-reads-workspace-files` and `gates-document-evaluated-in-the-governor` on `ess/system.yaml` and `ess/ess-inputs.yaml`, which the `waves` collision lines report.
5. Both sibling clauses are true and agree with the story. `Holder` has no `ordered:` key (`~/beyond10x/sdc/ess/domains/decision.yaml:182-184`), and the only two keys are at `:109` and `:171`. The generator writes `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` (`ess` `0.96.0:crates/generate/ess-synth/src/rust/items.rs:150-152`). Loom's generated enums carry the same derive today (`generated/rust/loom/src/bindings.rs:61`).

What I read: 4 artifacts in full (the three stories and `ess-format-37`) plus the epic and its 26 stories by grep, the git diff and the ledger. I ran `aep plan artifact relations`, `graph --format json`, `waves --status proposed`, `validate` (valid; only old review-result notices), and read the tree files and ESS `0.96.0` sources named above. Acceptance reads traced to a producer: one (`format: ess/37` in `ess/system.yaml`, line :266, produced by `ess-format-37`), and an edge records it.

What I could not establish: whether session `sdc`'s own code compares levels or strengths by order (the ledger leaves this unchecked too). Outside my lane, for the scope and parallel-safety critics: the stale Scope section above. Also, the epic's Source paragraph (`epic/decision-requests-in-the-runtime.md:447`) lists the sixth message (`ess/31`) but not the 2026-11-21 decision for `ess/37`.

```findings
- file: .engineering/planning/story/decision-request-entity.md
  line: 185
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "two sentences new in this revision call `story:ess-format-37` \"(draft)\", but it has been `active` in wave 2026-11-21-w6 since `f51c2141`; change both to \"(active, wave 2026-11-21-w6)\""
```
