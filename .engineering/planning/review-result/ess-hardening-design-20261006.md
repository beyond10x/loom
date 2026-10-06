---
format: aep.planning-md/3
id: review-result:ess-hardening-design-20261006
kind: review-result
status: active
title: Independent design/spec review with a planted omission
relations:
- reviews: evaluation-plan:ess-hardening-20261006
revision: 1
---
Control detected the planted omission, and the corresponding finding clears against the real specification. All four strict validations passed: copied intake, real intake, Loom, and Commission.

The copy’s `.engineering/drafts/hardening/review-control/domains/routing.yaml:27` says:

> `variants: [ApprovalRequired, NothingAdmissible, StepBudget, NoLocalExecutor, Refused]`

This is **missing** the variant explicitly prescribed by `.engineering/planning/architecture-design/effect-isolation.md:280`:

> `StopReason::ConfinementUnavailable`

Real `ess/intake/domains/routing.yaml:27` includes `ConfinementUnavailable`. This is the corrected control result; it does not clear the separate findings below.

Paths in the table are repository-relative. `EI` means `.engineering/planning/architecture-design/effect-isolation.md`.

| # | Classification | Design citation and quote | Specification citation and quote | Assessment |
|---|---|---|---|---|
| 1 | missing | EI:307, “confined tests get no network”; EI:316, “no network and writes only to `target/`” | `ess/intake/domains/confinement.yaml:24`, `{name: network, type: String}`; :25, `{name: writable_scopes, type: "List<String>"}` | Shipped behavior has a typed vocabulary but no declared constraint or command outcome enforcing these limits. Arbitrary network strings and writable scopes remain structurally valid. This is a specification gap, not evidence of a runtime escape. |
| 2 | missing | EI:276, `ConfinementProfile` relation “requested by a `SliceRun`” | `ess/intake/domains/routing.yaml:93`, `name: intake.routing.SliceRun`; :97–105 fields end with `stop_reason` | No field or relation connects a run to its requested profile. Consequently, the ESS run record cannot distinguish the requested confinement posture. The design’s relation was not carried into the specification. |
| 3 | missing | EI:310, “every `tests.run` observation carries the `AppliedConfinement` record” | `ess/intake/domains/confinement.yaml:34`, `name: intake.confinement.AppliedConfinement`; `ess/commission/domains/responsibility.yaml:488–489`, `name: payload` / `type: Json` | The applied record exists as an unattached struct. There is no intake test-observation declaration or outcome requiring it. Keeping Commission’s payload domain-neutral is explicitly correct under EI:282–283; the missing obligation belongs on intake’s side. |
| 4 | missing | EI:305, “stops with a named `ConfinementRefusal` … unless … `--confinement none`”; EI:309, “re-execs itself … when no delegated cgroup is present” | `ess/intake/domains/confinement.yaml:7–9`, `ConfinementRefusal` enum; `ess/intake/domains/routing.yaml:113–115`, `initial: Ended`, `states: [Ended]`, `terminal: [Ended]` | The approved refusal/default/delegation behavior has no command, transition, guard, or outcome. Enum membership alone does not specify when refusal is mandatory or prohibit an automatic fallback. This is shipped behavior held by Rust tests rather than executable ESS. |
| 5 | missing | `docs/commission/contracts/frontier.md:65–66`, “the least-authority entry decides, whatever their order”; :71–74 specifies conflicting capabilities and deterministic reason encoding | `ess/commission/domains/responsibility.yaml:703–705`, `name: not-admitted` / `external: the current frontier refuses the action`; :710, “naming one capability” | The specification defers to opaque admission without declaring duplicate precedence, blank capabilities, conflict handling, or deterministic reason aggregation. These are already-built contract promises, not draft selector work. Existing hand-written tests may enforce them, but the ESS external clause cannot derive those cases. |
| 6 | stale mapping | `docs/commission/contracts/frontier.md:7–8`, “declared in `ess/domains/responsibility.yaml`” | Actual declaration: `ess/commission/domains/responsibility.yaml:6`, `domain: commission.responsibility`; its :2 still cites `docs/design/commission-design.md` and `docs/contracts/{commission-executor,frontier,evidence}.md` | Import left both directions of the Commission mapping stale. The cited root specification path does not exist; the cited root design/contract paths now require `docs/commission/`. Documentation traceability defect, not a runtime defect. |
| 7 | missing | `docs/contracts/loom-action-selection.md:88`, “Low confidence falls back to a stronger path.” | `ess/domains/run.yaml:478–481`, optional `confidence` and `strategy` inputs; :515–523, unconditional `selected` outcome stores them | **Already planned:** `story:confidence-fallback` is `draft`. Its supplied-threshold and fallback policy is not specified yet. Do not present this as a confinement regression or newly broken shipped guarantee. |
| 8 | missing | `docs/contracts/loom-action-selection.md:73`, “schema validation” before revalidation/execution | `ess/domains/run.yaml:55–61`, `CatalogueEntry` contains only `action` and `status`; :532–537, `RequestArguments` takes only request and selection identifiers | **Explicitly deferred:** `decision-blocker:action-argument-schema` remains `open`, and `story:argument-generator` excludes validation. The design sketch promises more than the current specification, but the repository already records the unresolved ownership decision. |
| 9 | missing | `docs/contracts/loom-action-selection.md:90`, “Selection telemetry should be available to Metaharness for evaluation.” | `ess/domains/run.yaml:714–729`, `Selections` view exposes identifiers, action, strategy, revision and state; no telemetry record | **Already planned:** `story:selection-telemetry` is `draft` and specifies the future record. This is a known delivery gap, not a new defect. |

Counts: **8 missing, 1 stale mapping, 0 contradicts, 0 spec-only, 0 unclear**. The planted missing declaration is additional control evidence and is excluded from these counts. Of the nine real findings, three are explicitly deferred/planned; five identify shipped behavior or its model relation that executable ESS does not hold; one concerns mapping drift.

I read all seven directly supplied design/contract documents end to end, and all four domain files in both directions. I did not turn declarations citing separate operator decisions, implementation-port stories, or Atlas ADRs into speculative `spec-only` findings merely because their justification is outside the supplied design set.

Unreached material: the separately linked, 2,800-line historical `docs/commission/history/beyond10x-agent-sdk-design-pre-commission-name.md`, private Atlas ADRs, and upstream Substrate/Harness designs. No sections of the seven direct input documents were left unread. No files were changed.
