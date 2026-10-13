<!--
  generated from intake v1
  model digest e5f987ee3613bbc2c1f7decd667a2b91fb1819f149cc970316814040874257c4
  contract digest 3bcee985eed31b4d9ea384c5bf3512a98619ba6016205bfe0291f1860171e18a
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — intake v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

51 capabilities: **51 generated**, **0 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

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

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
