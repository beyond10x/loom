//! Values between the suite's vocabulary ([`Node`]) and the generated run and session models.
//!
//! Decoding reads one declared input field into the generated type the model gives it, and answers
//! `None` when the value cannot be held by that type: the command then reaches no declared outcome.
//! An `Optional` field reads as `Some(None)` when it is absent or null. Encoding writes a generated
//! value back under the shape the model declares for it, an absent `Optional` as null.

use std::collections::BTreeMap;

use b10x_loom_executor::model::primitives::{Decimal, Uuid};
use b10x_loom_executor::model::run::{
    ActionCatalogueState, CatalogueEntry, CatalogueEntryStatus, SelectionRecordState,
    SelectionState, SelectionStrategy,
};
use b10x_loom_executor::model::session::{AgentSessionState, ChangedFile, SessionKind};
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

/// The `Optional<Uuid>` field `name` (or of a newtype of one), canonical as [`uuid`] reads it;
/// `None` for any other value.
pub fn optional_uuid(input: &Input, name: &str) -> Option<Option<Uuid>> {
    match input.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Text(text)) if is_canonical_uuid(text) => Some(Some(Uuid(text.clone()))),
        Some(_) => None,
    }
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

/// A `Uuid` (or a newtype of one) as a suite value.
pub fn id(value: &Uuid) -> Node {
    Node::Text(value.0.clone())
}

/// An `Integer` as a suite value, exactly.
pub fn number(value: i64) -> Node {
    Node::Number(Number::from(value))
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

fn name(text: &str) -> Node {
    Node::Text(text.to_owned())
}

/// The `Optional<String>` field `name`.
pub fn optional_text(input: &Input, name: &str) -> Option<Option<String>> {
    match input.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Text(text)) => Some(Some(text.clone())),
        Some(_) => None,
    }
}

/// The `loom.session.SessionKind` field `name`.
pub fn session_kind(input: &Input, name: &str) -> Option<SessionKind> {
    Some(match input.get(name)?.as_text()? {
        "ClaudeCode" => SessionKind::ClaudeCode,
        "Codex" => SessionKind::Codex,
        "Harness" => SessionKind::Harness,
        _ => return None,
    })
}

/// The `List<loom.session.ChangedFile>` field `name`, a member's absent or null `content` a
/// removal.
pub fn changed_files(input: &Input, name: &str) -> Option<Vec<ChangedFile>> {
    input
        .get(name)?
        .as_seq()?
        .iter()
        .map(|item| {
            let members = item.as_map()?;
            Some(ChangedFile {
                path: members.get("path")?.as_text()?.to_owned(),
                content: match members.get("content") {
                    None | Some(Node::Null) => None,
                    Some(Node::Text(text)) => Some(text.clone()),
                    Some(_) => return None,
                },
            })
        })
        .collect()
}

/// A `String` as a suite value.
pub fn text_node(text: &str) -> Node {
    name(text)
}

/// The declared name of a session kind.
pub fn session_kind_name(kind: SessionKind) -> Node {
    name(match kind {
        SessionKind::ClaudeCode => "ClaudeCode",
        SessionKind::Codex => "Codex",
        SessionKind::Harness => "Harness",
    })
}

/// A `List<loom.session.ChangedFile>` as a suite value, a removal's `content` null.
pub fn changed_files_node(files: &[ChangedFile]) -> Node {
    Node::Seq(
        files
            .iter()
            .map(|file| {
                Node::Map(BTreeMap::from([
                    ("path".to_owned(), name(&file.path)),
                    (
                        "content".to_owned(),
                        file.content.as_deref().map_or(Node::Null, name),
                    ),
                ]))
            })
            .collect(),
    )
}

/// The declared name of an agent session state.
pub fn agent_session_state(state: AgentSessionState) -> Node {
    name(match state {
        AgentSessionState::Ended => "Ended",
    })
}
