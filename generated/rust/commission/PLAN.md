<!--
  generated from commission v1
  model digest 1ba42c043f9934dcbbb38e6d540f230eda0871defa63756e7c3db7d01b3050c3
  contract digest 9f2ffaa60ef43fd8f3ad719e5f980e33a4fc329e9ba4c5a990bbcfa1579a3135
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — commission v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

96 capabilities: **93 generated**, **3 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `commission.responsibility.ActionBinding.State` |
| domain type | `commission.responsibility.ActionBindingKey` |
| domain type | `commission.responsibility.ActionRequest.State` |
| domain type | `commission.responsibility.ActionRequestId` |
| domain type | `commission.responsibility.ActionStatus` |
| domain type | `commission.responsibility.Admission` |
| domain type | `commission.responsibility.AdmissionNeedsAuthority` |
| domain type | `commission.responsibility.AdmissionRefused` |
| domain type | `commission.responsibility.Agent.State` |
| domain type | `commission.responsibility.AgentId` |
| domain type | `commission.responsibility.AgentRevision.State` |
| domain type | `commission.responsibility.AgentRevisionId` |
| domain type | `commission.responsibility.AuthorityContext` |
| domain type | `commission.responsibility.AuthorityDecision.State` |
| domain type | `commission.responsibility.AuthorityDecisionId` |
| domain type | `commission.responsibility.AuthorityVerdict` |
| domain type | `commission.responsibility.AuthorityVerdictApprovalRequired` |
| domain type | `commission.responsibility.AuthorityVerdictDeny` |
| domain type | `commission.responsibility.Case.State` |
| domain type | `commission.responsibility.CaseId` |
| domain type | `commission.responsibility.Commission.State` |
| domain type | `commission.responsibility.CommissionId` |
| domain type | `commission.responsibility.CompletionDetermination` |
| domain type | `commission.responsibility.CompletionDeterminationComplete` |
| domain type | `commission.responsibility.ConnectorAttemptId` |
| domain type | `commission.responsibility.ConnectorAuditRef` |
| domain type | `commission.responsibility.ConnectorCredentialRef` |
| domain type | `commission.responsibility.ConnectorEndpoint.State` |
| domain type | `commission.responsibility.ConnectorEndpointUrl` |
| domain type | `commission.responsibility.ConnectorInstanceId` |
| domain type | `commission.responsibility.ConnectorOperationEffect` |
| domain type | `commission.responsibility.ConnectorOperationId` |
| domain type | `commission.responsibility.EffectOutcome` |
| domain type | `commission.responsibility.EffectOutcomePerformed` |
| domain type | `commission.responsibility.EffectOutcomeRefused` |
| domain type | `commission.responsibility.Evidence.State` |
| domain type | `commission.responsibility.EvidenceId` |
| domain type | `commission.responsibility.ExecutorOutcome` |
| domain type | `commission.responsibility.ExecutorOutcomeCaseMoved` |
| domain type | `commission.responsibility.ExecutorOutcomeNeedsHumanJudgment` |
| domain type | `commission.responsibility.ExecutorOutcomeProposedAction` |
| domain type | `commission.responsibility.ExecutorOutcomeSuspended` |
| domain type | `commission.responsibility.Frontier.State` |
| domain type | `commission.responsibility.FrontierAction` |
| domain type | `commission.responsibility.FrontierClaim` |
| domain type | `commission.responsibility.FrontierId` |
| domain type | `commission.responsibility.FrontierObligation` |
| domain type | `commission.responsibility.GovernorError` |
| domain type | `commission.responsibility.HumanDecisionRequest` |
| domain type | `commission.responsibility.Observation.State` |
| domain type | `commission.responsibility.ObservationId` |
| domain type | `commission.responsibility.PrincipalId` |
| domain type | `commission.responsibility.ProposedActionArguments` |
| domain type | `commission.responsibility.Run.State` |
| domain type | `commission.responsibility.RunId` |
| domain type | `commission.responsibility.RunOutcome` |
| domain type | `commission.responsibility.RunOutcomeAwaitingApproval` |
| domain type | `commission.responsibility.RunOutcomeCaseMovedOn` |
| domain type | `commission.responsibility.RunOutcomeCompleted` |
| domain type | `commission.responsibility.RunOutcomeNeedsAuthority` |
| domain type | `commission.responsibility.RunOutcomeNeedsExternalEvidence` |
| domain type | `commission.responsibility.RunOutcomeNeedsHumanJudgment` |
| domain type | `commission.responsibility.RunOutcomeSuspended` |
| domain type | `commission.responsibility.SuspensionReason` |
| domain type | `commission.responsibility.Truth` |
| domain type | `commission.responsibility.Unit` |
| entity lifecycle | `commission.responsibility.ActionBinding` |
| entity lifecycle | `commission.responsibility.ActionRequest` |
| entity lifecycle | `commission.responsibility.Agent` |
| entity lifecycle | `commission.responsibility.AgentRevision` |
| entity lifecycle | `commission.responsibility.AuthorityDecision` |
| entity lifecycle | `commission.responsibility.Case` |
| entity lifecycle | `commission.responsibility.Commission` |
| entity lifecycle | `commission.responsibility.ConnectorEndpoint` |
| entity lifecycle | `commission.responsibility.Evidence` |
| entity lifecycle | `commission.responsibility.Frontier` |
| entity lifecycle | `commission.responsibility.Observation` |
| entity lifecycle | `commission.responsibility.Run` |
| command contract | `commission.responsibility.ResumeRun` |
| command behaviour | `commission.responsibility.ResumeRun` |
| command contract | `commission.responsibility.RevalidateActionRequest` |
| command contract | `commission.responsibility.StartRun` |
| command contract | `commission.responsibility.SuspendRun` |
| event type | `commission.responsibility.RunResumed` |
| event type | `commission.responsibility.RunStarted` |
| event type | `commission.responsibility.RunSuspended` |
| error type | `commission.responsibility.ActionNeedsAuthority` |
| error type | `commission.responsibility.ActionNotAdmitted` |
| error type | `commission.responsibility.ActionRequestStale` |
| error type | `commission.responsibility.RunStateConflict` |
| error type | `commission.responsibility.RunStorageFailed` |
| view type | `commission.responsibility.RunStates` |
| view query | `commission.responsibility.RunStates` |

