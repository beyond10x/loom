---
format: aep.planning-md/3
id: story:result-references
kind: story
status: implemented
title: Reuse inspected results through bounded previews and exact references
relations:
- decomposes: epic:loom-native-harness
- serves: vision:O3
- serves: vision:O1
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: CHANGELOG.md
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/loom-intake-slice
- confidence: cited
  path: docs/design/result-references.md
- confidence: cited
  path: docs/qualification/2026-10-06-result-references.md
- confidence: cited
  path: ess/intake
- confidence: cited
  path: generated/rust/intake
- confidence: cited
  path: website
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-06T16:28:19Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-06T16:28:19Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"approval":1}}}
- {from: "active", to: "implemented", at: "2026-10-06T16:53:17Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":3,"approval":1}}, executor: "agent:loom-result-references"}
---
## Outcome

A Loom software-change run retains immutable inspected content, exposes bounded previews and handles, and resolves whole/range/JSON-pointer references and text compositions into ordinary edit arguments before Commission admission. A local recorded-model end-to-end test proves exact destination bytes and a smaller subsequent model request. Unknown, corrupt, invalid, cross-run and over-budget selections refuse before effects. No quality or billed-token saving is inferred from fixture byte savings.

## Authorization and ownership

The operator asked to implement the researched first priority, then selected the custom harness and explicitly named `b10x/loom`. This authorizes the implementation and its local verification without another proposal stop. Source owner: `crates/loom-intake-slice/src/selector.rs:70`, `effect.rs:128`, and Commission admission in `crates/loom-commission/src/runtime.rs`. Context Lens remains an observer. This is one bounded unit; no multi-story decomposition panel applies. AEP implementing skill 0.20.1 informs independent review; this work lands through a bot pull request outside a multi-story wave.

## ESS first

`ess/intake/domains/results.yaml` declares the typed data contract first (spec-only commit `0c692ab`). `task intake-drift` must fail on that commit because generated Rust lacks the new domain. Regeneration and implementation follow; both existing ESS gates remain prerequisites. The intake specification declares data, not effect commands; its synthesized suite has zero scenarios, so runtime acceptance is established by named integration tests, not an empty conformance claim.

## Acceptance

`result_references` tests cover immutable capture, source digest checks, UTF-8 boundaries, one-based inclusive LF lines retaining CRLF, RFC6901 escaping and exact JSON numeric lexemes, missing versus null, invalid selectors, bounded compositions, unavailable scope, and unchanged literal arguments. `result_reference_workflow` runs inspect -> bounded briefing -> reference generation -> ordinary governed edit using a recorded model. It verifies final bytes, checks a forged/stale reference cannot reach a write, and measures serialized model input and generated argument bytes separately.

Captured test output is explicitly partial because existing runners retain tails. Never claim recovery of discarded output. Storage is bounded and run-scoped; it survives transcript trimming within that run and expires with its briefing. No cross-run or cross-agent sharing is introduced. Retrieval is bounded deterministic context access within argument generation, not a new consequential frontier action.

## Scope

- cited: `ess/intake/`, `generated/rust/intake/`, `crates/loom-intake-slice/src/selector.rs`, `crates/loom-intake-slice/src/lib.rs`, and intake tests.
- inferred: new `crates/loom-intake-slice/src/results.rs` owns storage/selection, with generated domain types. `docs/design/result-references.md`, README, AGENTS, CHANGELOG and website status/guide describe the usable contract.
- explicitly excluded: provider wires, Commission authority semantics, upstream test-runner full-log retention, ported AgentLoop, live paid model experiments, unrelated history pruning or instruction rewrites.

## Gate and recovery

Run package tests early, then `task check`, `task plan`, and `task website` for changed site content. Use isolated managed tree `loom-result-references`, branch `impl/result-references`, tree-local `target/`, scratch `.engineering/drafts/result-references/`. Initial free disk 15 GiB. No remote release or version tag. Commit/publish as bot; archive local recovery if publication is unavailable. Independent review follows the integrated workflow.
