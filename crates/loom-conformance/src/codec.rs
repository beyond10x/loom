//! Values between the suite's vocabulary ([`Node`]) and the generated run model.
//!
//! Decoding reads one declared input field into the generated type the model gives it, and answers
//! `None` when the value cannot be held by that type: the command then reaches no declared outcome.
//! An `Optional` field reads as `Some(None)` when it is absent or null. Encoding writes a generated
//! value back under the shape the model declares for it, an absent `Optional` as null.

use std::collections::BTreeMap;

use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    ActionCatalogueState, CatalogueEntry, CatalogueEntryStatus, ReportedUsage, RunEnding,
    SelectionRecordState, SelectionState, SelectionStrategy, SessionState,
};
use ess_primitives::facts::{Number, is_canonical_uuid};
use ess_primitives::node::Node;

/// The declared input of one command invocation.
pub type Input = BTreeMap<String, Node>;

/// The text field `name`.
pub fn text(input: &Input, name: &str) -> Option<String> {
    input.get(name)?.as_text().map(ToOwned::to_owned)
}

/// The `Uuid` field `name`, in the canonical hyphenated form the generated model carries; `None`
/// for any other text.
pub fn uuid(input: &Input, name: &str) -> Option<Uuid> {
    text(input, name)
        .filter(|text| is_canonical_uuid(text))
        .map(Uuid)
}

/// The `Integer` field `name`.
pub fn integer(input: &Input, name: &str) -> Option<i64> {
    integer_of(input.get(name)?)
}

fn integer_of(node: &Node) -> Option<i64> {
    match node {
        Node::Number(number) => number.as_i64(),
        _ => None,
    }
}

/// The `List<String>` field `name`.
pub fn texts(input: &Input, name: &str) -> Option<Vec<String>> {
    input
        .get(name)?
        .as_seq()?
        .iter()
        .map(|item| item.as_text().map(ToOwned::to_owned))
        .collect()
}

/// The `Optional<Decimal>` field `name`, in the exact spelling [`Number::exact_text`] gives it.
pub fn optional_decimal(input: &Input, name: &str) -> Option<Option<Decimal>> {
    match input.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Number(number)) => Some(Some(Decimal(number.exact_text()))),
        Some(_) => None,
    }
}

/// The `loom.run.RunEnding` field `name`.
pub fn run_ending(input: &Input, name: &str) -> Option<RunEnding> {
    Some(match input.get(name)?.as_text()? {
        "Answered" => RunEnding::Answered,
        "Stopped" => RunEnding::Stopped,
        "Failed" => RunEnding::Failed,
        _ => return None,
    })
}

/// The `loom.run.SelectionStrategy` field `name`.
pub fn strategy(input: &Input, name: &str) -> Option<SelectionStrategy> {
    Some(match input.get(name)?.as_text()? {
        "ReasoningModel" => SelectionStrategy::ReasoningModel,
        "FastTyped" => SelectionStrategy::FastTyped,
        "Rule" => SelectionStrategy::Rule,
        "Hybrid" => SelectionStrategy::Hybrid,
        _ => return None,
    })
}

/// The `Optional<loom.run.SelectionStrategy>` field `name`.
pub fn optional_strategy(input: &Input, name: &str) -> Option<Option<SelectionStrategy>> {
    match input.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(_) => strategy(input, name).map(Some),
    }
}

/// The `List<loom.run.CatalogueEntry>` field `name`.
pub fn entries(input: &Input, name: &str) -> Option<Vec<CatalogueEntry>> {
    input
        .get(name)?
        .as_seq()?
        .iter()
        .map(|item| {
            let members = item.as_map()?;
            Some(CatalogueEntry {
                action: members.get("action")?.as_text()?.to_owned(),
                status: match members.get("status")?.as_text()? {
                    "Admissible" => CatalogueEntryStatus::Admissible,
                    "ApprovalRequired" => CatalogueEntryStatus::ApprovalRequired,
                    _ => return None,
                },
            })
        })
        .collect()
}

/// The `Optional<loom.run.ReportedUsage>` field `name`.
pub fn optional_usage(input: &Input, name: &str) -> Option<Option<ReportedUsage>> {
    let members = match input.get(name) {
        None | Some(Node::Null) => return Some(None),
        Some(node) => node.as_map()?,
    };
    let required = |field: &str| members.get(field).and_then(integer_of);
    Some(Some(ReportedUsage {
        model: members.get("model")?.as_text()?.to_owned(),
        input_tokens: required("input_tokens")?,
        output_tokens: required("output_tokens")?,
        cached_input_tokens: required("cached_input_tokens")?,
        cache_creation_input_tokens: match members.get("cache_creation_input_tokens") {
            None | Some(Node::Null) => None,
            Some(node) => Some(integer_of(node)?),
        },
    }))
}

/// A `Uuid` (or a newtype of one) as a suite value.
pub fn id(value: &Uuid) -> Node {
    Node::Text(value.0.clone())
}

/// An `Integer` as a suite value, exactly.
pub fn number(value: i64) -> Node {
    Node::Number(Number::from(value))
}

/// The declared name of a run ending.
pub fn run_ending_name(ending: RunEnding) -> Node {
    name(match ending {
        RunEnding::Answered => "Answered",
        RunEnding::Stopped => "Stopped",
        RunEnding::Failed => "Failed",
    })
}

/// The declared name of a selection strategy.
pub fn strategy_name(strategy: SelectionStrategy) -> Node {
    name(match strategy {
        SelectionStrategy::ReasoningModel => "ReasoningModel",
        SelectionStrategy::FastTyped => "FastTyped",
        SelectionStrategy::Rule => "Rule",
        SelectionStrategy::Hybrid => "Hybrid",
    })
}

/// The declared name of a session state.
pub fn session_state(state: SessionState) -> Node {
    name(match state {
        SessionState::Active => "Active",
        SessionState::Filed => "Filed",
        SessionState::Interrupted => "Interrupted",
    })
}

/// The declared name of a selection state.
pub fn selection_state(state: SelectionState) -> Node {
    name(match state {
        SelectionState::Selected => "Selected",
        SelectionState::Admitted => "Admitted",
        SelectionState::Refused => "Refused",
        SelectionState::Overruled => "Overruled",
    })
}

/// The declared name of a selection record state.
pub fn selection_record_state(state: SelectionRecordState) -> Node {
    name(match state {
        SelectionRecordState::Recorded => "Recorded",
    })
}

/// The declared name of a catalogue state.
pub fn catalogue_state(state: ActionCatalogueState) -> Node {
    name(match state {
        ActionCatalogueState::Projected => "Projected",
    })
}

/// An `Optional<loom.run.ReportedUsage>` as a suite value.
pub fn usage(usage: Option<&ReportedUsage>) -> Node {
    let Some(usage) = usage else {
        return Node::Null;
    };
    Node::Map(BTreeMap::from([
        ("model".to_owned(), Node::Text(usage.model.clone())),
        ("input_tokens".to_owned(), number(usage.input_tokens)),
        ("output_tokens".to_owned(), number(usage.output_tokens)),
        (
            "cached_input_tokens".to_owned(),
            number(usage.cached_input_tokens),
        ),
        (
            "cache_creation_input_tokens".to_owned(),
            usage.cache_creation_input_tokens.map_or(Node::Null, number),
        ),
    ]))
}

fn name(text: &str) -> Node {
    Node::Text(text.to_owned())
}
