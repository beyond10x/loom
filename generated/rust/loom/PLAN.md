<!--
  generated from loom v1
  model digest 66a02a0baf5fbe8890612e69df450544c958c9f01799dc162cea2a954a89a58c
  contract digest 70e66c4d4dd22d9a004633e14def9f35e93b84c0782021c37bc41550f5e18b6a
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — loom v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

267 capabilities: **264 generated**, **3 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

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
| domain type | `loom.plugin.TurnResult` |
| domain type | `loom.resource.AgentVariant` |
| domain type | `loom.resource.CheckResult` |
| domain type | `loom.resource.CommandSpec` |
| domain type | `loom.resource.Condition` |
| domain type | `loom.resource.ConditionStatus` |
| domain type | `loom.resource.ConditionType` |
| domain type | `loom.resource.ConfigFile` |
| domain type | `loom.resource.ConfinementProfile` |
| domain type | `loom.resource.ConfinementProfileKind` |
| domain type | `loom.resource.DeploymentAvailability` |
| domain type | `loom.resource.DeploymentStatus` |
| domain type | `loom.resource.EnvVar` |
| domain type | `loom.resource.ImageRef` |
| domain type | `loom.resource.LoomAgent.State` |
| domain type | `loom.resource.LoomAgentSpec` |
| domain type | `loom.resource.ResourceName` |
| domain type | `loom.resource.RunEnd` |
| domain type | `loom.resource.RunPhase` |
| domain type | `loom.resource.SecretFiles` |
| domain type | `loom.resource.SecretRef` |
| domain type | `loom.resource.Workspace` |
| domain type | `loom.run.ActionCatalogue.State` |
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
| domain type | `loom.session.AgentSession.State` |
| domain type | `loom.session.CallDecision` |
| domain type | `loom.session.ChangedFile` |
| domain type | `loom.session.ChildEnd` |
| domain type | `loom.session.CommissionId` |
| domain type | `loom.session.DecidedBy` |
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
| command contract | `loom.resource.ApplyLoomAgent` |
| command behaviour | `loom.resource.ApplyLoomAgent` |
| command contract | `loom.resource.DeleteLoomAgent` |
| command behaviour | `loom.resource.DeleteLoomAgent` |
| command contract | `loom.resource.ObserveDeployment` |
| command behaviour | `loom.resource.ObserveDeployment` |
| command contract | `loom.resource.ObserveRun` |
| command behaviour | `loom.resource.ObserveRun` |
| command contract | `loom.resource.RecordCheck` |
| command behaviour | `loom.resource.RecordCheck` |
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
| event type | `loom.resource.LoomAgentApplied` |
| event type | `loom.resource.LoomAgentCompleted` |
| event type | `loom.resource.LoomAgentDegraded` |
| event type | `loom.resource.LoomAgentDeleted` |
| event type | `loom.resource.LoomAgentFailed` |
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
| error type | `loom.agent.SendRefused` |
| error type | `loom.agent.ServeRefused` |
| error type | `loom.agent.SpecRefused` |
| error type | `loom.resource.LoomAgentNotFound` |
| error type | `loom.resource.LoomAgentStateConflict` |
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
| command behaviour | `loom.agent.CheckAgentSpec` | kept an obligation by the fields of error `loom.agent.SpecRefused`, which the specification gives no source, in `malformed` | given `loom.agent.CheckAgentSpec` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `malformed` externally decided (the document at the path does not decode as a loom.agent/1 document), error `loom.agent.SpecRefused`; `secret-literal` externally decided (a string of the document holds a credential), error `loom.agent.SpecRefused`; `unknown-format` externally decided (the document's format is not loom.agent/1), error `loom.agent.SpecRefused`; `blank-text` externally decided (a text member of the document is empty or only whitespace), error `loom.agent.SpecRefused`; `not-a-variable-name` externally decided (a bearer_env of the document is not an environment variable name), error `loom.agent.SpecRefused`; `port-out-of-range` externally decided (a port of the document is outside 1 to 65535), error `loom.agent.SpecRefused`; `not-a-plain-path` externally decided (an allow_paths entry of an http binding is not a plain path), error `loom.agent.SpecRefused`; `not-an-instant` externally decided (a Timestamp member of the document is not an RFC 3339 instant), error `loom.agent.SpecRefused`; `unknown-protocol` externally decided (the inner loop names a protocol the plugin catalog does not hold, or states are declared with no inner loop), error `loom.agent.SpecRefused`; `unknown-action` externally decided (a state names an action the inner loop's protocol does not declare), error `loom.agent.SpecRefused`; `unbound-source` externally decided (a state, the outer loop or the channel names a source no binding declares), error `loom.agent.SpecRefused`; `http-method` externally decided (an http binding's method is not GET), error `loom.agent.SpecRefused`; `send-without-writes` externally decided (reply.send is a capability while writes are none or undeclared), error `loom.agent.SpecRefused`; `empty-channel-allow` externally decided (the channel's allow list is empty), error `loom.agent.SpecRefused`; `serveable` otherwise |
| command behaviour | `loom.run.SelectAction` | kept an obligation by the guard `exists entry in entries: (entry.action == input.action)`, in `not-in-catalogue` | given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `selection-exists` for an identity a record already carries, error `loom.run.SelectionExists`; `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected` |
| command behaviour | `loom.session.RecordFailedSession` | kept an obligation by `creates:` leaving the required field `files` of `loom.session.AgentSession` undetermined, in `failed` | given `loom.session.RecordFailedSession` input, decide and enact exactly one outcome. Declared outcomes (declaration order, not selection precedence): `failed` otherwise, creates `loom.session.AgentSession`, emits `loom.session.SessionFailed` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
