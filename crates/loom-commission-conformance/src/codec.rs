//! Values between the suite's vocabulary ([`Node`]) and the generated responsibility model.
//!
//! Decoding reads one declared input field into the generated type the model gives it, and answers
//! `None` when the value cannot be held by that type: the command then reaches no declared outcome.
//! Encoding writes a generated value back under the shape the model declares for it.

use std::collections::BTreeMap;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    BudgetLimit, CaseId, CommissionId, HostStop, HumanDecisionRequest, RunBudgetBreach, RunState,
    SuspensionKey, SuspensionReason, SuspensionState,
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

/// The `List<String>` field `name`; `None` when it is no list or holds anything but text.
pub fn text_list(input: &Input, name: &str) -> Option<Vec<String>> {
    match input.get(name)? {
        Node::Seq(items) => items
            .iter()
            .map(|item| item.as_text().map(ToOwned::to_owned))
            .collect(),
        _ => None,
    }
}

/// The `List<Uuid>` field `name`, each item in the canonical form [`uuid`] reads; `None` when it is
/// no list or holds anything else.
pub fn uuid_list(input: &Input, name: &str) -> Option<Vec<Uuid>> {
    match input.get(name)? {
        Node::Seq(items) => items
            .iter()
            .map(|item| {
                item.as_text()
                    .filter(|text| is_canonical_uuid(text))
                    .map(|text| Uuid(text.to_owned()))
            })
            .collect(),
        _ => None,
    }
}

/// The `Boolean` field `name`.
pub fn boolean(input: &Input, name: &str) -> Option<bool> {
    match input.get(name)? {
        Node::Bool(value) => Some(*value),
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

/// The declared name of a Suspension state.
pub fn suspension_state(state: SuspensionState) -> Node {
    Node::Text(
        match state {
            SuspensionState::Pending => "Pending",
            SuspensionState::Resuming => "Resuming",
        }
        .to_owned(),
    )
}

/// A `SuspensionKey`: the struct holding the commission's id, `commission_id`, and nothing else.
pub fn suspension_key(node: &Node) -> Option<SuspensionKey> {
    let members = node.as_map()?;
    if members.len() != 1 {
        return None;
    }
    let id = members.get("commission_id")?.as_text()?;
    is_canonical_uuid(id).then(|| SuspensionKey {
        commission_id: CommissionId(Uuid(id.to_owned())),
    })
}

/// A `SuspensionKey` as a suite value.
pub fn from_suspension_key(key: &SuspensionKey) -> Node {
    Node::Map(BTreeMap::from([(
        "commission_id".to_owned(),
        Node::Text(key.commission_id.0.0.clone()),
    )]))
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
        "LoopBudget" => SuspensionReason::LoopBudget(to_json(value)?),
        "Dependency" => {
            SuspensionReason::Dependency(strings(value)?.into_iter().map(CaseId).collect())
        }
        "Evidence" => SuspensionReason::Evidence(strings(value)?),
        "ExternalAvailability" => SuspensionReason::ExternalAvailability(to_json(value)?),
        "Human" => SuspensionReason::Human(HumanDecisionRequest(to_json(value)?)),
        "Time" => SuspensionReason::Time(to_json(value)?),
        "RunBudget" => SuspensionReason::RunBudget(run_budget_breach(value)?),
        "HostStop" => SuspensionReason::HostStop(host_stop(value)?),
        _ => return None,
    })
}

/// A `SuspensionReason` as a suite value, or `None` when its `Json` holds a number [`read_json`]
/// refuses.
pub(crate) fn from_suspension_reason(reason: &SuspensionReason) -> Option<Node> {
    let (kind, value) = match reason {
        SuspensionReason::Authority(value) => ("Authority", read_json(value)?),
        SuspensionReason::Budget(value) => ("Budget", read_json(value)?),
        SuspensionReason::LoopBudget(value) => ("LoopBudget", read_json(value)?),
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
        SuspensionReason::RunBudget(breach) => ("RunBudget", from_run_budget_breach(breach)),
        SuspensionReason::HostStop(stop) => ("HostStop", from_host_stop(stop)?),
    };
    Some(Node::Map(BTreeMap::from([
        ("kind".to_owned(), Node::Text(kind.to_owned())),
        ("value".to_owned(), value),
    ])))
}

