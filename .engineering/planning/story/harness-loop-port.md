---
format: aep.planning-md/3
id: story:harness-loop-port
kind: story
status: implemented
title: Wire the ported Harness loop to Loom's projection, selection and revalidation
summary: The ported loop's tool list is the projected catalogue and every tool call goes through selection, arguments and revalidation; the port itself is story:harness-crate-port.
refs:
- provider: taskboard
  reference: L-002
relations:
- decomposes: epic:loom-native-harness
- depends_on: story:agent-executor
- depends_on: story:harness-module-map
- depends_on: story:selection-revalidation
- serves: vision:O1
- serves: vision:O3
- serves: vision:governed-autonomy
- depends_on: story:run-pipeline-skeleton
- depends_on: story:harness-crate-port
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: crates/loom-executor/src/harness/
- confidence: cited
  path: crates/loom-executor/src/harness/governed.rs
- confidence: cited
  path: crates/loom-executor/src/harness/mod.rs
- confidence: cited
  path: crates/loom-executor/src/lib.rs
- confidence: cited
  path: crates/loom-executor/src/session.rs
- confidence: cited
  path: crates/loom-executor/tests/harness_loop_port.rs
- confidence: cited
  path: docs/design/harness-map.md
- confidence: cited
  path: website/data/status.json
- confidence: cited
  path: website/docs/concepts/commission-and-harness.md
revision: 32
transitions:
- {from: "draft", to: "proposed", at: "2026-10-06T11:44:41Z", actor: "human:timo", revision: 19, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-06T11:44:41Z", actor: "human:timo", revision: 20, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "active", to: "implemented", at: "2026-10-06T17:15:20Z", actor: "human:timo", revision: 32, decided_on: {"recorded":{"test_result":1,"review_outcome":9,"verification":1}}}
---
## Outcome

The Harness loop ported by `story:harness-crate-port` calls Loom's own pieces at Harness's seams:
the catalogue from `story:frontier-projection` is what each request's tool list carries (through
the `TurnEnvironmentProvider` seam, `harness-loop/src/environment.rs:47` at `798325f0`), and a
model's tool call goes through selection, argument generation and revalidation
(`story:action-selector`, `story:argument-generator`, `story:selection-revalidation`) before it
becomes a `ProposedAction`.

On 2026-10-04 the porting half of this story (the five crates, the provider-wire fixtures, the
licence, Harness unchanged; former acceptance items 1, 5 and 6) moved to
`story:harness-crate-port`, so the port no longer waits for the selection chain. This story keeps
the wiring and its acceptance test, `ported_loop_round_trip`.

## Shared surface

Depends on `story:harness-crate-port` (the ported loop), `story:selection-revalidation` (the
pipeline a tool call goes through; it reaches `story:frontier-projection`,
`story:action-selector` and `story:argument-generator` through it), `story:run-pipeline-skeleton`
and `story:harness-module-map`. These are behavioural edges: acceptance items 1 to 3 below exercise
the projection, the selector, the generator and the refusal path. It edits the ported
`crates/loom/src/harness/` tree and the executor pipeline in `crates/loom/src/lib.rs`.
`story:compaction-contract` and `story:interruption-recovery` depend on it.

## ESS first

- **Specification change:** none — wiring adds no domain noun; it uses
  `loom.run.ProjectCatalogue`, `SelectAction`, `RequestArguments` and `RevalidateSelection`, declared
  by `story:run-pipeline-skeleton`. If wiring meets a domain noun, the story stops and reports it.
- **Red test:** the first commit adds `ported_loop_round_trip` in
  `crates/loom/tests/harness_loop_port.rs`; it fails on that commit because the ported loop's tool
  list is still the `ToolPort` attached at startup, not the projected catalogue (item 1), and a tool
  call does not reach the selector (item 2).

## Scope

