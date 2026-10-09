//! The JSON of a record line and of the state, over the generated `loom.plugin` types.
//!
//! A record line is one object: `item`, `intent` (`ask`, `request`, `task`, `find`) and
//! `confidence` when classified, `outcome` (`proposed`, `declined`, `proposed_case`,
//! `unclassified`, `stopped`), `reads` (`[{"source", "kind"}]`, `kind` `list`, `search` or `get`),
//! and `proposal`, `proposed_case` (`{"protocol", "confidence", "reasons"}`) and `detail` when
//! present. An absent optional field is omitted. The state is `{"cursors": [{"name", "value"}],
//! "handled": [<item id>], "failing": [{"item", "failures", "last_failure"}]}`, a failing item
//! whole: `{"id", "revision", "text", "details"}`, `details` the JSON value it is. A decimal is
//! written as the JSON number it spells, or as a string when it spells none.

use loom::datasource::{ReadKind, SourceName};
use loom::json::{self, DecodeError, Value, member};
use loom::plugin::{
    Cursor, InboundItem, Intent, ItemFailures, ItemId, PluginState, ProposedCase, RecordLine,
    RecordOutcome, SourceRead,
};
use loom::primitives::Decimal;

const INTENTS: [(&str, Intent); 4] = [
    ("ask", Intent::Ask),
    ("request", Intent::Request),
    ("task", Intent::Task),
    ("find", Intent::Find),
];

const OUTCOMES: [(&str, RecordOutcome); 5] = [
    ("proposed", RecordOutcome::Proposed),
    ("declined", RecordOutcome::Declined),
    ("proposed_case", RecordOutcome::ProposedCase),
    ("unclassified", RecordOutcome::Unclassified),
    ("stopped", RecordOutcome::Stopped),
];

const KINDS: [(&str, ReadKind); 3] = [
    ("list", ReadKind::List),
    ("search", ReadKind::Search),
    ("get", ReadKind::Get),
];

/// The name `table` gives `value`.
fn name_of<T: PartialEq + Copy>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, held)| *held == value)
        .map(|(name, _)| *name)
        .expect("every variant is named")
}

/// The variant `table` names `name`.
fn named<T: Copy>(table: &[(&str, T)], value: &Value, at: &str) -> Result<T, DecodeError> {
    let text = json::text_at(value, at, "a name")?;
    table
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, held)| *held)
        .ok_or_else(|| DecodeError::of(at, "a known name", value))
}

/// The name of a read kind, as a record line and a turn's arguments spell it.
pub(crate) fn kind_name(kind: ReadKind) -> &'static str {
    name_of(&KINDS, kind)
}

/// The read kind `name` spells.
pub(crate) fn kind_named(name: &str) -> Option<ReadKind> {
    KINDS
        .iter()
        .find(|(held, _)| *held == name)
        .map(|(_, kind)| *kind)
}

/// The name of an intent.
pub(crate) fn intent_name(intent: Intent) -> &'static str {
    name_of(&INTENTS, intent)
}

fn push_decimal(out: &mut String, value: &Decimal) {
    match json::parse(value.0.trim()) {
        Ok(Value::Number(spelling)) => out.push_str(&spelling),
        _ => json::push_text(out, &value.0),
    }
}

fn decimal_at(value: &Value, at: &str) -> Result<Decimal, DecodeError> {
    match value {
        Value::Number(spelling) => Ok(Decimal(spelling.clone())),
        Value::Text(text) => Ok(Decimal(text.clone())),
        other => Err(DecodeError::of(at, "a decimal", other)),
    }
}

/// One record line as one line of JSON, without its newline.
pub fn encode_record_line(line: &RecordLine) -> String {
    let mut out = String::from("{");
    member(&mut out, "item");
    json::push_text(&mut out, &line.item.0);
    if let Some(intent) = line.intent {
        member(&mut out, "intent");
        json::push_text(&mut out, intent_name(intent));
    }
    if let Some(confidence) = &line.confidence {
        member(&mut out, "confidence");
        push_decimal(&mut out, confidence);
    }
    member(&mut out, "outcome");
    json::push_text(&mut out, name_of(&OUTCOMES, line.outcome));
    member(&mut out, "reads");
    out.push('[');
    for (at, read) in line.reads.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        out.push('{');
        member(&mut out, "source");
        json::push_text(&mut out, &read.source.0);
        member(&mut out, "kind");
        json::push_text(&mut out, kind_name(read.kind));
        out.push('}');
    }
    out.push(']');
    if let Some(proposal) = &line.proposal {
        member(&mut out, "proposal");
        json::push_text(&mut out, proposal);
    }
    if let Some(case) = &line.proposed_case {
        member(&mut out, "proposed_case");
        out.push('{');
        member(&mut out, "protocol");
        json::push_text(&mut out, &case.protocol);
        member(&mut out, "confidence");
        push_decimal(&mut out, &case.confidence);
        member(&mut out, "reasons");
        out.push('[');
        for (at, reason) in case.reasons.iter().enumerate() {
            if at > 0 {
                out.push(',');
            }
            json::push_text(&mut out, reason);
        }
        out.push(']');
        out.push('}');
    }
    if let Some(detail) = &line.detail {
        member(&mut out, "detail");
        json::push_text(&mut out, detail);
    }
    out.push('}');
    out
}

/// One record line read back from its JSON.
///
/// # Errors
/// Text that is no JSON, or JSON that is not a record line.
pub fn decode_record_line(text: &str) -> Result<RecordLine, String> {
    let value = json::parse(text).map_err(|error| format!("{error:?}"))?;
    record_line(&value).map_err(|error| format!("{error:?}"))
}

fn optional<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value.member(name).filter(|member| **member != Value::Null)
}

