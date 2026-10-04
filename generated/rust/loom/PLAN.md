<!--
  generated from loom v1
  model digest d801974218ef3d92c2eb884a7d4e7c56bb3fb32826e0145126651ace85e0bde9
  contract digest d245142e3f655485701182d044d14de4c925f8e8e31542a01bc3184e331a667a
  do not edit: regenerate with `ess synthesize --layout crate`
-->
# Synthesis plan — loom v1

Scope: `component-skeletons`, laid out as `crate`, planned by `ess-synth`. Regenerate with `ess synthesize --layout crate`.

44 capabilities: **42 generated**, **2 obligations**, **0 refused**. An obligation is yours to implement against its contract; a refusal is a fact about this synthesis scope, not about the specification.

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
| command contract | `loom.run.ProjectCatalogue` |
| command behaviour | `loom.run.ProjectCatalogue` |
| command contract | `loom.run.RequestArguments` |
| command contract | `loom.run.RevalidateSelection` |
| command behaviour | `loom.run.RevalidateSelection` |
| command contract | `loom.run.SelectAction` |
| event type | `loom.run.ActionSelected` |
| event type | `loom.run.ArgumentsRequested` |
| event type | `loom.run.CatalogueProjected` |
| event type | `loom.run.SelectionAdmitted` |
| event type | `loom.run.SelectionNotInFrontier` |
| event type | `loom.run.SelectionStale` |
| error type | `loom.run.ActionNotInCatalogue` |
| error type | `loom.run.CatalogueExists` |
| error type | `loom.run.CatalogueNotFound` |
| error type | `loom.run.CatalogueRevisionMismatch` |
| error type | `loom.run.SelectionNotFound` |
| error type | `loom.run.SelectionNotSelected` |
| error type | `loom.run.SelectionStateConflict` |
| view type | `loom.run.Catalogues` |
| view query | `loom.run.Catalogues` |
| view type | `loom.run.Selections` |
| view query | `loom.run.Selections` |

## Ports — yours to provide

What the specification fully determines is generated; what it cannot determine is an obligation. A generated command behaviour or view query reads and writes through the ports below, and they are yours to provide: synthesis generates each port's contract and never an implementation of one, so where instances live stays your decision.

| port | what it answers |
| --- | --- |
| storage | one per entity a generated behaviour or query reads or writes: the instance stored under an identity; storing, replacing and removing one; and every stored instance, in the order the store keeps them |
| context | where a generated behaviour asks it: the caller's attributes, every identity and value the specification says the implementation assigns, and whether each `external:` branch is taken |

## Obligations — yours to implement

| capability | source | why not generated | contract |
| --- | --- | --- | --- |
| command behaviour | `loom.run.RequestArguments` | kept an obligation by `when_related:`, in `selection-unknown` | given `loom.run.RequestArguments` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `selection-unknown` when no `loom.run.Selection` carries the identity `input.selection_id` names, error `loom.run.SelectionNotFound`; `selection-not-selected` when the `loom.run.Selection` that `input.selection_id` names satisfies `state != Selected`, error `loom.run.SelectionNotSelected`; `requested` otherwise, creates `loom.run.ArgumentRequest`, emits `loom.run.ArgumentsRequested` |
| command behaviour | `loom.run.SelectAction` | kept an obligation by `when_related:`, in `catalogue-unknown` | given `loom.run.SelectAction` input, decide and enact exactly one outcome. Selection precedence: on commands with `when_related:`, check `existing_instance` then `exists: false` before input-guarded refusals; choose the first declared input refusal whose guard holds; then check addressed-row existence (`unknown_instance`, and `existing_instance` on commands without `when_related:`); then the held state (`when_subject_state` and `when_subject`), with `wrong_state` only if the selected branch moves from a state the row does not hold; then accepting and external branches in declaration order. An accepting branch that moves nothing answers in every state. Related-presence predicates do not precede input-guarded refusals. Declared outcomes (declaration order, not selection precedence): `catalogue-unknown` when no `loom.run.ActionCatalogue` carries the identity `input.catalogue_id` names, error `loom.run.CatalogueNotFound`; `not-in-catalogue` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `not (exists entry in entries: (entry.action == input.action))`, error `loom.run.ActionNotInCatalogue`; `revision-mismatch` when the `loom.run.ActionCatalogue` that `input.catalogue_id` names satisfies `case_revision != input.case_revision`, error `loom.run.CatalogueRevisionMismatch`; `selected` otherwise, creates `loom.run.Selection`, emits `loom.run.ActionSelected` |

## Refused — not represented by this synthesis

| capability | source | stage | why |
| --- | --- | --- | --- |
