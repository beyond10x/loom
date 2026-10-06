# ESS hardening pass, 2026-10-06

Reviewed source: `04a1a73a7073affb99ef730e8dedb7eabc862794` (the merged confinement change).
ESS CLI and Rust runner: **0.53.0**. Plan: `evaluation-plan:ess-hardening-20261006`.
This is a bounded audit, not a claim that all Loom behavior is held by executable ESS.
No release or live model run was performed.

The audit found five gaps between shipped behavior and its ESS declarations, one stale mapping
(corrected here), three previously deferred design gaps, and a reproducible ESS synthesis defect
filed as [ESS #464](https://github.com/beyond10x/ess/issues/464). No runtime confinement escape was
found or claimed by this pass; the earlier [real confinement qualification](2026-10-05-confined-tests.md)
is separate evidence.

## What the baseline actually exercises

All three specifications pass `ess specify validate --path <path> --strict-requires` and compile
to canonical JSON IR. The implementation baseline command was:

```console
cargo test -p b10x-loom-commission-conformance -p b10x-loom-executor --locked
```

It passed **606 Rust tests**, with two existing explicit ignores: refusal re-entry
(`story:interruption-recovery`) and missing-frontier revalidation
(`story:selection-revalidation`). Neither ignore establishes coverage.

| Specification | Model | Executable ESS coverage observed here |
|---|---|---|
| `ess/commission` | 4 commands, 10 entities, 1 view | Native Rust `CommissionTarget`: **13 passed, 0 failed/error/skipped/unsupported** |
| `ess/` | 9 commands, 5 entities, 3 views | **36 scenarios synthesized**, zero synthesis refusals. No whole-suite Rust target exists; `story:loom-ess-conformance` is draft. Ordinary Rust tests are not a substitute for that target. |
| `ess/intake` | 0 commands, 4 entities, 0 views | **0 scenarios**. Confinement structs generate Rust types but declare no executable confinement behavior. |

The [Commission baseline report](ess-hardening-2026-10-06/commission-baseline.json) is produced by
the real Rust adapter. The interpreted results below exercise ESS's model, not Loom's runtime.

## Technique 8: independent design review

An independent read-only agent followed the hardening skill's design-review brief in both directions.
The complete returned review, including both citations for every finding, is retained without
rewriting in `review-result:ess-hardening-design-20261006` under `.engineering/planning/review-result/`.
It covers seven direct design/contract inputs and all four domain files. Private Atlas ADRs,
upstream Substrate/Harness designs and the separately linked historical SDK design were not reviewed.

**Planted defect:** remove `ConfinementUnavailable` from a scratch copy of
`intake.routing.StopReason`. That copy still passed strict validation. The reviewer reported it
missing against the approved design's explicit variant at
`.engineering/planning/architecture-design/effect-isolation.md:280`, citing the copied declaration.
Against the real specification, which includes the variant, that finding clears. This control
proves the reviewer detects the planted omission; it does not erase the real findings.

| Finding | Classification and disposition |
|---|---|
| No network / `target/`-only writes are unconstrained strings and lists in the model | Spec gap; runtime tests hold these guarantees, executable ESS does not |
| `SliceRun` has no requested-profile field or relation | Spec gap against the approved design's relation |
| `AppliedConfinement` has no declared per-test-observation obligation | Spec gap on intake's side; Commission's generic JSON payload remains appropriate |
| Default refusal, explicit opt-out and delegation retry have no executable outcomes | Spec gap; enum membership does not declare when those outcomes are required |
| Duplicate frontier entries, conflicting/blank capabilities and deterministic reason aggregation end at an opaque external-admission clause | Spec gap against the already-built frontier contract |
| Commission's design/spec mapping still names paths from before the import | Stale mapping; corrected here in `frontier.md` and specification comments, with no semantic model change |
| Confidence fallback | Existing draft `story:confidence-fallback`, not a new runtime regression |
| Argument schema validation | Existing open `decision-blocker:action-argument-schema`; the argument-generator story excludes validation |
| Selection telemetry | Existing draft `story:selection-telemetry` |

The reviewer classified eight findings as missing and one as stale mapping; three of the eight
are already deferred. No contradictory or unjustified declaration was established. The five
remaining model gaps stay open in the verification record; this audit does not invent a new
observation model or silently decide the recorded ownership questions.

## Technique 7: semantic compatibility gate exercised

The native gate was run as:

```console
ess verify diff --from <before-spec> --to <after-spec> --fail-on breaking-or-unknown --format json
```

The previous specification was materialized from tag `0.1.0`. Its older ESS requirement emits a
version warning; comparison deliberately uses the current compiler without `--strict-requires`.
The comparison checks callers, readers and history. No acknowledgement was supplied.

| Comparison | Exit | Result |
|---|---:|---|
| Current intake against itself | 0 | Identical-spec green control |
| Current intake against the copy with `ConfinementUnavailable` removed | 4 | Planted removal named `type/intake.routing.StopReason/variant-removed/ConfinementUnavailable` |
| Current intake against the restored copy | 0 | Restored green control |
| Release `ess/` against current `ess/` | 0 | No semantic change |
| Release `ess/commission` against current `ess/commission` | 0 | No semantic change |
| Release `ess/intake` against current `ess/intake` | 4 | **Unknown** compatibility: `system/intake/unclassified-changed` accompanies the added domain |

[Red control](ess-hardening-2026-10-06/removed-variant.json),
[restored control](ess-hardening-2026-10-06/restored-variant.json), and release comparisons for
[Loom](ess-hardening-2026-10-06/loom-release-diff.json),
[Commission](ess-hardening-2026-10-06/commission-release-diff.json), and
[intake](ess-hardening-2026-10-06/intake-release-diff.json) retain exact classifications and digests.
The intake result is unresolved review evidence, not a declaration that the change breaks callers.
A permanent release comparison was not added to `task check`: choosing its release baseline and
reviewing the exact unknown change remain explicit follow-up work. No gate was weakened.

## Technique 1: specification mutation against the Rust implementation

```console
ess verify conform mutate --path ess/commission --emit <audit-dir>
ess verify conform mutate --collect <audit-dir> --report-out <report.json>
```

Between those commands, the [Rust audit driver](ess-hardening-2026-10-06/commission-runner.rs)
called the existing `b10x_loom_commission_conformance::run_suite` for every emitted suite and wrote
its returned `report.json` and `diagnostics.json` beside it. It contains no implementation model
or replacement target. Its CLI uses clap. Build the existing conformance crate and `loom-xtask`
with the workspace lockfile, then compile the driver with `rustc --edition 2024`, those crates'
`target/debug/deps` artifacts for `--extern`, and `-L dependency=target/debug/deps`. Arguments are
`--suite <suite.json> --report <report.json> --diagnostics <diagnostics.json>`.

**Baseline:** 13 passed, none unscored. **Results:** 10 mutants, **2 killed**, **0 survived**,
0 inconclusive/unwitnessed/equivalent, **8 stillborn**, and **3 unavailable sites**. Collection
exits **3**, so this is a partial audit, not a complete mutation pass or a 100% score.

The audit's red controls are the two runnable error substitutions:

| Defect | Named scenarios that kill it | Driver exit |
|---|---|---:|
| `ResumeRun/wrong-state`: `RunStateConflict` replaced by `ActionNeedsAuthority` | `ResumeRun/outcome/wrong-state`; `Run/state/Running/refuses/ResumeRun` | 1 |
| `SuspendRun/wrong-state`: same error substitution | `SuspendRun/outcome/wrong-state`; `Run/state/Suspended/refuses/SuspendRun` | 1 |

Scenario prefixes above are `commission.responsibility.`; the
[machine report](ess-hardening-2026-10-06/commission-mutations.json) carries full IDs. Each mutant
fails two of 13 scenarios. Returning to the original emitted suite passes
[13/13 again](ess-hardening-2026-10-06/commission-restored.json), exit 0.

The compiler rejects eight mutants: three outcomes made unobservable by dropping their only event,
three error substitutions leaving a payload for the old error, and two transitions creating
undeclared terminal states. Three single-event substitution sites have no compatible declared
alternative. These results say nothing about implementation resistance at those sites; they are
limitations of this audit rather than survivors, passing tests or runtime defects.

## Upstream defect found while checking the Loom model

A separate model-only attempt, `ess verify conform mutate --path ess --target interpreted`,
refused with **ESS-MUTATE-001**, exit 3. Mutation stopped there; no Loom-model score is claimed.
Running the unmodified suite with `ess verify conform run --path ess --target interpreted
--report-format 2 --report-out <report.json> --format json` reports
[34 passed and 2 failed](ess-hardening-2026-10-06/loom-interpreted.json), exit 1, with no skip,
error or unsupported scenario.

Both failures are the `RevalidateSelection/not-in-frontier` outcome/transition scenarios.
Their generated setup stores revision `194041`, but their revalidation input uses revision `1`.
The preceding `stale-revision` guard therefore fires before the injected external branch.
The [diagnostics](ess-hardening-2026-10-06/loom-interpreted-failures.json) show the wrong outcome
and event, while the resulting refusal state happens to agree.

**One-variable diagnostic control:** in a scratch copy of the generated suite only, set those two
revalidation input revisions to the value their own setup passed to `SelectAction`. The same
unchanged specification and interpreter then report
[36/36 passed](ess-hardening-2026-10-06/loom-witness-control.json), exit 0. This proves the witness
problem; it is not a committed suite repair, a product qualification, or permission to alter
outcome precedence. The original failing suite remains the audit baseline.

[ESS #464](https://github.com/beyond10x/ess/issues/464), filed through `b10x-gates api` as
`b10x-bot[bot]`, carries the source revision, commands, failing IDs and control. No ESS source or
other session's checkout was changed.

## Techniques not run

- **Random sequences (2):** no whole-system Rust target for Loom/intake, and the Loom reference
  baseline is red. Building a new explorer or implementing draft semantics exceeds this pass.
- **Caller replay (3):** no captured semantic command stream mapped to these ESS targets was used.
  The earlier live fixture transcript is not silently treated as such a stream.
- **Determinism (4):** repeatable baseline counts are not a determinism test. No nondeterministic
  implementation defect was planted, so this technique is unqualified.
- **Metamorphic relations (5):** no paired trace/refusal control was executed.
- **Exhaustive guard analysis (6):** the declared Commission admission decisions are external prose;
  a finite truth table would invent their semantics. Loom's stored/related guards were not analyzed
  exhaustively. No zero-gap claim is made.

Raw logs, emitted suites, canonical IR and scratch controls are retained in the managed worktree
archive. The committed JSON evidence is the compact record. The pass changes documentation paths
and records evidence; it changes no runtime behavior or semantic ESS declaration.

## Verification of this audit change

`task check` completed with exit 0 after the path corrections and audit artifacts were written:
strict specification gates, generated-model drift, conformance, dependency guards, formatting,
workspace Clippy/tests and documentation drift all passed. `task plan` completed with exit 0:
105 artifacts, valid. It reports one advisory because the independent design review is retained
as the skill's cited prose table rather than an AEP `findings` JSON block; AEP cannot enumerate
that review's individual findings. The immutable original was not rewritten to hide this limitation.
