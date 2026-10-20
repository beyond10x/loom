//! Values between the suite's vocabulary ([`Node`]) and the generated intake model.
//!
//! Decoding reads one declared input field into the generated type the model gives it, and answers
//! `None` when the value cannot be held by that type: the command then reaches no declared outcome.
//! Encoding writes a generated value back under the shape the model declares for it.

use std::collections::BTreeMap;

use ess_primitives::facts::{Number, is_canonical_uuid};
use ess_primitives::node::Node;
use intake_model::confinement::Backend;
use intake_model::context::ContextPolicy;
use intake_model::primitives::{Decimal, Uuid};
use intake_model::routing::{AcceptedRoute, CatalogDigest, MergeHandoffState, ResumeContextState};

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
    match input.get(name)? {
        Node::Number(number) => number.as_i64(),
        _ => None,
    }
}

/// The `Optional<String>` field `name`: `Some(None)` when it is absent or null, `None` when it
/// holds anything but text.
pub fn optional_text(input: &Input, name: &str) -> Option<Option<String>> {
    match input.get(name) {
        None | Some(Node::Null) => Some(None),
        Some(Node::Text(text)) => Some(Some(text.clone())),
        Some(_) => None,
    }
}

/// The `List<String>` field `name`; `None` when it is no list or holds anything but text.
pub fn text_list(input: &Input, name: &str) -> Option<Vec<String>> {
    items(input.get(name)?)
}

fn items(node: &Node) -> Option<Vec<String>> {
    match node {
        Node::Seq(items) => items
            .iter()
            .map(|item| item.as_text().map(ToOwned::to_owned))
            .collect(),
        _ => None,
    }
}

/// The `intake.confinement.Backend` field `name`, by its declared variant name.
pub fn backend(input: &Input, name: &str) -> Option<Backend> {
    match input.get(name)?.as_text()? {
        "Substrate" => Some(Backend::Substrate),
        "None" => Some(Backend::None),
        _ => None,
    }
}

/// The `intake.routing.AcceptedRoute` field `name`: an object naming exactly its declared members,
/// its threshold kept as the exact decimal the suite wrote.
pub fn route(input: &Input, name: &str) -> Option<AcceptedRoute> {
    let Node::Map(route) = input.get(name)? else {
        return None;
    };
    let declared = [
        "protocol",
        "intent_revision",
        "catalog",
        "context_policy",
        "threshold",
    ];
    if route.len() != declared.len() || declared.iter().any(|member| !route.contains_key(*member)) {
        return None;
    }
    let Node::Seq(catalog) = route.get("catalog")? else {
        return None;
    };
    let catalog = catalog
        .iter()
        .map(|entry| {
            let Node::Map(entry) = entry else {
                return None;
            };
            if entry.len() != 2 {
                return None;
            }
            Some(CatalogDigest {
                name: entry.get("name")?.as_text()?.to_owned(),
                digest: entry.get("digest")?.as_text()?.to_owned(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let Node::Number(threshold) = route.get("threshold")? else {
        return None;
    };
    Some(AcceptedRoute {
        protocol: route.get("protocol")?.as_text()?.to_owned(),
        intent_revision: route.get("intent_revision")?.as_text()?.to_owned(),
        catalog,
        context_policy: match route.get("context_policy")?.as_text()? {
            "Legacy" => ContextPolicy::Legacy,
            "Bounded" => ContextPolicy::Bounded,
            _ => return None,
        },
        threshold: Decimal(threshold.exact_text()),
    })
}

/// An `Integer` as a suite value, exactly.
pub fn number(value: i64) -> Node {
    Node::Number(Number::from(value))
}

/// A list of text as a suite value.
pub fn texts(values: &[String]) -> Node {
    Node::Seq(values.iter().cloned().map(Node::Text).collect())
}

/// An `Optional<String>` as a suite value: null when absent.
pub fn optional(value: Option<String>) -> Node {
    value.map_or(Node::Null, Node::Text)
}

/// The declared name of a backend.
pub fn backend_name(backend: Backend) -> Node {
    Node::Text(
        match backend {
            Backend::Substrate => "Substrate",
            Backend::None => "None",
        }
        .to_owned(),
    )
}

/// The declared name of a resume context's state.
pub fn context_state(state: ResumeContextState) -> Node {
    Node::Text(
        match state {
            ResumeContextState::Kept => "Kept",
        }
        .to_owned(),
    )
}

/// The declared name of a hand-off's state.
pub fn handoff_state(state: MergeHandoffState) -> Node {
    Node::Text(
        match state {
            MergeHandoffState::Recorded => "Recorded",
        }
        .to_owned(),
    )
}
