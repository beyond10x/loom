<!--
  generated from loom v1
  contract digest b8861e6c0dcd516ae6a61f772ec91a14f639811cafa340e3d466e8a1ef368f96
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — loom v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

415 capabilities: **399 generated**, **13 obligations**, **3 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `loom.agent.A2aCard` |
| domain type | `loom.agent.A2aExposure` |
| domain type | `loom.agent.A2aRefusalReason` |
| domain type | `loom.agent.ActionBudget` |
| domain type | `loom.agent.AgentId` |
| domain type | `loom.agent.AgentIdentity` |
| domain type | `loom.agent.AgentSpec` |
| domain type | `loom.agent.AgentState` |
| domain type | `loom.agent.Binding` |
| domain type | `loom.agent.BreachStep` |
| domain type | `loom.agent.BudgetLimit` |
| domain type | `loom.agent.ChannelSpec` |
| domain type | `loom.agent.ConfinementMode` |
| domain type | `loom.agent.ConnectorsBinding` |
| domain type | `loom.agent.DecisionMode` |
| domain type | `loom.agent.DeploySpec` |
| domain type | `loom.agent.EgressAperture` |
| domain type | `loom.agent.ExposeSpec` |
| domain type | `loom.agent.HttpBinding` |
| domain type | `loom.agent.HttpReadRefusalReason` |
| domain type | `loom.agent.HttpTrustRefusalReason` |
| domain type | `loom.agent.InnerLoop` |
| domain type | `loom.agent.KubernetesDeploy` |
| domain type | `loom.agent.KubernetesEgress` |
| domain type | `loom.agent.KubernetesIngress` |
| domain type | `loom.agent.KubernetesResources` |
| domain type | `loom.agent.Label` |
| domain type | `loom.agent.LimitBreachStep` |
| domain type | `loom.agent.Loops` |
| domain type | `loom.agent.OuterLoop` |
| domain type | `loom.agent.PolicySpec` |
| domain type | `loom.agent.RefusalReason` |
| domain type | `loom.agent.RunBudget` |
| domain type | `loom.agent.RunRefusalReason` |
| domain type | `loom.agent.RuntimeSpec` |
| domain type | `loom.agent.SendRefusalReason` |
| domain type | `loom.agent.ServeRefusalReason` |
| domain type | `loom.agent.SessionSpec` |
| domain type | `loom.agent.StateWhen` |
| domain type | `loom.agent.StopPolicy` |
| domain type | `loom.agent.ToolPolicy` |
| domain type | `loom.agent.WriteMode` |
| domain type | `loom.agent.WritesSpec` |
| domain type | `loom.authority.Answer` |
| domain type | `loom.authority.CaseResource` |
| domain type | `loom.authority.HostContext` |
| domain type | `loom.authority.MandateHost` |
| domain type | `loom.authority.Outcome` |
| domain type | `loom.bindings.ActionBudget` |
| domain type | `loom.bindings.ArtifactPath` |
| domain type | `loom.bindings.ArtifactRevision` |
| domain type | `loom.bindings.BindingsDocument` |
| domain type | `loom.bindings.BindingsRefusal` |
| domain type | `loom.bindings.BindingsRefusalReason` |
| domain type | `loom.bindings.BoundAction` |
| domain type | `loom.bindings.EvidenceEntry` |
| domain type | `loom.bindings.Executor` |
| domain type | `loom.bindings.ExecutorSpec` |
| domain type | `loom.bindings.ProducerIndependence` |
| domain type | `loom.bindings.ProgramRefusal` |
| domain type | `loom.bindings.ProgramRefusalReason` |
| domain type | `loom.bindings.ProgramRun` |
| domain type | `loom.bindings.RecordedEvidence` |
| domain type | `loom.bindings.ResultRule` |
| domain type | `loom.bindings.ResultSource` |
| domain type | `loom.bindings.RetryCondition` |
| domain type | `loom.bindings.ToolAllowList` |
| domain type | `loom.datasource.AdapterAlias` |
| domain type | `loom.datasource.ConnectionId` |
| domain type | `loom.datasource.ConnectorsCliConfig` |
| domain type | `loom.datasource.DataSource` |
| domain type | `loom.datasource.MutationClassification` |
| domain type | `loom.datasource.OperationId` |
| domain type | `loom.datasource.ReadKind` |
| domain type | `loom.datasource.ReadRefusal` |
| domain type | `loom.datasource.ReadResult` |
| domain type | `loom.datasource.SourceEntity` |
| domain type | `loom.datasource.SourceName` |
| domain type | `loom.datasource.WriteApproval` |
| domain type | `loom.datasource.WriteOperation` |
| domain type | `loom.datasource.WriteRefusal` |
| domain type | `loom.datasource.WriteResult` |
| domain type | `loom.decision.ActionId` |
| domain type | `loom.decision.ArtifactId` |
| domain type | `loom.decision.ArtifactRevisionCitation` |
| domain type | `loom.decision.AutonomyLevel` |
| domain type | `loom.decision.CapabilityId` |
| domain type | `loom.decision.CaseId` |
| domain type | `loom.decision.CheckVerdict` |
| domain type | `loom.decision.Citation` |
| domain type | `loom.decision.Consequence` |
| domain type | `loom.decision.DecisionClassId` |
| domain type | `loom.decision.DecisionOption` |
| domain type | `loom.decision.DecisionRequest.State` |
| domain type | `loom.decision.DecisionRequestId` |
| domain type | `loom.decision.EscalationSuppressedRecord` |
| domain type | `loom.decision.Grant` |
| domain type | `loom.decision.Ground` |
| domain type | `loom.decision.GroundSource` |
| domain type | `loom.decision.Holder` |
| domain type | `loom.decision.OptionLetter` |
| domain type | `loom.decision.OutcomeId` |
| domain type | `loom.decision.PathLineCitation` |
| domain type | `loom.decision.Principal` |
| domain type | `loom.decision.PrincipalKind` |
| domain type | `loom.decision.ProducedDocument` |
| domain type | `loom.decision.ProducedRecord` |
| domain type | `loom.decision.ProposerKind` |
| domain type | `loom.decision.RecordCitation` |
| domain type | `loom.decision.RecordStore` |
| domain type | `loom.decision.Revision` |
| domain type | `loom.decision.Strength` |
| domain type | `loom.decision.SuppressionCause` |
| domain type | `loom.decision.UnblockKind` |
| domain type | `loom.decision.Unblocks` |
| domain type | `loom.evaluation.ActionStatus` |
| domain type | `loom.evaluation.CaseSnapshot` |
| domain type | `loom.evaluation.ClaimValue` |
| domain type | `loom.evaluation.DecidedAction` |
| domain type | `loom.evaluation.DecidedClaim` |
| domain type | `loom.evaluation.DecidedObligation` |
| domain type | `loom.evaluation.EvaluationDecision` |
| domain type | `loom.evaluation.EvaluationInput` |
| domain type | `loom.evaluation.EvaluationRefusal` |
| domain type | `loom.evaluation.EvaluationRequest` |
| domain type | `loom.evaluation.EvidenceRecord` |
| domain type | `loom.evaluation.ExplicitDecision` |
| domain type | `loom.evaluation.ProtocolName` |
| domain type | `loom.governor.CompletionRule` |
| domain type | `loom.governor.StoredArtifact` |
| domain type | `loom.governor.StoredCase` |
| domain type | `loom.governor.StoredCaseId` |
| domain type | `loom.governor.StoredEvidence` |
| domain type | `loom.governor.StoredEvidenceData` |
| domain type | `loom.governor.StoredIndependence` |
| domain type | `loom.governor.StoredInput` |
| domain type | `loom.governor.StoredObservation` |
| domain type | `loom.metering.ActionUsage` |
| domain type | `loom.metering.CarriedUsage` |
| domain type | `loom.metering.EffectsUsage` |
| domain type | `loom.metering.RunUsageReport` |
| domain type | `loom.metering.StepUsage` |
| domain type | `loom.models.ModelRole` |
| domain type | `loom.models.ModelsConfig` |
| domain type | `loom.models.RoleModel` |
| domain type | `loom.plugin.Classification` |
| domain type | `loom.plugin.Cursor` |
| domain type | `loom.plugin.InboundItem` |
| domain type | `loom.plugin.Intent` |
| domain type | `loom.plugin.ItemFailures` |
| domain type | `loom.plugin.ItemId` |
| domain type | `loom.plugin.Objective` |
| domain type | `loom.plugin.PluginConfig` |
| domain type | `loom.plugin.PluginState` |
| domain type | `loom.plugin.Poll` |
| domain type | `loom.plugin.Projection` |
| domain type | `loom.plugin.Proposal` |
| domain type | `loom.plugin.ProposedCase` |
| domain type | `loom.plugin.RecordLine` |
| domain type | `loom.plugin.RecordOutcome` |
| domain type | `loom.plugin.SourceRead` |
| domain type | `loom.plugin.StepOutcome` |
| domain type | `loom.plugin.TurnResult` |
| domain type | `loom.plugin.TurnStep` |
| domain type | `loom.resource.AdmissionRefusalReason` |
| domain type | `loom.resource.AgentVariant` |
| domain type | `loom.resource.Boundary` |
| domain type | `loom.resource.BoundaryUpstream` |
| domain type | `loom.resource.CheckResult` |
| domain type | `loom.resource.CommandSpec` |
| domain type | `loom.resource.Condition` |
| domain type | `loom.resource.ConditionStatus` |
| domain type | `loom.resource.ConditionType` |
| domain type | `loom.resource.ConfigFile` |
| domain type | `loom.resource.ConfigMapKeyRef` |
| domain type | `loom.resource.ConfinementProfile` |
| domain type | `loom.resource.ConfinementProfileKind` |
| domain type | `loom.resource.ConnectorsBoundary` |
| domain type | `loom.resource.DeploymentAvailability` |
| domain type | `loom.resource.DeploymentStatus` |
| domain type | `loom.resource.EnvVar` |
| domain type | `loom.resource.GitBoundary` |
| domain type | `loom.resource.ImageRef` |
| domain type | `loom.resource.LastRun` |
| domain type | `loom.resource.LoomAgent.State` |
| domain type | `loom.resource.LoomAgentSpec` |
| domain type | `loom.resource.ResourceName` |
| domain type | `loom.resource.RunEnd` |
| domain type | `loom.resource.RunPhase` |
| domain type | `loom.resource.ScheduleInput` |
| domain type | `loom.resource.ScheduleSpec` |
| domain type | `loom.resource.SecretFiles` |
| domain type | `loom.resource.SecretRef` |
| domain type | `loom.resource.StageDestination` |
| domain type | `loom.resource.StageRefusalReason` |
| domain type | `loom.resource.Workspace` |
| domain type | `loom.run.ActionCatalogue.State` |
| domain type | `loom.run.ActionFacts` |
| domain type | `loom.run.ArgumentRequest.State` |
| domain type | `loom.run.ArgumentRequestId` |
| domain type | `loom.run.CatalogueEntry` |
| domain type | `loom.run.CatalogueEntryStatus` |
| domain type | `loom.run.CatalogueId` |
| domain type | `loom.run.CommissionRunId` |
| domain type | `loom.run.LoopStop` |
| domain type | `loom.run.LoopStopBudgetUnobservable` |
| domain type | `loom.run.LoopStopMaxInputTokens` |
| domain type | `loom.run.Selection.State` |
| domain type | `loom.run.SelectionId` |
| domain type | `loom.run.SelectionRecord.State` |
| domain type | `loom.run.SelectionRecordId` |
| domain type | `loom.run.SelectionStrategy` |
| domain type | `loom.run.TurnId` |
| domain type | `loom.runlog.ActionFinished` |
| domain type | `loom.runlog.ActionStarted` |
| domain type | `loom.runlog.AuthorityRecorded` |
| domain type | `loom.runlog.CaseOpened` |
| domain type | `loom.runlog.ChildClosed` |
| domain type | `loom.runlog.ChildOpened` |
| domain type | `loom.runlog.Discarded` |
| domain type | `loom.runlog.Evaluated` |
| domain type | `loom.runlog.RevisionMove` |
| domain type | `loom.runlog.RunLogEvent` |
| domain type | `loom.runlog.RunLogLine` |
| domain type | `loom.runlog.RunLogRefusal` |
| domain type | `loom.runlog.RunLogRefused` |
| domain type | `loom.runlog.Stopped` |
| domain type | `loom.runlog.Terminated` |
| domain type | `loom.session.AgentSession.State` |
| domain type | `loom.session.CallDecision` |
| domain type | `loom.session.ChangedFile` |
| domain type | `loom.session.ChildEnd` |
| domain type | `loom.session.CommissionId` |
| domain type | `loom.session.DecidedBy` |
| domain type | `loom.session.FailureRefusalReason` |
| domain type | `loom.session.FrontierId` |
| domain type | `loom.session.SessionFailure` |
| domain type | `loom.session.SessionId` |
| domain type | `loom.session.SessionKind` |
| domain type | `loom.session.ToolDecision` |
| domain type | `loom.slack.ChannelObjectives` |
| domain type | `loom.slack.SlackConfig` |
| domain type | `loom.workflow.DeterministicStep` |
| domain type | `loom.workflow.JudgementStep` |
| domain type | `loom.workflow.OutputReference` |
| domain type | `loom.workflow.OutputType` |
| domain type | `loom.workflow.RecordedOutput` |
| domain type | `loom.workflow.StepCondition` |
| domain type | `loom.workflow.StepKind` |
| domain type | `loom.workflow.StepOutput` |
| domain type | `loom.workflow.StepRequest` |
| domain type | `loom.workflow.Truth` |
| domain type | `loom.workflow.UnproducedOutput` |
| domain type | `loom.workflow.WorkflowDefinition` |
| domain type | `loom.workflow.WorkflowRunRecord` |
| domain type | `loom.workflow.WorkflowStep` |
| entity lifecycle | `loom.decision.DecisionRequest` |
| entity lifecycle | `loom.resource.LoomAgent` |
| entity lifecycle | `loom.run.ActionCatalogue` |
| entity lifecycle | `loom.run.ArgumentRequest` |
| entity lifecycle | `loom.run.Selection` |
| entity lifecycle | `loom.run.SelectionRecord` |
| entity lifecycle | `loom.session.AgentSession` |
| command contract | `loom.agent.AdmitA2aRequest` |
| command behaviour | `loom.agent.AdmitA2aRequest` |
| command contract | `loom.agent.AdmitHttpRead` |
| command behaviour | `loom.agent.AdmitHttpRead` |
| command contract | `loom.agent.AdmitHttpTrust` |
| command behaviour | `loom.agent.AdmitHttpTrust` |
| command contract | `loom.agent.AdmitSend` |
| command behaviour | `loom.agent.AdmitSend` |
| command contract | `loom.agent.AdmitServe` |
| command behaviour | `loom.agent.AdmitServe` |
| command contract | `loom.agent.BoundHttpAnswer` |
| command behaviour | `loom.agent.BoundHttpAnswer` |
| command contract | `loom.agent.CheckAgentSpec` |
| command contract | `loom.agent.RecordA2aTurn` |
| command behaviour | `loom.agent.RecordA2aTurn` |
| command contract | `loom.agent.RunOnce` |
| command behaviour | `loom.agent.RunOnce` |
| command contract | `loom.decision.Decide` |
| command contract | `loom.decision.Escalate` |
| command contract | `loom.decision.Expire` |
| command contract | `loom.decision.Recommend` |
| command contract | `loom.decision.RecordProduced` |
| command contract | `loom.decision.Request` |
| command contract | `loom.decision.Supersede` |
| command contract | `loom.decision.TakeRecommendation` |
| command contract | `loom.resource.AdmitLoomAgent` |
| command contract | `loom.resource.ApplyLoomAgent` |
| command behaviour | `loom.resource.ApplyLoomAgent` |
| command contract | `loom.resource.DeleteLoomAgent` |
| command behaviour | `loom.resource.DeleteLoomAgent` |
| command contract | `loom.resource.ObserveDeployment` |
| command behaviour | `loom.resource.ObserveDeployment` |
| command contract | `loom.resource.ObserveRun` |
| command behaviour | `loom.resource.ObserveRun` |
| command contract | `loom.resource.ObserveScheduledRun` |
| command behaviour | `loom.resource.ObserveScheduledRun` |
| command contract | `loom.resource.RecordCheck` |
| command behaviour | `loom.resource.RecordCheck` |
| command contract | `loom.resource.StageFiles` |
| command contract | `loom.run.OverruleSelection` |
| command behaviour | `loom.run.OverruleSelection` |
| command contract | `loom.run.ProjectCatalogue` |
| command behaviour | `loom.run.ProjectCatalogue` |
| command contract | `loom.run.RecordSelection` |
| command behaviour | `loom.run.RecordSelection` |
| command contract | `loom.run.RequestArguments` |
| command behaviour | `loom.run.RequestArguments` |
| command contract | `loom.run.RevalidateSelection` |
| command behaviour | `loom.run.RevalidateSelection` |
| command contract | `loom.run.SelectAction` |
| command contract | `loom.session.ProposeSessionChange` |
| command behaviour | `loom.session.ProposeSessionChange` |
| command contract | `loom.session.RecordFailedSession` |
| event type | `loom.decision.DecisionEscalated` |
| event type | `loom.decision.DecisionExpired` |
| event type | `loom.decision.DecisionRequested` |
| event type | `loom.decision.DecisionSuperseded` |
| event type | `loom.decision.DecisionTaken` |
| event type | `loom.decision.ProducedRecordRecorded` |
| event type | `loom.decision.RecommendationRecorded` |
| event type | `loom.decision.RecommendationTaken` |
| event type | `loom.decision.RecommendationWithheld` |
| event type | `loom.resource.LoomAgentApplied` |
| event type | `loom.resource.LoomAgentCompleted` |
| event type | `loom.resource.LoomAgentDegraded` |
| event type | `loom.resource.LoomAgentDeleted` |
| event type | `loom.resource.LoomAgentFailed` |
| event type | `loom.resource.LoomAgentLastRunObserved` |
| event type | `loom.resource.LoomAgentObserved` |
| event type | `loom.resource.LoomAgentRecovered` |
| event type | `loom.resource.LoomAgentRefused` |
| event type | `loom.resource.LoomAgentRunObserved` |
| event type | `loom.resource.LoomAgentRunning` |
| event type | `loom.run.ActionSelected` |
| event type | `loom.run.ArgumentsRequested` |
| event type | `loom.run.CatalogueProjected` |
| event type | `loom.run.SelectionAdmitted` |
| event type | `loom.run.SelectionNotInFrontier` |
| event type | `loom.run.SelectionOverruled` |
| event type | `loom.run.SelectionRecorded` |
| event type | `loom.run.SelectionStale` |
| event type | `loom.session.SessionChangeProposed` |
| event type | `loom.session.SessionFailed` |
| error type | `loom.agent.A2aRefused` |
| error type | `loom.agent.HttpReadRefused` |
| error type | `loom.agent.HttpTrustRefused` |
| error type | `loom.agent.RunRefused` |
| error type | `loom.agent.SendRefused` |
| error type | `loom.agent.ServeRefused` |
| error type | `loom.agent.SpecRefused` |
| error type | `loom.decision.AnswerNotAllowed` |
| error type | `loom.decision.DecisionStateConflict` |
| error type | `loom.decision.EscalationSuppressed` |
| error type | `loom.decision.NoDefaultOnSilence` |
| error type | `loom.decision.NoRecordToProduce` |
| error type | `loom.decision.NoSuchRequest` |
| error type | `loom.decision.NotARequestClass` |
| error type | `loom.decision.NotDue` |
| error type | `loom.decision.NotEligible` |
| error type | `loom.decision.OptionOrAnswer` |
| error type | `loom.decision.ProducedRecordConflict` |
| error type | `loom.decision.RecordKeyOpen` |
| error type | `loom.decision.SilenceHasDefault` |
| error type | `loom.decision.TooFewOptions` |
| error type | `loom.decision.UngroundedRecommendation` |
| error type | `loom.resource.AdmissionRefused` |
| error type | `loom.resource.LoomAgentNotFound` |
| error type | `loom.resource.LoomAgentStateConflict` |
| error type | `loom.resource.StageRefused` |
| error type | `loom.resource.StaleGeneration` |
| error type | `loom.run.ActionNotInCatalogue` |
| error type | `loom.run.CatalogueExists` |
| error type | `loom.run.CatalogueNotFound` |
| error type | `loom.run.CatalogueRevisionMismatch` |
| error type | `loom.run.SelectionExists` |
| error type | `loom.run.SelectionNotFound` |
| error type | `loom.run.SelectionNotSelected` |
| error type | `loom.run.SelectionRecordExists` |
| error type | `loom.run.SelectionStateConflict` |
| error type | `loom.session.FailureRefused` |
| view type | `loom.decision.ArtifactDecisionHistory` |
| view query | `loom.decision.ArtifactDecisionHistory` |
| view type | `loom.decision.DecisionRequests` |
| view query | `loom.decision.DecisionRequests` |
| view type | `loom.decision.OpenDecisionsForPrincipal` |
| view query | `loom.decision.OpenDecisionsForPrincipal` |
| view type | `loom.resource.LoomAgents` |
| view query | `loom.resource.LoomAgents` |
| view type | `loom.run.Catalogues` |
| view query | `loom.run.Catalogues` |
| view type | `loom.run.SelectionRecords` |
| view query | `loom.run.SelectionRecords` |
| view type | `loom.run.Selections` |
| view query | `loom.run.Selections` |
| view type | `loom.session.AgentSessions` |
| view query | `loom.session.AgentSessions` |