## Ports — yours to provide

What the specification fully determines is generated; what it cannot determine is an obligation. A generated command behaviour or view query reads and writes through the ports below, and they are yours to provide: synthesis generates each port's contract and never an implementation of one, so where instances live stays your decision.

| port | what it answers |
| --- | --- |
| storage | one per entity a generated behaviour or query reads or writes: the instance stored under an identity; storing, replacing and removing one; and every stored instance, in the order the store keeps them |
| context | where a generated behaviour asks it: the caller's attributes, every identity and value the specification says the implementation assigns, and whether each `external:` branch is taken |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `commission.responsibility.RevalidateActionRequest` | kept an obligation by the fields of error `commission.responsibility.ActionRequestStale`, which the specification gives no source, in `stale` | given `commission.responsibility.RevalidateActionRequest` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `stale` externally decided (the governor's current revision of the case is not the request's expected case revision), error `commission.responsibility.ActionRequestStale`; `not-admitted` externally decided (the current frontier refuses the action), error `commission.responsibility.ActionNotAdmitted`; `needs-authority` externally decided (the current frontier lists the action as ApprovalRequired, naming one capability), error `commission.responsibility.ActionNeedsAuthority`; `admitted` otherwise |
| command behaviour | `commission.responsibility.StartRun` | kept an obligation by the fields of error `commission.responsibility.RunStorageFailed`, which the specification gives no source, in `storage-failed` | given `commission.responsibility.StartRun` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `started` otherwise, creates `commission.responsibility.Run`, emits `commission.responsibility.RunStarted`; `storage-failed` externally decided (the run store cannot hold the new run), error `commission.responsibility.RunStorageFailed` |
| command behaviour | `commission.responsibility.SuspendRun` | kept an obligation by the fields of error `commission.responsibility.RunStorageFailed`, which the specification gives no source, in `storage-failed` | given `commission.responsibility.SuspendRun` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `suspended` otherwise, takes `suspend` of `commission.responsibility.Run`, emits `commission.responsibility.RunSuspended`; `wrong-state` from a state no declared move starts in, error `commission.responsibility.RunStateConflict`, and for an instance no record carries, without the error's fields; `storage-failed` externally decided (the run store cannot record the suspension), error `commission.responsibility.RunStorageFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
