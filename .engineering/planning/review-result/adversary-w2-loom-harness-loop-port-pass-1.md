---
format: aep.planning-md/3
id: review-result:adversary-w2-loom-harness-loop-port-pass-1
kind: review-result
status: active
title: Wave 2026-10-06-w2 adversary, loom story:harness-loop-port, pass 1
relations:
- reviews: story:harness-loop-port
revision: 1
---
```
unit: story:harness-loop-port
verdict: red
cases: executed 615→623, red 3
origin: introduced 3, pre-existing 0, undecided 0
wrote-outside-worktree: ~/.cache/loom-waves-2026-10-06/w2/harness-loop-port/adversary-1/ (9 logs); build artifacts in the assigned ~/.cache/b10x-target/loom-w2-harness-loop-port
needs-coordinator: yes. The head fails the package gate (finding 3), and the implementor's green gate log was written before the head commit.
```

The findings cover worktree `loom-w2-harness-loop-port` at head `16140c9`, plus one new untracked test file.

### 1. Diff
```
$ git --no-pager diff --stat
(empty: no tracked file changed)
$ git status --short
?? crates/loom-executor/tests/adversary_w2_harness_loop_port.rs
```
The only path I wrote in the worktree is that test file (714 lines). No implementation file was touched.

### 2. Cases added (`crates/loom-executor/tests/adversary_w2_harness_loop_port.rs`)
They use a scripted in-process `ModelPort` and `FakeGovernor`. Nothing goes over the network.

| Case | Asserts | Now |
|---|---|---|
| `adversary_w2_request_tools_are_the_catalogue_whatever_the_config_says` | with an output schema in the config, request 1's tools are exactly the catalogue | **red** |
| `adversary_w2_a_selection_admitted_inside_a_delegate_is_proposed` | with delegation on, a catalogue call made inside the delegate and admitted by the pipeline is returned as a `ProposedAction` | **red** |
| `…failed_revalidation_is_denied_and_the_previous_catalogue_is_gone` | a failed revalidation is denied to the model; turn 2 drops the action; calling it again is refused before the selector; 5 governor reads | green |
| `…colliding_tool_names_suspend_before_any_request` | `repository.merge` and `repository_merge` together give Suspended(ExternalAvailability) naming both, with 0 requests | green |
| `…a_session_on_another_wire_is_refused_and_not_opened` | a session on the wrong wire gives Suspended and 0 requests; the same session id then proposes on the right wire | green |
| `…without_a_governor_the_handed_frontier_is_the_catalogue` | proposes with 0 revalidations; with two calls in one turn, only the first is selected | green |
| `…a_summary_turn_is_not_a_turn_of_the_session` | a compaction summary turn is not recorded, and turns 1–3 keep their own items (no other test reaches the `recorded` guard) | green |
| `…the_most_permissive_ceiling_still_proposes` | at the `Destructive` ceiling the call is still proposed rather than reaching `ToolPort::call` | green |

Each red case was run alone (`cargo test -p b10x-loom-executor --locked --test adversary_w2_harness_loop_port -- <name> --exact`), verbatim:
```
panicked at crates/loom-executor/tests/adversary_w2_harness_loop_port.rs:96:5:
assertion `left == right` failed: acceptance 1 and `LoopPorts::config` ("Its tools are the catalogue's, whatever it says"): the request's tool list is the projected catalogue and nothing else
  left: ["repository_inspect", "repository_edit", "tests_run", "repository_merge", "answer"]
 right: ["repository_inspect", "repository_edit", "tests_run", "repository_merge"]
```
```
panicked at crates/loom-executor/tests/adversary_w2_harness_loop_port.rs:149:5:
assertion `left == right` failed: governed.rs module docs: an admitted selection stops the loop at its checkpoint and is returned as the ProposedAction; selections [SelectionSnapshot { state: Admitted, ... action: "tests.run", ... strategy: ReasoningModel, case_revision: 2 }]; revalidations [Admitted { ... }]; loop stop Some(Completed)
  left: CompletedLocalReasoning(Unit(true))
 right: ProposedAction(ExecutorOutcomeProposedAction { action: "tests.run", arguments: ProposedActionArguments(Object([("suite", Text("unit"))])) })
```

### 3. Suite run
Command: `cargo test -p b10x-loom-executor --locked --no-fail-fast`. Exit 101, 620 passed, 3 failed, 1 ignored. The three failures:
```
test adversary2_seams_are_named_from_ported_crates ... FAILED
test adversary_w2_a_selection_admitted_inside_a_delegate_is_proposed ... FAILED
test adversary_w2_request_tools_are_the_catalogue_whatever_the_config_says ... FAILED
```
The first one is an existing test. Run alone, verbatim:
```
panicked at crates/loom-executor/tests/adversary2_harness_map.rs:275:9:
§ Seams: `approval checkpoint` does not cite `harness-loop/src/approval.rs`
```
The "before" count of 615 comes from the implementor's `gate-test.log`. That log was written at 18:14:58, but `16140c9` was committed at 18:16:33, so the reported green gate does not cover the head. Clippy and rustfmt are clean on the new file.

