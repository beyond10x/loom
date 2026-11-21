---
format: aep.planning-md/3
id: review-result:tree-daemon-under-enforced-limits-parallel-safety-r2
kind: review-result
status: active
title: Plan critic parallel-safety, round 2, on epic:tree-daemon-under-enforced-limits
relations:
- reviews: epic:tree-daemon-under-enforced-limits
- reviews: story:substrate-daemon-runner
- reviews: story:engine-configuration-document
- reviews: story:executable-descendants-held
- reviews: story:ingress-routes-per-action
- reviews: story:confinement-level-on-oci-and-pod
- reviews: story:action-limits-enforced
- reviews: story:limit-hits-in-run-log
- reviews: story:executable-allow-list
- reviews: story:confinement-level-required
- reviews: story:named-egress-per-action
- reviews: story:agent-session-through-substrate
- reviews: story:tree-daemon
revision: 1
---
approve

**Round-1 findings.** All 12 are resolved, none carried. I checked each against the graph: `aep plan artifact graph --format json`.

The 12 `depends_on` links of the chain are all present, each from the later story to the earlier one. `engine-configuration-document` depends on `agent-node-run-records`, and the other eleven each depend on their predecessor. A script over the graph found two more facts:
- Every story of the chain has all the earlier ones among its transitive `depends_on` ancestors.
- All 14 stories of `epic:protocol-tree-on-bindings` are ancestors of `engine-configuration-document`, so none can run beside any story of this set.

There is no cycle. No upstream story depends on a story of this set. The only inbound edges are the four blockers' `blocks` edges.

| R1 finding (file:line) | Disposition |
|---|---|
| `confinement-level-required.md:40`, `EngineConfinement` variants shared with the daemon runner | Resolved. The story's ESS first (`:63-68`) now edits only `engine.yaml` and `bindings.yaml`. It depends on `substrate-daemon-runner` (`:14`). |
| `substrate-daemon-runner.md:34`, unordered against `bound-commands-run-confined` | Resolved. The path is `substrate-daemon-runner` → `engine-configuration-document` → `agent-node-run-records` → `bound-agent-node` → `bound-artifact-revisions` → `bound-program-result-table` → `bound-commands-run-confined`. The body names the order at `:150-154`. |
| `action-limits-enforced.md:32`, five stories on `confinement.rs` | Resolved. The chain orders all five. `executable-allow-list.md:130-134` and `named-egress-per-action.md:219-223` name the file. |
| `action-limits-enforced.md:50`, daemon arm with no edge | Resolved. The edge runs through `confinement-level-required` to `substrate-daemon-runner`, and the body says so at `:165-168`. |
| `confinement-level-required.md:42`, `BoundAction` in `bindings.yaml` from five stories | Resolved. All five are totally ordered. |
| `named-egress-per-action.md:41`, `bindings.yaml` and `tree.yaml` shared | Resolved. Its types moved to `engine.yaml` (`:228-232`), and the chain orders `bindings.yaml`. |
| `executable-allow-list.md:41`, `ExecutablePolicy` in `tree.yaml` | Resolved. It is now `ExecutablesSection` in `engine.yaml` (`:139`). |
| `tree-daemon.md:41`, unordered against four stories | Resolved. `tree-daemon` depends on `agent-session-through-substrate` (`:108`). |
| `limit-hits-in-run-log.md:25`, `TreeStatus` and the `loom-cli` printer unnamed | Resolved. The body names both at `:57-61`, with the printer path marked inferred. `tree-daemon` comes after it. |
| `agent-session-through-substrate.md:23`, executable policy with no edge | Resolved. The path is `agent-session-through-substrate` → `named-egress-per-action` → `executable-allow-list`. The body names it at `:24` and `:53`. |
| `agent-session-through-substrate.md:33`, `AgentRun` and `loom-session-executor` against `agent-node-run-records` | Resolved. The path is `agent-session-through-substrate` → `named-egress-per-action` → `executable-allow-list` → `limit-hits-in-run-log` → `agent-node-run-records`, and that last edge is direct (`limit-hits-in-run-log.md:15`). The body names it at `:53-56`. |
| `action-limits-enforced.md:62`, "each step's record" unplaced | Resolved. It is `loom.engine.StepConfinement` in `ess/domains/engine.yaml`. It is named in the ESS first of `engine-configuration-document` (`:66`), `substrate-daemon-runner` (`:161`), `confinement-level-required` (`:64`), `action-limits-enforced` (`:175`), `executable-allow-list` (`:141`), `named-egress-per-action` (`:230`) and `ingress-routes-per-action` (`:122`). |

**What the chain does not cover.** I found nothing.
- **Scope-section check.** I read the Scope sections of `bound-commands-run-confined`, `agent-node-run-records`, `tree-run-command` and `confinement-opens-from-engine-config`. Every file or type they list as colliding is edited in this set only by a story that sits after all 14 upstream stories. That covers `confinement.yaml`, `confinement.rs`, `executor.rs`, `loom-cli` `lib.rs` and `main.rs`, `ess/system.yaml`, `ess-inputs.yaml`, `runlog.yaml`, `tree.yaml`, `bindings.yaml`, `crates/loom-tree`, `loom-session-executor/src/lib.rs`, `cli.md` and `Cargo.lock`.
- **Test files.** The 12 new test file paths each appear in one story only.
- **Surfaces.** Every story names at least an ESS file and a test path, so none is unplaceable.
- **Step's record.** `executable-descendants-held.md:51` says only "`descendants_held` exists" without naming `StepConfinement`. It edits no member of that type and names `engine.yaml` in `:43`, so I do not count it as a finding.

**What I read**
- **Artifacts:** 17 in the set: the epic, 12 stories and 4 blockers. I also read the round-1 report and `.engineering/drafts/e2r2-changes.md`, and four upstream stories (Scope sections plus bodies).
- **Commands:** `aep plan artifact show`, `graph --format json` with an ancestor and cycle script, `validate` filtered to this set (no output), and `waves` (the set is unscoped, so it holds no data on it). I also ran `git grep` on `confinement.rs` and `confinement.yaml`.
- **Surfaces:** 12 stories placed cited, 0 inferred, 0 unplaceable. `crates/loom-cli/src/tree.rs` is inferred, but only in `tree-run-command`'s scope.
- **Acceptance reads traced to a producer in the set:** 17.
  - All 17 are ordered by a `depends_on` path.
  - Six have a direct edge. Eleven are ordered only through the chain.

**What I could not establish**
- Out of lane: the daemon's CPU time and `memory_peak_bytes` need `CommandBuilder::measure`. `limit-hits-in-run-log.md:65` assumes measurement is on, and `substrate-daemon-runner.md` does not say its runner asks for it. The order holds. The open question is which story owns the setting.
- Outside the set, not assessed: `story:tool-decisions-on-step` (draft, `epic:loom-sdk-for-products`) is scoped to `crates/loom-session-executor/`. `agent-node-run-records` Scope already names it as a collision, and `agent-session-through-substrate` edits the same crate.
- The three held stories each call for a Substrate pin bump "of its own", and no such story exists. Nothing in the set produces it, so I could not trace that read.
- Not mine: the serial chain also holds `ingress-routes-per-action` behind `upstream-blocker:substrate-executable-allow-list`, through `executable-descendants-held`. You accepted the chain.

```findings
[]
```
