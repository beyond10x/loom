---
format: aep.planning-md/3
id: review-result:docs-and-pin-adversary-q13
kind: review-result
status: active
title: Adversary pass over the docs layering and the metaharness 0.19.0 pin (wave 2026-10-26-q13)
relations:
- reviews: story:docs-agent-stack-layers
- reviews: story:metaharness-0-19-pin
revision: 1
---
## Review

Adversary pass (aep:adversary, opus), wave 2026-10-26-q13, over two units.

u1 `story:docs-agent-stack-layers` at `7c7bd671`: every ADR citation the unit added matches 0096,
0098 and 0108; no link added. Three pre-existing sentences still contradicted the ADRs (the landing
page's "Loom as the native one", loom-design.md's comparison list, Metaharness as the reader of
selection records on five pages). Two loom-docs guards added (`adv_q13_agent_stack_layers.rs`), both
red at `7c7bd671`; fixed by the coordinator in `bb07f5cc`, `cargo test -p loom-docs --locked`
28 passed 0 failed, `task docs-check` exit 0.

u4 `story:metaharness-0-19-pin` at `9986027d`: three guards added (`adv_q13_metaharness_pin.rs`),
green; mutants of `without_native_tools` M1, M2, M4 killed by `from_spec` and the guards, M3 (drop
the non-empty guard) survives as equivalent because Loom never plans the owned surface. One stale
doc corrected in `93ee8137`; the hand-built b10x-under-ask path escalated to
`story:session-executor-b10x-ask`. `cargo test -p b10x-loom-session-executor -p b10x-loom-sdk
-p b10x-loom-conformance --locked` exit 0, 184 executed.

```findings
[
{"file":"website/docs/index.md","line":67,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"pre-existing","message":"The landing page said Metaharness 'drives and compares harnesses, with Loom as the native one', the claim ADR 0096 removed (superseded 0071). Red: adv_q13_no_page_presents_loom_as_a_compared_harness. Fixed in bb07f5cc."},
{"file":"docs/design/loom-design.md","line":123,"category":"contract-drift","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"Section Metaharness listed 'Loom + Laya selector, Loom + reasoning selector, external coding harness' as what Metaharness compares, putting Loom among the harnesses. Fixed in bb07f5cc (a trial's work, ADR 0101)."},
{"file":"docs/contracts/loom-action-selection.md","line":133,"category":"contract-drift","severity":"warning","verdict":"needs-revision","origin":"pre-existing","message":"'Metaharness reads the records through Loom::selection_records' contradicts ADR 0098 D3 (metaharness never depends on loom-sdk); also action-selection.md:45, status.json:45, fast-selector.md:42, laya-fast-selection.md:74. Red: adv_q13_no_page_puts_metaharness_above_loom. Fixed in bb07f5cc."},
{"file":"crates/loom-session-executor/src/lib.rs","line":178,"category":"contract-drift","severity":"note","verdict":"needs-revision","origin":"introduced","message":"SpecError::DecisionModeUnenforced said b10x-harness runs only under observe, 'the one mode metaharness runs it in'; metaharness 0.19.0 runs b10x under frame and ask too. Doc and the comment at tests/adversary_w20261023w1_u2.rs:351 corrected in 93ee8137. The pub `tools` field with Ask on Kind::B10x now starts a session that decide_among answers with Claude file-tool rules; no in-repo caller does this and nothing tests it: escalated to story:session-executor-b10x-ask."}
]
```
