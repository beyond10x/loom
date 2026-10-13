<!--
  generated from intake v1
  model digest 6a863e68d9e21bd404ea8cd1c1da3b9677c04d000b3ba83ac143ad567a38235c
  contract digest 3c2bf8b3896629e198b3ad7d91d750b1b57d6d5df6039ce670e57cf6bedd69c6
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — intake v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

55 capabilities: **54 generated**, **1 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `intake.confinement.AppliedConfinement` |
| domain type | `intake.confinement.Backend` |
| domain type | `intake.confinement.ConfinementProfile` |
| domain type | `intake.confinement.ConfinementRefusal` |
| domain type | `intake.confinement.EnvironmentEntry` |
| domain type | `intake.confinement.ToolchainRoot` |
| domain type | `intake.context.ContextPolicy` |
| domain type | `intake.context.ContextReport` |
| domain type | `intake.context.ContextRequestMetric` |
| domain type | `intake.context.HistoryEvent` |
| domain type | `intake.context.RefusalState` |
| domain type | `intake.context.RequestPhase` |
| domain type | `intake.context.TestState` |
| domain type | `intake.context.WorkingState` |
| domain type | `intake.events.ApprovalEvent` |
| domain type | `intake.events.RouteEvent` |
| domain type | `intake.events.RunEvent` |
| domain type | `intake.events.RunEventLine` |
| domain type | `intake.events.SchemaVersion` |
| domain type | `intake.events.TerminalEvent` |
| domain type | `intake.events.ToolCallEvent` |
| domain type | `intake.events.TurnEvent` |
| domain type | `intake.events.UsageEvent` |
| domain type | `intake.protocols.ProtocolDefinition` |
| domain type | `intake.protocols.ProtocolSource` |
| domain type | `intake.protocols.SourceKind` |
| domain type | `intake.query.TimeObservation` |
| domain type | `intake.results.Capture` |
| domain type | `intake.results.Rendering` |
| domain type | `intake.results.ResultReference` |
| domain type | `intake.results.ResultSelector` |
| domain type | `intake.results.SelectionKind` |
| domain type | `intake.results.StoredResult` |
| domain type | `intake.routing.AcceptedRoute` |
| domain type | `intake.routing.AgentRevisionId` |
| domain type | `intake.routing.CaseId` |
| domain type | `intake.routing.CatalogDigest` |
| domain type | `intake.routing.CommissionId` |
| domain type | `intake.routing.ExtractedReference.State` |
| domain type | `intake.routing.Intent.State` |
| domain type | `intake.routing.IntentId` |
| domain type | `intake.routing.ProtocolPick.State` |
| domain type | `intake.routing.ReferenceKind` |
| domain type | `intake.routing.ResumeContext.State` |
| domain type | `intake.routing.SliceRun.State` |
| domain type | `intake.routing.StopReason` |
| entity lifecycle | `intake.routing.ExtractedReference` |
| entity lifecycle | `intake.routing.Intent` |
| entity lifecycle | `intake.routing.ProtocolPick` |
| entity lifecycle | `intake.routing.ResumeContext` |
| entity lifecycle | `intake.routing.SliceRun` |
| command contract | `intake.routing.DropResumeContext` |
| error type | `intake.routing.ResumeContextNotFound` |
| error type | `intake.routing.ResumeContextStorageFailed` |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `intake.routing.DropResumeContext` | kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed` | given `intake.routing.DropResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `dropped` otherwise, deletes `intake.routing.ResumeContext`; `not-found` for an identity no record carries, error `intake.routing.ResumeContextNotFound`; `storage-failed` externally decided (the resume context store cannot remove the context), error `intake.routing.ResumeContextStorageFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