Derived 2026-10-06 by `story-scoper` at `04a1a73`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Path mapping:** the story's `crates/loom/` paths are `crates/loom-executor/` since
  `story:crate-names`: `crates/loom/src/harness/` → `crates/loom-executor/src/harness/`,
  `crates/loom/src/lib.rs` → `crates/loom-executor/src/lib.rs`,
  `crates/loom/tests/harness_loop_port.rs` → `crates/loom-executor/tests/harness_loop_port.rs`,
  `crates/loom/tests/harness_port_contract.rs` → `crates/loom-executor/tests/harness_port_contract.rs`;
  Harness `harness-loop/src/environment.rs:47` → `crates/loom-executor/src/harness/turn_loop/environment.rs:49`,
  `harness-wire/src/port.rs:135` → `crates/loom-executor/src/harness/wire/port.rs:137`; the
  Commission fake governor is `crates/loom-commission-testkit/src/fake_governor.rs`, already a
  dev-dependency (`crates/loom-executor/Cargo.toml:23`), read and not edited — cited
- **Primary surface:** `crates/loom-executor`: the `AgentExecutor::run` pipeline and Loom's own
  `TurnEnvironmentProvider` and `ToolPort` implementations — cited (story § Scope)
- **Files:** `crates/loom-executor/src/lib.rs` (`impl AgentExecutor for Loom`, the executor
  pipeline) — cited
- **Files:** `crates/loom-executor/src/harness/` (the `TurnEnvironmentProvider` and `ToolPort`
  implementations that feed the projected catalogue and route tool calls into the pipeline) — cited
- **Files:** `crates/loom-executor/tests/harness_loop_port.rs` (new, `ported_loop_round_trip`) — cited
- **Symbols:** `TurnEnvironmentProvider` (`src/harness/turn_loop/environment.rs:49`), `ToolPort`
  (`src/harness/wire/port.rs:137`), `projection::project`, `ActionSelector`, `ArgumentGenerator`,
  `ProposedAction`, `loom.run.RecordTurn` (`ess/domains/run.yaml:320`) — cited
- **Also likely:** `crates/loom-executor/src/harness/mod.rs`: it declares the new Loom-owned module
  beside the five ported ones, because the ported subdirectories may not name Loom's modules
  (see Safety fact) — inferred
- **Also likely:** `crates/loom-executor/src/session.rs`: recording one `Turn` per completed turn
  through `RecordTurn` (the carried note from wave 2026-10-04-w15) needs `TurnStorage` and
  `SessionStorage` implementations, and no file in `crates/loom-executor/src` implements either
  yet — inferred
- **Documents:** none. `docs/design/harness-map.md:98-100` already says this story wires the
  `TurnEnvironmentProvider` seam, and no specification change is planned (§ ESS first) — cited
- **Confidence:** medium. The story names all three paths and the tree has them under the renamed
  crate, but it does not name the file inside `src/harness/` for the new wiring, and the
  turn-recording half has no site the story names — cited
- **Would collide with:** any unit editing `crates/loom-executor/src/lib.rs` (the executor
  pipeline), any unit adding or changing a module under `crates/loom-executor/src/harness/`, and
  any unit on session filing in `crates/loom-executor/src/session.rs` — inferred
- **Safety fact:** the bridge cannot live inside the five ported modules.
  `tests/adversary2_harness_port.rs:48-62` lets `turn_loop` name only `wire` and five external
  crates, refuses any `crate::<loom module>` path (`:455-456`), and fixes the ported file count at
  36 (`:637`). So the code that names `projection`, `selection` and `b10x_loom_commission` lands
  outside `harness/{wire,http,responses,messages,turn_loop}/`. The refusal in acceptance item 3
  already comes from `refuse_unadmitted` (`src/harness/turn_loop/mod.rs:1727`) once the
  environment's tool list is the catalogue. Step 2 (pointed at `file:line`), unproven — cited

Not established while scoping:

- **Whether `turn_loop/` itself has to change.** `turn_loop/mod.rs:2424-2441` rejects a turn's tool
  list unless every entry is exactly a tool the attached `ToolPort` offers, and no loop exit carries
  a `ProposedAction`. Wiring may need an edit to the ported loop, which the boundary test above
  constrains. Settle this before the story is proposed.