### 4. Findings

**F3 (blocker, NEEDS-CHANGE, introduced): `docs/design/harness-map.md:97`.** Commit `16140c9` added "(the approval checkpoint below)" to the `ToolPort` bullet. The existing check takes the first bullet that contains "approval checkpoint", so it now picks the wrong one and the package gate fails. In the base file (`git show 2e86894:docs/design/harness-map.md`) the phrase appears only in its own bullet, at line 101. What reaches it: `cargo test -p b10x-loom-executor`, the brief's own gate. Fix: reword line 97, for example "see the approval seam below". Do not weaken the test.

**F1 (warning, INFEASIBLE, introduced): `governed.rs:181`.** `config` is passed to the loop unchanged. The loop's own tools (answer, `delegate`, `skill`, `recall`) are added after the catalogue, so the tool list is no longer the catalogue. This contradicts the unit's own doc at `governed.rs:95-96` ("whatever it says"), `CHANGELOG.md:14` ("exactly that catalogue") and acceptance item 1. What reaches it: nothing found. No in-tree caller sets those config fields, and the CHANGELOG says `b10x-loom run` does not use the loop yet. Fix: clear those fields in `run_loop`, or correct the doc.

**F2 (warning, INFEASIBLE, introduced): `governed.rs:509`.** Inside a delegate, the model's call goes through selection, arguments and an admitted revalidation. `self.ended` is set, but the nested loop refuses the checkpoint (`turn_loop/mod.rs:4409`). The run then ends `Completed`, so an admitted selection is recorded but never proposed. This breaks the module doc at `governed.rs:20-22` and acceptance item 2 under delegation. What reaches it: only a config with delegation on, and no caller found. It goes away if F1 is fixed by clearing the loop's own tools.

### 5. Attacked and could not break
- A frontier change between turns: turn N+1 carries the new catalogue, and a call against turn N's catalogue is refused.
- An approval-gated action is proposed, and Commission decides. A blocked action is never published.
- No path reaches `ToolPort::call` or an effect, including the batch path, delegates and the most permissive ceiling.
- The model cannot set the case revision, frontier, authority or checkpoint id; the checkpoint id is the selection id.
- Recorded `Turn`s against `run.yaml`: only completed turns, provider items verbatim, summary and delegate turns not recorded.
- Acceptance item 2 in substance: exactly 1 selection and 1 argument request per call, including two calls in one turn. Loom's own configured selector and generator are skipped by design.
- `CompletedLocalReasoning` versus `NoUsefulAction`: Commission treats them the same (`outcome.rs:102`).
- Arguments nested deeper than Commission's 64-level limit give Suspended, which matches how the ported wire already treats invalid JSON. Not reported.
- `LoopExecutor` under `run_until_blocked`: not built. The only difference I could identify is the known stale-revision limitation.
- Two runs at once on one session would produce the same turn and selection ids. I did not test this because nothing runs that way.

### 6. Paths written outside the worktree
- `~/.cache/loom-waves-2026-10-06/w2/harness-loop-port/adversary-1/`: `red-1.log`, `red-2.log`, `red-3-existing.log`, `rest.log`, `summary.log`, `ceiling.log`, `suite.log`, `suite-nff.log`, `clippy.log`
- `~/.cache/b10x-target/loom-w2-harness-loop-port` (assigned; now 2.2G, disk at 11G free)

### 7.
```findings
[
  {"file": "docs/design/harness-map.md", "line": 97, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "16140c9 put 'approval checkpoint' into the ToolPort bullet, so adversary2_seams_are_named_from_ported_crates (adversary2_harness_map.rs:275) now fails and the package gate is red at head; the reported green gate predates the commit"},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 181, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "run_loop passes LoopConfig unchanged, so a config with an output schema, delegation, skills or memories adds the loop's own tools to every request, contradicting 'Its tools are the catalogue's, whatever it says', the CHANGELOG and acceptance item 1; no in-tree caller sets those fields"},
  {"file": "crates/loom-executor/src/harness/governed.rs", "line": 509, "category": "acceptance", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a catalogue call made inside a delegate is selected and admitted, then the nested loop refuses the checkpoint, so the run ends CompletedLocalReasoning with an admitted selection never proposed, breaking the module docs and acceptance item 2; reachable only with delegation configured"}
]
```
