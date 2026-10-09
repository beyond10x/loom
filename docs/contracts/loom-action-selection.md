# Contract Sketch — Loom Action Selection

## Goal

Separate **which admissible action to take** from **how to construct its arguments**.

This allows cheap/fast decision models to perform routine routing.

## Interfaces

Conceptual:

```rust
pub trait ActionSelector {
    fn select(
        &self,
        context: &SelectionContext,
        candidates: &[ActionDescriptor],
    ) -> Result<Selection, SelectionError>;
}

pub struct Selection {
    pub action: ActionId,
    pub confidence: Option<f32>,
}

pub trait ArgumentGenerator {
    fn generate(
        &self,
        context: &ArgumentContext,
        action: &ActionDescriptor,
    ) -> Result<serde_json::Value, ArgumentError>;
}
```

As built (`crates/loom-executor/src/arguments.rs`), the generator is handed the argument context and the one
catalogue entry the selection names, never the rest of the catalogue, and returns Commission's JSON
`Value`, which becomes the proposed action's `ProposedActionArguments`; an `Err` carries the reason
the generator could not answer. Before it is called, Loom records the `loom.run.ArgumentRequest`
against the selection it serves (`loom.run.RequestArguments`).

```rust
pub trait ArgumentGenerator {
    fn generate(
        &self,
        context: &ArgumentContext,
        entry: &CatalogueEntry,
    ) -> Result<Value, String>;
}
```

## Laya path

```text
Frontier candidates
        ↓
compact selection context
        ↓
Laya typed choice
        ↓
ActionId + probability
        ↓
threshold
  ┌─────┴─────┐
  │           │
high        low
  │           │
  ▼           ▼
argument   full reasoning
generator  selector/planner
  │
  ▼
schema validation
  │
  ▼
revalidate frontier + revision + authority
  │
  ▼
execute
```

As built, the "Laya typed choice" step is `b10x-loom-selector-laya` (`LayaSelector`,
`SelectionStrategy::FastTyped`). It asks one `choice` question whose criteria are the candidate ids,
reads `answer_confidence` as the probability, and answers `SelectorError::Unavailable` for a choice
outside the candidates or any transport or answer failure, so the step below it can fall back. The
threshold step is not built yet. See
[Laya integration § As built](../integrations/laya-fast-selection.md#as-built).

## Safety rules

1. Candidate labels originate from the current frontier.
2. Unknown action IDs are rejected.
3. Selection confidence never grants authority.
4. A selected action is revalidated before effect.
5. Low confidence falls back to a stronger path.
6. For >~20 actions, selectors may use family → action hierarchy.
7. Selection telemetry should be available to Metaharness for evaluation.