- The file name for the wiring code (a new file beside `src/harness/mod.rs`, or a top-level module).
- Revalidation's call site: depends on what `story:selection-revalidation` builds in `src/revalidation.rs`.
- Where `RecordTurn` lands: `session.rs` is a reading; `src/arguments.rs` (`RequestRecord`) or `lib.rs` could hold `Turn` storage instead.
- The software-change frontiers exist only as private functions in `crates/loom-executor/tests/agent_executor.rs:56,79`; sharing them touches that file.
- Harness's `provider_emulated.rs` suite (39 cases): port it here or record why not; `src/harness/mod.rs` already says it is not carried.

Confirmed by the implementor in wave 2026-10-06-w2 (the lines above are kept as scoped):

| scoped line | result |
|---|---|
| `src/harness/mod.rs` declares the new module | confirmed: `src/harness/mod.rs:45` `pub mod governed;` |
| `session.rs` needs `TurnStorage` and `SessionStorage`; none existed | confirmed: `TurnRecord` in `session.rs` from `:627`, with `RecordTurn`, `ResumeSession` and `FileSession` |
| Collides with `lib.rs`, harness modules, `session.rs` | confirmed: `lib.rs` `propose` (pipeline extracted from `AgentExecutor::run`), new `src/harness/governed.rs`, `session.rs` additions |
| Whether `turn_loop/` must change | settled: no file under `turn_loop/` changed; the tool port and environment provider share one per-turn catalogue list (`turn_loop/mod.rs:2436` check holds) and `LoopStop::AwaitingApproval` with its checkpoint is the exit |
| File for the wiring | settled: `crates/loom-executor/src/harness/governed.rs` |
| Where `RecordTurn` lands | settled: `governed.rs`, through `TurnRecord` in `session.rs` |
| Software-change frontiers | settled: copied into `tests/harness_loop_port.rs`; `agent_executor.rs` unedited |

Landed: `crates/loom-executor/src/{lib.rs,session.rs,harness/mod.rs,harness/governed.rs}`, four test files, `CHANGELOG.md`, `AGENTS.md`, `website/data/status.json`, `website/docs/concepts/commission-and-harness.md`, `docs/design/harness-map.md`.

## Constraints

Read-only on `beyond10x/harness`.

## Acceptance

The test `ported_loop_round_trip` in `crates/loom/tests/harness_loop_port.rs` passes and checks,
over a provider-emulated endpoint, with the Commission fake governor serving the software-change
frontier:

1. The tool list of every request equals the catalogue projected from the frontier current when
   the request was assembled.
2. A scripted model reply that calls a catalogue action makes Loom return a `ProposedAction` for
   that action, and the model's tool call reached the selector and the argument generator exactly
   once each.
3. A scripted model reply that calls a name outside the catalogue is answered to the model as a
   refusal naming that name, and no `ProposedAction` is returned.

## Source

TASKBOARD L-002 (the executor carries Harness's loop); `epic:loom-native-harness` § Outcome; Atlas
ADR 0071 and 0072; `docs/design/harness-map.md` § Seams a port reuses; finding 2 of
`review-result:loom-native-harness-scope-r1`; the split of 2026-10-04 into
`story:harness-crate-port`.

## From wave 2026-10-04-w10 (harness-crate-port, adversary pass 1)

Harness `tests/provider_emulated.rs` at 798325f0 (39 cases across the two wires) was not carried: it
drives Python fake endpoints over a socket. Port it to Rust here, or record why not; the contract,
summary-request and transport tests were carried as `crates/loom/tests/harness_port_contract.rs`.

## Carried from story:session-transcript-streaming (wave 2026-10-04-w15)

`loom.run.RecordTurn` and `Turn.items` are declared, but the ported `session.rs` stores the whole
conversation as Harness does and records no per-turn `Turn`. Wiring the loop to the run records one
`Turn` per completed turn through `RecordTurn`, so the declared command has a caller.
