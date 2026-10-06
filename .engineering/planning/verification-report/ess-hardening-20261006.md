---
format: aep.planning-md/3
id: verification-report:ess-hardening-20261006
kind: verification-report
status: draft
title: 'ESS hardening: qualified controls, model gaps and upstream witness defect'
relations:
- verifies: evaluation-plan:ess-hardening-20261006
- informed_by: review-result:ess-hardening-design-20261006
revision: 2
---
## Result

Bounded ESS hardening of Loom at `04a1a73a7073affb99ef730e8dedb7eabc862794` with ESS 0.53.0.
Full commands, findings, controls and technique limits: [qualification report](../../../docs/qualification/2026-10-06-ess-hardening.md).

- Baseline: 606 Rust tests passed, two existing explicit ignores. Native Commission conformance: 13 passed and no failed/error/skipped/unsupported scenarios. Root Loom synthesizes 36 scenarios without a whole-suite Rust target; intake synthesizes zero.
- Design review control detected a removed `ConfinementUnavailable` variant; actual spec clears that control. Immutable review: `review-result:ess-hardening-design-20261006`.
- Five shipped-model gaps remain open: confinement limits, requested-profile attachment, applied-observation obligation, default/refusal/delegation outcomes, and explicit frontier-admission semantics. The stale Commission mapping paths were fixed. Confidence fallback, argument schemas and telemetry were already recorded as deferred work.
- Semantic diff control: variant removal exits 4; restored copy exits 0. Release comparisons for Loom/Commission pass; intake's added-domain change remains unknown, exit 4. No acknowledgement was invented.
- Commission mutation: 2 killed, 0 survived, 8 stillborn, 3 unavailable sites; aggregate exit 3. Both runnable mutants fail two named scenarios each; the original suite returns to 13/13. This is partial evidence, not full mutation qualification.
- ESS's unmodified Loom interpreted suite reports 34 passed and 2 failed. The generated missing-frontier witnesses accidentally satisfy an earlier stale-revision guard. Aligning only their two revision inputs in a scratch copy gives 36/36. Filed upstream as [ESS #464](https://github.com/beyond10x/ess/issues/464); no product workaround committed.
- Random sequences, caller replay, determinism, metamorphic relations and exhaustive guard analysis were not qualified; the report states each reason.

## Changes and evidence

Only documentation mapping comments and audit records change; no semantic specification or runtime behavior changes. No new noun is introduced. This is exempt from a behavioral specification-first commit because the specification edits only correct source-path comments.

Machine reports and the Rust driver are under `docs/qualification/ess-hardening-2026-10-06/`. Raw logs, emitted suites and IR are retained with the managed tree's archive. No live model call, release, private design publication or change to another session's checkout occurred.