## Ports — yours to provide

What the specification fully determines is generated; what it cannot determine is an obligation. A generated command behaviour or view query reads and writes through the ports below, and they are yours to provide: synthesis generates each port's contract and never an implementation of one, so where instances live stays your decision.

| port | what it answers |
| --- | --- |
| storage | one per entity a generated behaviour or query reads or writes: the instance stored under an identity; storing, replacing and removing one; and every stored instance, in the order the store keeps them |
| context | where a generated behaviour asks it: the caller's attributes, every identity and value the specification says the implementation assigns, and whether each `external:` branch is taken |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `loom.agent.CheckAgentSpec` | kept an obligation by the fields of error `loom.agent.SpecRefused`, which the specification gives no source, in `malformed` | given `loom.agent.CheckAgentSpec` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `malformed` externally decided (the document at the path does not decode as a loom.agent/1 document), error `loom.agent.SpecRefused`; `secret-literal` externally decided (a string of the document holds a credential), error `loom.agent.SpecRefused`; `unknown-format` externally decided (the document's format is not loom.agent/1), error `loom.agent.SpecRefused`; `blank-text` externally decided (a text member of the document is empty or only whitespace), error `loom.agent.SpecRefused`; `not-a-variable-name` externally decided (a bearer_env of the document is not an environment variable name), error `loom.agent.SpecRefused`; `port-out-of-range` externally decided (a port of the document is outside 1 to 65535), error `loom.agent.SpecRefused`; `not-a-plain-path` externally decided (an allow_paths entry of an http binding is not a plain path), error `loom.agent.SpecRefused`; `not-an-instant` externally decided (a Timestamp member of the document is not an RFC 3339 instant), error `loom.agent.SpecRefused`; `unknown-protocol` externally decided (the inner loop names a protocol the plugin catalog does not hold, or a spec with no loops declares states inbound-answer@1 does not admit), error `loom.agent.SpecRefused`; `unknown-action` externally decided (a state names an action the inner loop's protocol does not declare), error `loom.agent.SpecRefused`; `unbound-source` externally decided (a state, the outer loop or the channel names a source no binding declares), error `loom.agent.SpecRefused`; `http-method` externally decided (an http binding's method is not GET), error `loom.agent.SpecRefused`; `send-without-writes` externally decided (reply.send is a capability while writes are none or undeclared), error `loom.agent.SpecRefused`; `empty-channel-allow` externally decided (the channel's allow list is empty), error `loom.agent.SpecRefused`; `serveable` otherwise |
| command behaviour | `loom.decision.Decide` | kept an obligation by the fields of error `loom.decision.NotEligible`, which the specification gives no source, in `kind-not-principal` | given `loom.decision.Decide` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `kind-not-principal` when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`, error `loom.decision.NotEligible`; `silence-takes-recommendation` when `principal_kind == silence`, error `loom.decision.NotEligible`; `option-or-answer` when `((defined(option) and defined(answer)) or (not (defined(option)) and not (defined(answer))))`, error `loom.decision.OptionOrAnswer`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `answer-not-allowed` when the existing subject's stored fields satisfy `unblocks.kind != question` and `defined(answer)`, error `loom.decision.AnswerNotAllowed`; `person-only` when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `principal_kind != person`, error `loom.decision.NotEligible`; `not-listed` when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))`, error `loom.decision.NotEligible`; `decider-under-rule` when the existing subject's stored fields satisfy `(holder == rule or holder == rule_final or holder == rule_and_decider or (holder == rule_and_decider_final and not (exists o in options: ((o.letter == input.option and o.consequence.grant == denied)))))` and `principal_kind == decider`, error `loom.decision.NotEligible`; `rule-not-holder` when the existing subject's stored fields satisfy `not ((holder == rule or holder == rule_final))` and `principal_kind == rule`, error `loom.decision.NotEligible`; `decided` otherwise, takes `decide` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionTaken`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.Escalate` | kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request` | given `loom.decision.Escalate` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `suppressed` when the existing subject's stored fields satisfy `(holder == decider_final or holder == rule_final or holder == rule_and_decider_final)`, error `loom.decision.EscalationSuppressed`; `escalated` otherwise, takes `escalate` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionEscalated`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.Expire` | kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request` | given `loom.decision.Expire` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `not-due` when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.at as timestamp)`, error `loom.decision.NotDue`; `has-default` when the existing subject's stored fields satisfy `default_on_silence == true`, error `loom.decision.SilenceHasDefault`; `expired` otherwise, takes `expire` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionExpired`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.Recommend` | kept an obligation by the fields of error `loom.decision.UngroundedRecommendation`, which the specification gives no source, in `ungrounded` | given `loom.decision.Recommend` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `ungrounded` when `(check_verdict == verified and (grounds.count < 1 or strength == none))`, error `loom.decision.UngroundedRecommendation`; `silence-needs-strong` when `(check_verdict == verified and default_on_silence == true and strength != strong)`, error `loom.decision.NoDefaultOnSilence`; `option-or-answer` when `(check_verdict == verified and ((defined(recommended_option) and defined(recommended_answer)) or (not (defined(recommended_option)) and not (defined(recommended_answer)))))`, error `loom.decision.OptionOrAnswer`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `answer-not-allowed` when the existing subject's stored fields satisfy `unblocks.kind != question` and `(check_verdict == verified and defined(recommended_answer))`, error `loom.decision.AnswerNotAllowed`; `withheld` when `check_verdict == refused`, takes `recommend` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationWithheld`; `recommended` otherwise, takes `recommend` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationRecorded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.RecordProduced` | kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request` | given `loom.decision.RecordProduced` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `already-recorded` when the existing subject's stored fields satisfy `{expr: "coalesce(produced_record.record, "")"} == input.produced.record`, preserves `loom.decision.DecisionRequest`; `produced-conflict` when the existing subject's stored fields satisfy `defined(produced_record)`, error `loom.decision.ProducedRecordConflict`; `option-produces-none` when the existing subject's stored fields satisfy `(defined(decided_option) and not (exists o in options: ((o.letter == {fact: decided_option} and defined(o.consequence.produces)))))`, error `loom.decision.NoRecordToProduce`; `recorded` otherwise, takes `record_produced` of `loom.decision.DecisionRequest`, emits `loom.decision.ProducedRecordRecorded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.Request` | kept an obligation by the fields of error `loom.decision.NotARequestClass`, which the specification gives no source, in `escalation-is-not-a-class` | given `loom.decision.Request` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `escalation-is-not-a-class` when `decision_class == escalation`, error `loom.decision.NotARequestClass`; `too-few-options` when `options.count < 2`, error `loom.decision.TooFewOptions`; `record-key-open` when `defined(open_for_record)`, error `loom.decision.RecordKeyOpen`; `requested` otherwise, creates `loom.decision.DecisionRequest`, emits `loom.decision.DecisionRequested` |
| command behaviour | `loom.decision.Supersede` | kept an obligation by the fields of error `loom.decision.NoSuchRequest`, which the specification gives no source, in `no-request` | given `loom.decision.Supersede` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `superseded` otherwise, takes `supersede` of `loom.decision.DecisionRequest`, emits `loom.decision.DecisionSuperseded`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.decision.TakeRecommendation` | kept an obligation by the fields of error `loom.decision.NotEligible`, which the specification gives no source, in `kind-not-principal` | given `loom.decision.TakeRecommendation` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `kind-not-principal` when `((principal_kind == decider and not (principal starts_with "decider:")) or (principal_kind == rule and not (principal starts_with "rule:")) or (principal_kind == silence and principal != silence) or (principal_kind == person and (principal starts_with "decider:" or principal starts_with "rule:" or principal == silence)))`, error `loom.decision.NotEligible`; `no-request` for an identity no record carries, error `loom.decision.NoSuchRequest`; `person-only` when the existing subject's stored fields satisfy `(holder == person or holder == advised)` and `(principal_kind == decider or principal_kind == rule)`, error `loom.decision.NotEligible`; `not-listed` when the existing subject's stored fields satisfy `not (exists p in eligible: (p == input.principal))` and `principal_kind != silence`, error `loom.decision.NotEligible`; `not-strong` when the existing subject's stored fields satisfy `recommendation_strength != strong`, error `loom.decision.NotEligible`; `silence-without-default` when the existing subject's stored fields satisfy `default_on_silence == false` and `principal_kind == silence`, error `loom.decision.NoDefaultOnSilence`; `silence-not-due` when the existing subject's stored fields satisfy `(not (defined(expires_at)) or expires_at > input.decided_at as timestamp)` and `principal_kind == silence`, error `loom.decision.NotDue`; `taken` otherwise, takes `decide` of `loom.decision.DecisionRequest`, emits `loom.decision.RecommendationTaken`; `wrong-state` from a state no declared move starts in, error `loom.decision.DecisionStateConflict` |
| command behaviour | `loom.resource.AdmitLoomAgent` | kept an obligation by the fields of error `loom.resource.AdmissionRefused`, which the specification gives no source, in `malformed` | given `loom.resource.AdmitLoomAgent` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `malformed` externally decided (a member of the resource does not decode, or breaks a rule no other reason names), error `loom.resource.AdmissionRefused`; `secret-literal` externally decided (a member of the resource holds a credential), error `loom.resource.AdmissionRefused`; `unknown-format` externally decided (spec.agent's format is not loom.agent/1), error `loom.resource.AdmissionRefused`; `blank-text` externally decided (a text member of spec.agent is empty or only whitespace), error `loom.resource.AdmissionRefused`; `not-a-variable-name` externally decided (a bearer_env of spec.agent, or a spec.env name, is not an environment variable name), error `loom.resource.AdmissionRefused`; `port-out-of-range` externally decided (a port of spec.agent is outside 1 to 65535), error `loom.resource.AdmissionRefused`; `not-a-plain-path` externally decided (an allow_paths entry of an http binding of spec.agent is not a plain path), error `loom.resource.AdmissionRefused`; `not-an-instant` externally decided (a Timestamp member of spec.agent is not an RFC 3339 instant), error `loom.resource.AdmissionRefused`; `unknown-protocol` externally decided (the inner loop of an AgentServe resource's spec.agent names a protocol the plugin catalog does not hold), error `loom.resource.AdmissionRefused`; `unknown-action` externally decided (a state of an AgentServe resource's spec.agent names an action its protocol does not declare), error `loom.resource.AdmissionRefused`; `unbound-source` externally decided (a state, the outer loop or the channel of an AgentServe resource's spec.agent names a source no binding declares), error `loom.resource.AdmissionRefused`; `http-method` externally decided (an http binding of an AgentServe resource's spec.agent has a method other than GET), error `loom.resource.AdmissionRefused`; `send-without-writes` externally decided (an AgentServe resource's spec.agent has reply.send among its capabilities while writes are none or undeclared), error `loom.resource.AdmissionRefused`; `empty-channel-allow` externally decided (the channel of an AgentServe resource's spec.agent allows no channel id), error `loom.resource.AdmissionRefused`; `no-model-route` externally decided (spec.routes is empty and the pod would reach no model), error `loom.resource.AdmissionRefused`; `too-large` externally decided (a member is beyond what the credential walk reads, or the ConfigMap beyond 1 MiB), error `loom.resource.AdmissionRefused`; `boundary-with-secret` externally decided (spec.boundary stands beside spec.secretRef or spec.secretFiles), error `loom.resource.AdmissionRefused`; `boundary-upstream-invalid` externally decided (a boundary upstream is no plain http URL of a cluster service), error `loom.resource.AdmissionRefused`; `boundary-upstream-not-allowed` externally decided (no allowedBoundaryUpstreams entry gives a boundary upstream's kind, URL and audience exactly), error `loom.resource.AdmissionRefused`; `boundary-egress-host` externally decided (beside spec.boundary, an egress entry of spec.agent names a host), error `loom.resource.AdmissionRefused`; `boundary-egress-namespace` externally decided (beside spec.boundary, an egress entry of spec.agent names the namespace of an allowedBoundaryUpstreams upstream or of the modelRoute, names no namespace, or gives no pod selector), error `loom.resource.AdmissionRefused`; `boundary-egress-unbound` externally decided (beside spec.boundary, no GET http binding of spec.agent without a bearer_env has a base_url that is a cluster service in the namespace an egress entry names), error `loom.resource.AdmissionRefused`; `boundary-egress-not-allowed` externally decided (beside spec.boundary, no allowedBoundaryHttpEgress entry equals a namespace egress entry of spec.agent in namespace, selector and port), error `loom.resource.AdmissionRefused`; `secret-not-allowed` externally decided (spec.secretRef or spec.secretFiles names a Secret outside allowedSecretNames), error `loom.resource.AdmissionRefused`; `namespace-mismatch` externally decided (spec.agent's deploy.kubernetes.namespace is not the resource's namespace), error `loom.resource.AdmissionRefused`; `egress-host-not-allowed` externally decided (an egress entry of spec.agent names no destination, or a host outside allowedEgressHosts), error `loom.resource.AdmissionRefused`; `env-name-conflict` externally decided (a spec.env name is one the controller sets, or one an earlier entry gives), error `loom.resource.AdmissionRefused`; `config-file-invalid` externally decided (a spec.configFiles name is no plain file name), error `loom.resource.AdmissionRefused`; `config-file-conflict` externally decided (a spec.configFiles name is one an earlier entry gives, or one the ConfigMap already holds), error `loom.resource.AdmissionRefused`; `mount-path-invalid` externally decided (a workspace or Secret mount path is not absolute and plain, or overlaps a mount the controller makes or the other one), error `loom.resource.AdmissionRefused`; `claim-not-allowed` externally decided (spec.workspace names a claim outside allowedClaimNames), error `loom.resource.AdmissionRefused`; `profile-invalid` externally decided (a seccomp or AppArmor profile is out of form), error `loom.resource.AdmissionRefused`; `profile-not-allowed` externally decided (a seccomp or AppArmor profile is Unconfined or Localhost without the configuration allowing it), error `loom.resource.AdmissionRefused`; `resources-required` externally decided (a Command resource declares no size and the configuration gives no defaultCommandResources), error `loom.resource.AdmissionRefused`; `admitted` otherwise |
| command behaviour | `loom.resource.StageFiles` | kept an obligation by a typed response (`response:`) | given `loom.resource.StageFiles` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `destination-is-a-link` when `destination == Link`, error `loom.resource.StageRefused`; `destination-not-a-directory` when `destination == Other`, error `loom.resource.StageRefused`; `name-not-utf8` when `names_utf8 == false`, error `loom.resource.StageRefused`; `key-outside-volume` when `keys_inside == false`, error `loom.resource.StageRefused`; `stale-directory` when `stale_directory == true`, error `loom.resource.StageRefused`; `staged` otherwise |
| command behaviour | `loom.run.SelectAction` | kept an obligation by the guard `exists entry in entries: (entry.action == input.action)`, in `not-in-catalogue` | given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then choose the present `when_related:` predicate refusal whose predicate and optional input guard hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `selection-exists` for an identity a record already carries, error `loom.run.SelectionExists`; `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected` |
| command behaviour | `loom.session.RecordFailedSession` | kept an obligation by `creates:` leaving the required field `files` of `loom.session.AgentSession` undetermined, in `failed` | given `loom.session.RecordFailedSession` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `unmeasured-with-code` when `(failure.end == NotMeasured and defined(failure.code))`, error `loom.session.FailureRefused`; `measured-without-code` when `(failure.end != NotMeasured and not (defined(failure.code)))`, error `loom.session.FailureRefused`; `failed` otherwise, creates `loom.session.AgentSession`, emits `loom.session.SessionFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
| actor grants | `loom.decision.Decider` | planning | may invoke `loom.decision.Decide`, `loom.decision.Escalate`, `loom.decision.TakeRecommendation`; generated as data, not enforced: the grant is available as the declared actors and the qualified commands each may invoke, and enforcement stays with the caller, because a grant is checked against a caller identity, which types do not carry |
| actor grants | `loom.decision.Person` | planning | may invoke `loom.decision.Decide`, `loom.decision.Escalate`, `loom.decision.TakeRecommendation`; generated as data, not enforced: the grant is available as the declared actors and the qualified commands each may invoke, and enforcement stays with the caller, because a grant is checked against a caller identity, which types do not carry |
| actor grants | `loom.decision.Runtime` | planning | may invoke `loom.decision.Expire`, `loom.decision.Recommend`, `loom.decision.RecordProduced`, `loom.decision.Request`, `loom.decision.Supersede`, `loom.decision.TakeRecommendation`; generated as data, not enforced: the grant is available as the declared actors and the qualified commands each may invoke, and enforcement stays with the caller, because a grant is checked against a caller identity, which types do not carry |
