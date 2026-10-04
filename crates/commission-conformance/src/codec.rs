//! Values between the suite's vocabulary ([`Node`]) and the generated responsibility model.
//!
//! Decoding reads one declared input field into the generated type the model gives it, and answers
//! `None` when the value cannot be held by that type: the command then reaches no declared outcome.
//! Encoding writes a generated value back under the shape the model declares for it.

use std::collections::BTreeMap;

use b10x_commission::model::json::Value;
use b10x_commission::model::primitives::Uuid;
use b10x_commission::model::responsibility::{
    CaseId, HumanDecisionRequest, RunState, SuspensionReason,
};
use ess_primitives::facts::{Number, is_canonical_uuid};
use ess_primitives::node::Node;

/// The declared input of one command invocation.
pub type Input = BTreeMap<String, Node>;

/// The text field `name`.
pub fn text(input: &Input, name: &str) -> Option<String> {
    input.get(name)?.as_text().map(ToOwned::to_owned)
}

/// The `Uuid` field `name`, in the canonical hyphenated form the generated model carries and its
/// wire reader (`model::json::uuid_at`) accepts; `None` for any other text.
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

/// The `Json` field `name`.
pub fn json(input: &Input, name: &str) -> Option<Value> {
    input.get(name).and_then(to_json)
}

/// A `Json` value of the model, from a suite value.
pub fn to_json(node: &Node) -> Option<Value> {
    Some(match node {
        Node::Null => Value::Null,
        Node::Bool(value) => Value::Bool(*value),
        Node::Number(number) => Value::Number(number.exact_text()),
        Node::Text(text) => Value::Text(text.clone()),
        Node::Seq(items) => Value::Array(items.iter().map(to_json).collect::<Option<_>>()?),
        Node::Map(members) => Value::Object(
            members
                .iter()
                .map(|(name, value)| Some((name.clone(), to_json(value)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

/// A suite value, from a `Json` value of the model that [`to_json`] wrote.
///
/// # Panics
///
/// When `value` holds a number spelling [`to_json`] never writes; see [`read_json`], which
/// answers `None` for it instead and is what the target uses.
pub fn from_json(value: &Value) -> Node {
    read_json(value).unwrap_or_else(|| {
        panic!("a Json value holds a number spelling `to_json` never writes: {value:?}")
    })
}

/// A suite value, from a `Json` value of the model, or `None` when it holds a number that cannot
/// be held exactly as a [`Number`].
pub(crate) fn read_json(value: &Value) -> Option<Node> {
    Some(match value {
        Value::Null => Node::Null,
        Value::Bool(value) => Node::Bool(*value),
        Value::Number(spelling) => Node::Number(number_of(spelling)?),
        Value::Text(text) => Node::Text(text.clone()),
        Value::Array(items) => Node::Seq(items.iter().map(read_json).collect::<Option<_>>()?),
        Value::Object(members) => Node::Map(
            members
                .iter()
                .map(|(name, value)| Some((name.clone(), read_json(value)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

/// A number of the model's `Json`, read back exactly.
///
/// [`to_json`] writes every number with [`Number::exact_text`], and [`Number::decimal_literal`]
/// reads back every spelling `exact_text` writes as the value it was, the magnitudes ESS holds only
/// as a binary64 included (`tests/adversary2_conform_codec.rs` runs the round trip over 35,000
/// numbers). There is therefore no fallback: a spelling `decimal_literal` refuses is one `to_json`
/// did not write, such as an exponent, and it is refused rather than read some other way.
fn number_of(spelling: &str) -> Option<Number> {
    Number::decimal_literal(spelling)
}

/// An `Integer` as a suite value, exactly.
pub fn number(value: i64) -> Node {
    Node::Number(Number::from(value))
}

/// A list of text as a suite value.
pub fn texts(values: &[String]) -> Node {
    Node::Seq(values.iter().cloned().map(Node::Text).collect())
}

/// The declared name of a run state.
pub fn run_state(state: RunState) -> Node {
    Node::Text(
        match state {
            RunState::Running => "Running",
            RunState::Suspended => "Suspended",
        }
        .to_owned(),
    )
}

/// A `SuspensionReason`: the union tagged `kind`, its payload under `value`.
pub fn suspension_reason(node: &Node) -> Option<SuspensionReason> {
    let members = node.as_map()?;
    let value = members.get("value")?;
    let strings = |value: &Node| -> Option<Vec<String>> {
        value
            .as_seq()?
            .iter()
            .map(|item| item.as_text().map(ToOwned::to_owned))
            .collect()
    };
    Some(match members.get("kind")?.as_text()? {
        "Authority" => SuspensionReason::Authority(to_json(value)?),
        "Budget" => SuspensionReason::Budget(to_json(value)?),
        "Dependency" => {
            SuspensionReason::Dependency(strings(value)?.into_iter().map(CaseId).collect())
        }
        "Evidence" => SuspensionReason::Evidence(strings(value)?),
        "ExternalAvailability" => SuspensionReason::ExternalAvailability(to_json(value)?),
        "Human" => SuspensionReason::Human(HumanDecisionRequest(to_json(value)?)),
        "Time" => SuspensionReason::Time(to_json(value)?),
        _ => return None,
    })
}

/// A `SuspensionReason` as a suite value, or `None` when its `Json` holds a number [`read_json`]
/// refuses.
pub(crate) fn from_suspension_reason(reason: &SuspensionReason) -> Option<Node> {
    let (kind, value) = match reason {
        SuspensionReason::Authority(value) => ("Authority", read_json(value)?),
        SuspensionReason::Budget(value) => ("Budget", read_json(value)?),
        SuspensionReason::Dependency(cases) => (
            "Dependency",
            Node::Seq(
                cases
                    .iter()
                    .map(|case| Node::Text(case.0.clone()))
                    .collect(),
            ),
        ),
        SuspensionReason::Evidence(requirements) => ("Evidence", texts(requirements)),
        SuspensionReason::ExternalAvailability(value) => {
            ("ExternalAvailability", read_json(value)?)
        }
        SuspensionReason::Human(request) => ("Human", read_json(&request.0)?),
        SuspensionReason::Time(value) => ("Time", read_json(value)?),
    };
    Some(Node::Map(BTreeMap::from([
        ("kind".to_owned(), Node::Text(kind.to_owned())),
        ("value".to_owned(), value),
    ])))
}
