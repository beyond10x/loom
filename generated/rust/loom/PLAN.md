<!--
  generated from loom v1
  model digest 1e5c1537dda3b2b7e22b162efd4278fee13385bc49abc57d7ce5100af963d5c1
  contract digest d23e825dcb7bfe03fbe20cea75b62e61bd0a45f586a2de0740780d02d5d2b8a1
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — loom v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

67 capabilities: **64 generated**, **3 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

## Generated

| capability | source |
| --- | --- |
| domain type | `loom.run.ActionCatalogue.State` |
| domain type | `loom.run.ArgumentRequest.State` |
| domain type | `loom.run.ArgumentRequestId` |
| domain type | `loom.run.CatalogueEntry` |
| domain type | `loom.run.CatalogueEntryStatus` |
| domain type | `loom.run.CatalogueId` |
| domain type | `loom.run.CommissionRunId` |
| domain type | `loom.run.RunEnding` |
| domain type | `loom.run.Selection.State` |
| domain type | `loom.run.SelectionId` |
| domain type | `loom.run.SelectionStrategy` |
| domain type | `loom.run.Session.State` |
| domain type | `loom.run.SessionId` |
| domain type | `loom.run.Turn.State` |
| domain type | `loom.run.TurnId` |
| entity lifecycle | `loom.run.ActionCatalogue` |
| entity lifecycle | `loom.run.ArgumentRequest` |
| entity lifecycle | `loom.run.Selection` |
| entity lifecycle | `loom.run.Session` |
| entity lifecycle | `loom.run.Turn` |
| command contract | `loom.run.FileSession` |
| command behaviour | `loom.run.FileSession` |
| command contract | `loom.run.OpenSession` |
| command behaviour | `loom.run.OpenSession` |
| command contract | `loom.run.ProjectCatalogue` |
| command behaviour | `loom.run.ProjectCatalogue` |
| command contract | `loom.run.RecordTurn` |
| command contract | `loom.run.ReleaseSession` |
| command behaviour | `loom.run.ReleaseSession` |
| command contract | `loom.run.RequestArguments` |
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
| event type | `loom.run.SelectionStale` |
| event type | `loom.run.SessionFiled` |
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
| command behaviour | `loom.run.RecordTurn` | kept an obligation by `when_related:`, in `session-unknown` | given `loom.run.RecordTurn` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `session-unknown` when no `loom.run.Session` carries the identity `input.session_id` names, error `loom.run.SessionNotFound`; `session-not-active` when the `loom.run.Session` that `input.session_id` names satisfies `state != Active`, error `loom.run.SessionNotActive`; `recorded` otherwise, creates `loom.run.Turn`, emits `loom.run.TurnRecorded` |
| command behaviour | `loom.run.RequestArguments` | kept an obligation by `when_related:`, in `selection-unknown` | given `loom.run.RequestArguments` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `selection-unknown` when no `loom.run.Selection` carries the identity `input.selection_id` names, error `loom.run.SelectionNotFound`; `selection-not-selected` when the `loom.run.Selection` that `input.selection_id` names satisfies `state != Selected`, error `loom.run.SelectionNotSelected`; `requested` otherwise, creates `loom.run.ArgumentRequest`, emits `loom.run.ArgumentsRequested` |
| command behaviour | `loom.run.SelectAction` | kept an obligation by `when_related:`, in `catalogue-unknown` | given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
