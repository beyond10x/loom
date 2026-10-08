---
format: aep.planning-md/3
id: review-result:adversary-w7-20261008-loom-control-plane-storage-pass-1
kind: review-result
status: active
title: Wave 2026-10-08-w7 adversary, loom story:control-plane-storage, pass 1
relations:
- reviews: story:control-plane-storage
revision: 1
---
# Wave 2026-10-08-w7 adversary, loom story:control-plane-storage, pass 1

Tree `loom-20261008-w7-cps` at `de25b1e` (base `7a48bb5`). Free disk was 24G against the wave's
25G floor, so the pass built nothing; the red case is an existing test, shown by a probe on the
prebuilt xtask binary.

- Red, introduced: `crates/loom-commission-xtask/tests/checks.rs:266`,
  `no_hand_model_refuses_a_hand_written_run_outcome_and_run_command_types`, lists
  `("pub trait", "Context")`; the regenerated commission crate no longer declares `Context`, so
  `no-hand-model` accepts a hand-written `pub trait Context {}` (refused at base `7a48bb5`, accepted
  at `de25b1e`). `task check` runs it through `cargo test --workspace`. Fix named: swap `Context`
  for `RunStorageFailed` in that list.
- Note, introduced: `crates/loom-commission/src/outcome.rs:143`, the `RunStore` doc still calls it
  the storage and context ports of the generated run commands.
- Note, infeasible here: `generated/rust/commission/src/behaviour.rs:44`, the generated doc names
  `TryContext` and a legacy `Context` adapter that the regenerated crate no longer contains; only
  the ess generator can change it.

Attacked without a break: failed `start_run` gives `run_id: None` and reaches no executor,
authority or effect port (`durable_run_storage.rs:793`); `suspend` maps `StorageFailed` to
`RunStorage` before the `NotSuspended` fallback (`runtime.rs:314`), never `Obligation`;
`RunStore`'s obligations keep id uniqueness, wrong-state outcomes and events
(`adversary_run_outcomes.rs:308`, `:330`; `run_outcomes.rs:320`, `:352`); the conformance
`ScenarioStore` forcing fires once for the named command only; the changed assertions in
`adversary2_run_conformance.rs` are not weakened (9 to 11 scenarios matches the 15 synthesized,
2 of them `storage-failed`); no remaining caller uses the commission crate's `Context`,
`TryContext` or `unmet_context`.

```findings
[{"file": "crates/loom-commission-xtask/tests/checks.rs", "line": 266, "category": "contract-drift", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the regenerated commission crate no longer declares trait Context, so no-hand-model accepts a hand-written `pub trait Context {}` and this existing test fails at that entry (probe on the prebuilt xtask binary: refused at base 7a48bb5, accepted at HEAD); loom-commission-xtask was never built or tested in this unit"}, {"file": "crates/loom-commission/src/outcome.rs", "line": 143, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "RunStore doc still calls it the storage and context ports of the generated run commands, though it now implements the StartRun and SuspendRun obligations and no Context exists"}, {"file": "generated/rust/commission/src/behaviour.rs", "line": 44, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the generated Generated<P> doc names TryContext and a legacy Context adapter that the regenerated crate no longer contains; fixable only in the ess generator"}]
```
