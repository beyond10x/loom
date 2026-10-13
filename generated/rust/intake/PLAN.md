<!--
  generated from intake v1
  model digest 58fc59c73e160b3816e4c2add829c8c5a6c05ea1c6901838a7d61a911e157818
  contract digest c7bf5a36bfa3d72e856cecfa8dd5cf67906769eb0ed8049fb9d732fc4c997443
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — intake v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

66 capabilities: **64 generated**, **2 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

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
| domain type | `intake.routing.ActionRequestId` |
| domain type | `intake.routing.AgentRevisionId` |
| domain type | `intake.routing.CaseId` |
| domain type | `intake.routing.CatalogDigest` |
| domain type | `intake.routing.CommissionId` |
| domain type | `intake.routing.ExtractedReference.State` |
| domain type | `intake.routing.Grant.State` |
| domain type | `intake.routing.Intent.State` |
| domain type | `intake.routing.IntentId` |
| domain type | `intake.routing.MergeHandoff.State` |
| domain type | `intake.routing.ProtocolPick.State` |
| domain type | `intake.routing.ReferenceKind` |
| domain type | `intake.routing.ResumeContext.State` |
| domain type | `intake.routing.SliceRun.State` |
| domain type | `intake.routing.StopReason` |
| entity lifecycle | `intake.routing.ExtractedReference` |
| entity lifecycle | `intake.routing.Grant` |
| entity lifecycle | `intake.routing.Intent` |
| entity lifecycle | `intake.routing.MergeHandoff` |
| entity lifecycle | `intake.routing.ProtocolPick` |
| entity lifecycle | `intake.routing.ResumeContext` |
| entity lifecycle | `intake.routing.SliceRun` |
| command contract | `intake.routing.DropResumeContext` |
| command contract | `intake.routing.RecordMergeHandoff` |
| event type | `intake.routing.MergeHandedOff` |
| error type | `intake.routing.HeadAlreadyHandedOff` |
| error type | `intake.routing.MergeHandoffAlreadyRecorded` |
| error type | `intake.routing.MergeHandoffStorageFailed` |
| error type | `intake.routing.ResumeContextNotFound` |
| error type | `intake.routing.ResumeContextStorageFailed` |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `intake.routing.DropResumeContext` | kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed` | given `intake.routing.DropResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `dropped` otherwise, deletes `intake.routing.ResumeContext`; `not-found` for an identity no record carries, error `intake.routing.ResumeContextNotFound`; `storage-failed` externally decided (the resume context store cannot remove the context), error `intake.routing.ResumeContextStorageFailed` |
| command behaviour | `intake.routing.RecordMergeHandoff` | kept an obligation by a guard over the rows a selector selects (`when_related: {entity, where, exists | count | forall}`), in `head-already-handed-off` | given `intake.routing.RecordMergeHandoff` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `already-recorded` for an identity a record already carries, error `intake.routing.MergeHandoffAlreadyRecorded`; `head-already-handed-off` when, of the `intake.routing.MergeHandoff` rows satisfying `(commission_id == input.commission_id and head == input.head)`, at least one row is selected, error `intake.routing.HeadAlreadyHandedOff`; `recorded` otherwise, creates `intake.routing.MergeHandoff`, emits `intake.routing.MergeHandedOff`; `storage-failed` externally decided (the state directory cannot record the hand-off), error `intake.routing.MergeHandoffStorageFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