fn strings(value: &Value, at: &str) -> Result<Vec<String>, DecodeError> {
    json::items_at(value, at, "a list of strings")?
        .iter()
        .map(|item| json::text_at(item, at, "a string").map(str::to_owned))
        .collect()
}

fn record_line(value: &Value) -> Result<RecordLine, DecodeError> {
    let at = "record line";
    let reads = json::items_at(json::member_at(value, at, "reads")?, "reads", "a list")?
        .iter()
        .map(|read| {
            Ok(SourceRead {
                source: SourceName(
                    json::text_at(
                        json::member_at(read, "reads", "source")?,
                        "source",
                        "a name",
                    )?
                    .to_owned(),
                ),
                kind: named(&KINDS, json::member_at(read, "reads", "kind")?, "kind")?,
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
    let proposed_case = optional(value, "proposed_case")
        .map(|case| {
            Ok::<_, DecodeError>(ProposedCase {
                protocol: json::text_at(
                    json::member_at(case, "proposed_case", "protocol")?,
                    "protocol",
                    "a protocol",
                )?
                .to_owned(),
                confidence: decimal_at(
                    json::member_at(case, "proposed_case", "confidence")?,
                    "confidence",
                )?,
                reasons: strings(
                    json::member_at(case, "proposed_case", "reasons")?,
                    "reasons",
                )?,
            })
        })
        .transpose()?;
    Ok(RecordLine {
        item: ItemId(
            json::text_at(json::member_at(value, at, "item")?, "item", "an id")?.to_owned(),
        ),
        intent: optional(value, "intent")
            .map(|intent| named(&INTENTS, intent, "intent"))
            .transpose()?,
        confidence: optional(value, "confidence")
            .map(|confidence| decimal_at(confidence, "confidence"))
            .transpose()?,
        outcome: named(&OUTCOMES, json::member_at(value, at, "outcome")?, "outcome")?,
        reads,
        proposal: optional(value, "proposal")
            .map(|text| json::text_at(text, "proposal", "a string").map(str::to_owned))
            .transpose()?,
        proposed_case,
        detail: optional(value, "detail")
            .map(|text| json::text_at(text, "detail", "a string").map(str::to_owned))
            .transpose()?,
    })
}

/// The state as JSON.
pub(crate) fn encode_state(state: &PluginState) -> String {
    let mut out = String::from("{");
    member(&mut out, "cursors");
    out.push('[');
    for (at, cursor) in state.cursors.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        out.push('{');
        member(&mut out, "name");
        json::push_text(&mut out, &cursor.name);
        member(&mut out, "value");
        json::push_text(&mut out, &cursor.value);
        out.push('}');
    }
    out.push(']');
    member(&mut out, "handled");
    out.push('[');
    for (at, id) in state.handled.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        json::push_text(&mut out, &id.0);
    }
    out.push(']');
    member(&mut out, "failing");
    out.push('[');
    for (at, failing) in state.failing.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        out.push('{');
        member(&mut out, "item");
        push_item(&mut out, &failing.item);
        member(&mut out, "failures");
        json::push_integer(&mut out, failing.failures);
        member(&mut out, "last_failure");
        json::push_text(&mut out, &failing.last_failure);
        out.push('}');
    }
    out.push(']');
    out.push('}');
    out
}

/// An inbound item as one JSON object.
fn push_item(out: &mut String, item: &InboundItem) {
    out.push('{');
    member(out, "id");
    json::push_text(out, &item.id.0);
    member(out, "revision");
    json::push_text(out, &item.revision);
    member(out, "text");
    json::push_text(out, &item.text);
    member(out, "details");
    json::push_value(out, &item.details);
    out.push('}');
}

/// The inbound item `value` holds.
fn item_at(value: &Value) -> Result<InboundItem, DecodeError> {
    let text = |name: &str| {
        json::text_at(json::member_at(value, "item", name)?, name, "a string").map(str::to_owned)
    };
    Ok(InboundItem {
        id: ItemId(text("id")?),
        revision: text("revision")?,
        text: text("text")?,
        details: json::member_at(value, "item", "details")?.clone(),
    })
}

/// The state read back from its JSON.
pub(crate) fn decode_state(text: &str) -> Result<PluginState, String> {
    let value = json::parse(text).map_err(|error| format!("{error:?}"))?;
    let decoded = (|| {
        let cursors = json::items_at(
            json::member_at(&value, "state", "cursors")?,
            "cursors",
            "a list",
        )?
        .iter()
        .map(|cursor| {
            Ok(Cursor {
                name: json::text_at(
                    json::member_at(cursor, "cursors", "name")?,
                    "name",
                    "a name",
                )?
                .to_owned(),
                value: json::text_at(
                    json::member_at(cursor, "cursors", "value")?,
                    "value",
                    "a value",
                )?
                .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
        let handled = strings(json::member_at(&value, "state", "handled")?, "handled")?
            .into_iter()
            .map(ItemId)
            .collect();
        let failing = json::items_at(
            json::member_at(&value, "state", "failing")?,
            "failing",
            "a list",
        )?
        .iter()
        .map(|failing| {
            Ok(ItemFailures {
                item: item_at(json::member_at(failing, "failing", "item")?)?,
                failures: json::integer_at(
                    json::member_at(failing, "failing", "failures")?,
                    "failures",
                    "a count",
                )?,
                last_failure: json::text_at(
                    json::member_at(failing, "failing", "last_failure")?,
                    "last_failure",
                    "a reason",
                )?
                .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, DecodeError>>()?;
        Ok::<_, DecodeError>(PluginState {
            cursors,
            handled,
            failing,
        })
    })();
    decoded.map_err(|error| format!("{error:?}"))
}
