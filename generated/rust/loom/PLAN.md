<!--
  generated from loom v1
  model digest 661401844e582bd43becacc2018c2e17e03abd6db5247a31d68efc412baadd64
  contract digest 6085f971d79bcae6071aef5103a63bf925d279df28713c5dd4a816f29b50f01e
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — loom v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

142 capabilities: **141 generated**, **1 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `loom.datasource.AdapterAlias` |
| domain type | `loom.datasource.ConnectionId` |
| domain type | `loom.datasource.ConnectorsCliConfig` |
| domain type | `loom.datasource.DataSource` |
| domain type | `loom.datasource.OperationId` |
| domain type | `loom.datasource.ReadKind` |
| domain type | `loom.datasource.ReadRefusal` |
| domain type | `loom.datasource.ReadResult` |
| domain type | `loom.datasource.SourceEntity` |
| domain type | `loom.datasource.SourceName` |
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
| domain type | `loom.evaluation.ProtocolName` |
| domain type | `loom.governor.StoredArtifact` |
| domain type | `loom.governor.StoredCase` |
| domain type | `loom.governor.StoredCaseId` |
| domain type | `loom.governor.StoredEvidence` |
| domain type | `loom.governor.StoredEvidenceData` |
| domain type | `loom.governor.StoredObservation` |
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
| domain type | `loom.run.ActionCatalogue.State` |
| domain type | `loom.run.ArgumentRequest.State` |
| domain type | `loom.run.ArgumentRequestId` |
| domain type | `loom.run.CatalogueEntry` |
| domain type | `loom.run.CatalogueEntryStatus` |
| domain type | `loom.run.CatalogueId` |
| domain type | `loom.run.CommissionRunId` |
| domain type | `loom.run.Compaction.State` |
| domain type | `loom.run.CompactionId` |
| domain type | `loom.run.CredentialKind` |
| domain type | `loom.run.CredentialReference` |
| domain type | `loom.run.LoopStop` |
| domain type | `loom.run.LoopStopAwaitingApproval` |
| domain type | `loom.run.LoopStopBudgetUnobservable` |
| domain type | `loom.run.LoopStopCancelled` |
| domain type | `loom.run.LoopStopContextAboveTrigger` |
| domain type | `loom.run.LoopStopDeadline` |
| domain type | `loom.run.LoopStopMaxCost` |
| domain type | `loom.run.LoopStopMaxInputTokens` |
| domain type | `loom.run.LoopStopMaxOutputTokens` |
| domain type | `loom.run.LoopStopMaxTurns` |
| domain type | `loom.run.LoopStopProviderIncomplete` |
| domain type | `loom.run.LoopStopUnstructured` |
| domain type | `loom.run.ReportedUsage` |
| domain type | `loom.run.RunEnding` |
| domain type | `loom.run.Selection.State` |
| domain type | `loom.run.SelectionId` |
| domain type | `loom.run.SelectionStrategy` |
| domain type | `loom.run.Session.State` |
| domain type | `loom.run.SessionId` |
| domain type | `loom.run.Turn.State` |
| domain type | `loom.run.TurnId` |
| domain type | `loom.run.WireCredential` |
| domain type | `loom.slack.ChannelObjectives` |
| domain type | `loom.slack.SlackConfig` |
| entity lifecycle | `loom.run.ActionCatalogue` |
| entity lifecycle | `loom.run.ArgumentRequest` |
| entity lifecycle | `loom.run.Compaction` |
| entity lifecycle | `loom.run.Selection` |
| entity lifecycle | `loom.run.Session` |
| entity lifecycle | `loom.run.Turn` |
| command contract | `loom.run.FileSession` |
| command behaviour | `loom.run.FileSession` |
| command contract | `loom.run.InterruptSession` |
| command behaviour | `loom.run.InterruptSession` |
| command contract | `loom.run.OpenSession` |
| command behaviour | `loom.run.OpenSession` |
| command contract | `loom.run.OverruleSelection` |
| command behaviour | `loom.run.OverruleSelection` |
| command contract | `loom.run.ProjectCatalogue` |
| command behaviour | `loom.run.ProjectCatalogue` |
| command contract | `loom.run.RecordCompaction` |
| command behaviour | `loom.run.RecordCompaction` |
| command contract | `loom.run.RecordTurn` |
| command behaviour | `loom.run.RecordTurn` |
| command contract | `loom.run.ReleaseSession` |
| command behaviour | `loom.run.ReleaseSession` |
| command contract | `loom.run.RequestArguments` |
| command behaviour | `loom.run.RequestArguments` |
| command contract | `loom.run.ResumeSession` |
| command behaviour | `loom.run.ResumeSession` |
| command contract | `loom.run.RevalidateSelection` |
| command behaviour | `loom.run.RevalidateSelection` |
| command contract | `loom.run.SelectAction` |
| event type | `loom.run.ActionSelected` |
| event type | `loom.run.ArgumentsRequested` |
| event type | `loom.run.CatalogueProjected` |
| event type | `loom.run.SelectionAdmitted` |
| event type | `loom.run.SelectionNotInFrontier` |
| event type | `loom.run.SelectionOverruled` |
| event type | `loom.run.SelectionStale` |
| event type | `loom.run.SessionCompacted` |
| event type | `loom.run.SessionFiled` |
| event type | `loom.run.SessionInterrupted` |
| event type | `loom.run.SessionOpened` |
| event type | `loom.run.SessionReleased` |
| event type | `loom.run.SessionResumed` |
| event type | `loom.run.TurnRecorded` |
| error type | `loom.run.ActionNotInCatalogue` |
| error type | `loom.run.CatalogueExists` |
| error type | `loom.run.CatalogueNotFound` |
| error type | `loom.run.CatalogueRevisionMismatch` |
| error type | `loom.run.SelectionNotFound` |
| error type | `loom.run.SelectionNotSelected` |
| error type | `loom.run.SelectionStateConflict` |
| error type | `loom.run.SessionExists` |
| error type | `loom.run.SessionNotActive` |
| error type | `loom.run.SessionNotFound` |
| error type | `loom.run.SessionStateConflict` |
| error type | `loom.run.SessionWireMismatch` |
| view type | `loom.run.Catalogues` |
| view query | `loom.run.Catalogues` |
| view type | `loom.run.Selections` |
| view query | `loom.run.Selections` |
| view type | `loom.run.Sessions` |
| view query | `loom.run.Sessions` |

## Ports — yours to provide

What the specification fully determines is generated; what it cannot determine is an obligation. A generated command behaviour or view query reads and writes through the ports below, and they are yours to provide: synthesis generates each port's contract and never an implementation of one, so where instances live stays your decision.

| port | what it answers |
| --- | --- |
| storage | one per entity a generated behaviour or query reads or writes: the instance stored under an identity; storing, replacing and removing one; and every stored instance, in the order the store keeps them |
| context | where a generated behaviour asks it: the caller's attributes, every identity and value the specification says the implementation assigns, and whether each `external:` branch is taken |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `loom.run.SelectAction` | kept an obligation by the guard `exists entry in entries: (entry.action == input.action)`, in `not-in-catalogue` | given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
