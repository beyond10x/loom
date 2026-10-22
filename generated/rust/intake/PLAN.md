<!--
  generated from intake v1
  model digest 4ed9220e25a096d3ee91fdabe8d0f6b9cdd3b59b5a826a2dfa07924bac771607
  contract digest 0d10146dd4b2743e959d71f4eeb3cfb293bf2dba47c285732ceb93672da1b95c
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — intake v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

96 capabilities: **93 generated**, **3 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `intake.bindings.ActionBinding` |
| domain type | `intake.bindings.ActionBindings` |
| domain type | `intake.bindings.CommandEvidence` |
| domain type | `intake.confinement.AppliedConfinement` |
| domain type | `intake.confinement.Backend` |
| domain type | `intake.confinement.ChildExit` |
| domain type | `intake.confinement.ChildOutput` |
| domain type | `intake.confinement.ChildStream` |
| domain type | `intake.confinement.ConfinementProfile` |
| domain type | `intake.confinement.ConfinementRefusal` |
| domain type | `intake.confinement.EnvironmentEntry` |
| domain type | `intake.confinement.ReadOnlyFile` |
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
| domain type | `intake.events.SessionEvent` |
| domain type | `intake.events.SessionModel` |
| domain type | `intake.events.SessionToolCall` |
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
| domain type | `intake.routing.CaseInputs` |
| domain type | `intake.routing.CatalogDigest` |
| domain type | `intake.routing.ChangeKind` |
| domain type | `intake.routing.CommissionId` |
| domain type | `intake.routing.ExtractedReference.State` |
| domain type | `intake.routing.Grant.State` |
| domain type | `intake.routing.Intent.State` |
| domain type | `intake.routing.IntentId` |
| domain type | `intake.routing.MandateApprovalEntry` |
| domain type | `intake.routing.MandateApprovalId` |
| domain type | `intake.routing.MandateApprovalRecorded` |
| domain type | `intake.routing.MandateApprovedDecision` |
| domain type | `intake.routing.MergeHandoff.State` |
| domain type | `intake.routing.ProtocolPick.State` |
| domain type | `intake.routing.ReferenceKind` |
| domain type | `intake.routing.ResumeContext.State` |
| domain type | `intake.routing.Risk` |
| domain type | `intake.routing.SliceEvidence.State` |
| domain type | `intake.routing.SliceEvidenceKind` |
| domain type | `intake.routing.SliceEvidenceResult` |
| domain type | `intake.routing.SliceRun.State` |
| domain type | `intake.routing.StopReason` |
| entity lifecycle | `intake.routing.ExtractedReference` |
| entity lifecycle | `intake.routing.Grant` |
| entity lifecycle | `intake.routing.Intent` |
| entity lifecycle | `intake.routing.MergeHandoff` |
| entity lifecycle | `intake.routing.ProtocolPick` |
| entity lifecycle | `intake.routing.ResumeContext` |
| entity lifecycle | `intake.routing.SliceEvidence` |
| entity lifecycle | `intake.routing.SliceRun` |
| command contract | `intake.routing.DropResumeContext` |
| command contract | `intake.routing.KeepResumeContext` |
| command contract | `intake.routing.RecordMergeHandoff` |
| event type | `intake.routing.MergeHandedOff` |
| event type | `intake.routing.ResumeContextKept` |
| error type | `intake.routing.HeadAlreadyHandedOff` |
| error type | `intake.routing.MergeHandoffAlreadyRecorded` |
| error type | `intake.routing.MergeHandoffStorageFailed` |
| error type | `intake.routing.ResumeContextAlreadyKept` |
| error type | `intake.routing.ResumeContextConfigAndCatalog` |
| error type | `intake.routing.ResumeContextNotFound` |
| error type | `intake.routing.ResumeContextStorageFailed` |
| view type | `intake.routing.MergeHandoffs` |
| view query | `intake.routing.MergeHandoffs` |
| view type | `intake.routing.ResumeContexts` |
| view query | `intake.routing.ResumeContexts` |

## Ports — yours to provide

What the specification fully determines is generated; what it cannot determine is an obligation. A generated command behaviour or view query reads and writes through the ports below, and they are yours to provide: synthesis generates each port's contract and never an implementation of one, so where instances live stays your decision.

| port | what it answers |
| --- | --- |
| storage | one per entity a generated behaviour or query reads or writes: the instance stored under an identity; storing, replacing and removing one; and every stored instance, in the order the store keeps them |
| context | where a generated behaviour asks it: the caller's attributes, every identity and value the specification says the implementation assigns, and whether each `external:` branch is taken |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `intake.routing.DropResumeContext` | kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed` | given `intake.routing.DropResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `dropped` otherwise, deletes `intake.routing.ResumeContext`; `not-found` for an identity no record carries, error `intake.routing.ResumeContextNotFound`; `storage-failed` externally decided (the resume context store cannot remove the context), error `intake.routing.ResumeContextStorageFailed` |
| command behaviour | `intake.routing.KeepResumeContext` | kept an obligation by the fields of error `intake.routing.ResumeContextStorageFailed`, which the specification gives no source, in `storage-failed` | given `intake.routing.KeepResumeContext` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `config-and-catalog` when `(defined(config) and defined(catalog))`, error `intake.routing.ResumeContextConfigAndCatalog`; `already-kept` for an identity a record already carries, error `intake.routing.ResumeContextAlreadyKept`; `kept` otherwise, creates `intake.routing.ResumeContext`, emits `intake.routing.ResumeContextKept`; `storage-failed` externally decided (the resume context store cannot keep the context), error `intake.routing.ResumeContextStorageFailed` |
| command behaviour | `intake.routing.RecordMergeHandoff` | kept an obligation by a guard over the rows a selector selects (`when_related: {entity, where, exists | count | forall}`), in `head-already-handed-off` | given `intake.routing.RecordMergeHandoff` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `already-recorded` for an identity a record already carries, error `intake.routing.MergeHandoffAlreadyRecorded`; `head-already-handed-off` when, of the `intake.routing.MergeHandoff` rows satisfying `(commission_id == input.commission_id and head == input.head)`, at least one row is selected, error `intake.routing.HeadAlreadyHandedOff`; `recorded` otherwise, creates `intake.routing.MergeHandoff`, emits `intake.routing.MergeHandedOff`; `storage-failed` externally decided (the state directory cannot record the hand-off), error `intake.routing.MergeHandoffStorageFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