/// The names of `BudgetLimit`, as `ess/commission/` declares them.
const LIMITS: [(BudgetLimit, &str); 8] = [
    (BudgetLimit::MaxSteps, "MaxSteps"),
    (BudgetLimit::MaxTrials, "MaxTrials"),
    (BudgetLimit::MaxTokensIn, "MaxTokensIn"),
    (BudgetLimit::MaxTokensOut, "MaxTokensOut"),
    (BudgetLimit::MaxDurationMs, "MaxDurationMs"),
    (BudgetLimit::MaxCostMicroUsd, "MaxCostMicroUsd"),
    (BudgetLimit::MaxTokens, "MaxTokens"),
    (BudgetLimit::Deadline, "Deadline"),
];

/// A `RunBudgetBreach`: its fields by name, an optional one absent or null when it holds nothing.
fn run_budget_breach(node: &Node) -> Option<RunBudgetBreach> {
    let members = node.as_map()?;
    let present = |name: &str| {
        members
            .get(name)
            .filter(|value| !matches!(value, Node::Null))
    };
    let integer = |node: &Node| match node {
        Node::Number(number) => number.as_i64(),
        _ => None,
    };
    let name = members.get("limit")?.as_text()?;
    Some(RunBudgetBreach {
        action: match present("action") {
            Some(action) => Some(action.as_text()?.to_owned()),
            None => None,
        },
        limit: LIMITS.iter().find(|(_, known)| *known == name)?.0,
        max: integer(members.get("max")?)?,
        charged: match present("charged") {
            Some(charged) => Some(integer(charged)?),
            None => None,
        },
    })
}

/// A `RunBudgetBreach` as a suite value, an absent optional field left out.
fn from_run_budget_breach(breach: &RunBudgetBreach) -> Node {
    let mut members = BTreeMap::new();
    if let Some(action) = &breach.action {
        members.insert("action".to_owned(), Node::Text(action.clone()));
    }
    let limit = LIMITS
        .iter()
        .find(|(limit, _)| *limit == breach.limit)
        .map_or("", |(_, name)| name);
    members.insert("limit".to_owned(), Node::Text(limit.to_owned()));
    members.insert("max".to_owned(), number(breach.max));
    if let Some(charged) = breach.charged {
        members.insert("charged".to_owned(), number(charged));
    }
    Node::Map(members)
}

/// A `HostStop`: its `name` text and its `detail`, any JSON.
fn host_stop(node: &Node) -> Option<HostStop> {
    let members = node.as_map()?;
    Some(HostStop {
        name: members.get("name")?.as_text()?.to_owned(),
        detail: to_json(members.get("detail")?)?,
    })
}

/// A `HostStop` as a suite value, or `None` when its detail holds a number [`read_json`] refuses.
fn from_host_stop(stop: &HostStop) -> Option<Node> {
    Some(Node::Map(BTreeMap::from([
        ("name".to_owned(), Node::Text(stop.name.clone())),
        ("detail".to_owned(), read_json(&stop.detail)?),
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host's stop is encoded as the suite reads it and decoded back unchanged
    /// (`story:host-stop-before-step`). A suite map orders its members by name, as for every
    /// `Json` payload, so the detail's members are written in that order here.
    #[test]
    fn a_host_stop_reason_round_trips_through_the_suite_codec() {
        let reason = SuspensionReason::HostStop(HostStop {
            name: "DiskLow".to_owned(),
            detail: Value::Object(vec![
                ("free_bytes".to_owned(), Value::Number("1048576".to_owned())),
                ("path".to_owned(), Value::Text("target".to_owned())),
                (
                    "paused".to_owned(),
                    Value::Array(vec![Value::Bool(true), Value::Null]),
                ),
            ]),
        });
        let node = from_suspension_reason(&reason).expect("the reason encodes");
        assert_eq!(
            node.as_map()
                .and_then(|members| members.get("kind")?.as_text()),
            Some("HostStop")
        );
        assert_eq!(suspension_reason(&node), Some(reason));
    }
}
