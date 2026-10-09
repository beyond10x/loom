//! The JSON of a [`LoopStop`]: the wire codec of the type `ess/domains/run.yaml` declares.
//!
//! `LoopStop` is generated (`loom.run.LoopStop`) and the generated crate derives no serde, so the
//! three places that carry a stop, `LoopOutcome.stop`, `LoopEvent::Finished.stop` and
//! `LoopEvent::DelegateFinished.stop`, name this module in `#[serde(with = …)]`. It writes exactly
//! what the loop wrote before the type moved to ESS: an object whose `kind` is the cause's
//! kebab-case tag, followed by the cause's fields under their declared names. It reads that back
//! and refuses an unknown tag, an unknown or repeated field, a missing field, and a figure that is
//! not a whole number from 0 to `i64::MAX`.
//!
//! It declares no type of its own for the causes: every tag maps straight to a variant of the
//! generated enum, so there is one model and this is only how it is spelled in JSON.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::{self, SerializeMap};
use serde::{Deserializer, Serializer};
use serde_json::{Map, Value};

use crate::model::run::{
    LoopStop, LoopStopAwaitingApproval, LoopStopBudgetUnobservable, LoopStopCancelled,
    LoopStopContextAboveTrigger, LoopStopDeadline, LoopStopMaxCost, LoopStopMaxInputTokens,
    LoopStopMaxOutputTokens, LoopStopMaxTurns, LoopStopProviderIncomplete, LoopStopUnstructured,
};

/// The member that names the cause.
const TAG: &str = "kind";

/// Every tag, in declaration order, for the message an unknown one gets.
const TAGS: [&str; 12] = [
    "completed",
    "max-turns",
    "max-input-tokens",
    "max-output-tokens",
    "max-cost",
    "budget-unobservable",
    "deadline",
    "cancelled",
    "awaiting-approval",
    "provider-incomplete",
    "unstructured",
    "context-above-trigger",
];

/// Writes `stop` as `{"kind": <tag>, <field>: <value>, …}`, fields in their declared order.
///
/// A negative figure is refused rather than written: nothing this codec writes may fail to read
/// back, and a reader refuses one.
pub(crate) fn serialize<S: Serializer>(stop: &LoopStop, serializer: S) -> Result<S::Ok, S::Error> {
    let tag = tag(stop);
    let len = match stop {
        LoopStop::Completed => 1,
        LoopStop::MaxTurns(_)
        | LoopStop::Deadline(_)
        | LoopStop::Cancelled(_)
        | LoopStop::AwaitingApproval(_)
        | LoopStop::ProviderIncomplete(_)
        | LoopStop::Unstructured(_) => 2,
        LoopStop::MaxInputTokens(_)
        | LoopStop::MaxOutputTokens(_)
        | LoopStop::MaxCost(_)
        | LoopStop::BudgetUnobservable(_) => 3,
        LoopStop::ContextAboveTrigger(_) => 4,
    };
    let mut map = serializer.serialize_map(Some(len))?;
    map.serialize_entry(TAG, tag)?;
    match stop {
        LoopStop::Completed => {}
        LoopStop::MaxTurns(LoopStopMaxTurns { limit }) => {
            write_figure(&mut map, tag, "limit", *limit)?
        }
        LoopStop::MaxInputTokens(LoopStopMaxInputTokens { limit, reported })
        | LoopStop::MaxOutputTokens(LoopStopMaxOutputTokens { limit, reported }) => {
            write_figure(&mut map, tag, "limit", *limit)?;
            write_figure(&mut map, tag, "reported", *reported)?;
        }
        LoopStop::MaxCost(LoopStopMaxCost {
            limit_micro_usd,
            spent_micro_usd,
        }) => {
            write_figure(&mut map, tag, "limit_micro_usd", *limit_micro_usd)?;
            write_figure(&mut map, tag, "spent_micro_usd", *spent_micro_usd)?;
        }
        LoopStop::Deadline(LoopStopDeadline { limit_ms }) => {
            write_figure(&mut map, tag, "limit_ms", *limit_ms)?
        }
        LoopStop::Unstructured(LoopStopUnstructured { asked_again }) => {
            write_figure(&mut map, tag, "asked_again", *asked_again)?;
        }
        LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
            window,
            target,
            occupied,
        }) => {
            write_figure(&mut map, tag, "window", *window)?;
            write_figure(&mut map, tag, "target", *target)?;
            write_figure(&mut map, tag, "occupied", *occupied)?;
        }
        LoopStop::BudgetUnobservable(LoopStopBudgetUnobservable { name, reason }) => {
            map.serialize_entry("name", name)?;
            map.serialize_entry("reason", reason)?;
        }
        LoopStop::Cancelled(LoopStopCancelled { reason })
        | LoopStop::ProviderIncomplete(LoopStopProviderIncomplete { reason }) => {
            map.serialize_entry("reason", reason)?;
        }
        LoopStop::AwaitingApproval(LoopStopAwaitingApproval { checkpoint_id }) => {
            map.serialize_entry("checkpoint_id", checkpoint_id)?;
        }
    }
    map.end()
}

/// Writes one figure of a stop, refusing a negative one.
fn write_figure<M: SerializeMap>(
    map: &mut M,
    tag: &str,
    name: &str,
    value: i64,
) -> Result<(), M::Error> {
    if value < 0 {
        return Err(ser::Error::custom(format!(
            "`{name}` of a `{tag}` stop is negative ({value}); a stop's figures are whole \
             numbers from 0"
        )));
    }
    map.serialize_entry(name, &value)
}

/// The tag `stop` is written under.
fn tag(stop: &LoopStop) -> &'static str {
    match stop {
        LoopStop::Completed => "completed",
        LoopStop::MaxTurns(_) => "max-turns",
        LoopStop::MaxInputTokens(_) => "max-input-tokens",
        LoopStop::MaxOutputTokens(_) => "max-output-tokens",
        LoopStop::MaxCost(_) => "max-cost",
        LoopStop::BudgetUnobservable(_) => "budget-unobservable",
        LoopStop::Deadline(_) => "deadline",
        LoopStop::Cancelled(_) => "cancelled",
        LoopStop::AwaitingApproval(_) => "awaiting-approval",
        LoopStop::ProviderIncomplete(_) => "provider-incomplete",
        LoopStop::Unstructured(_) => "unstructured",
        LoopStop::ContextAboveTrigger(_) => "context-above-trigger",
    }
}

/// Reads a stop [`serialize`] wrote, refusing anything else.
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<LoopStop, D::Error> {
    let members = deserializer.deserialize_map(Members)?;
    read(members).map_err(de::Error::custom)
}

/// The JSON value of `stop`, for a record that embeds one in a `serde_json::Value`.
pub(crate) fn to_value(stop: &LoopStop) -> Result<Value, serde_json::Error> {
    serialize(stop, serde_json::value::Serializer)
}

/// Collects an object's members, refusing a member named twice.
///
/// Each value is read as a [`Value`], which takes a number in whatever form the input hands it
/// over, `serde_json`'s `arbitrary_precision` buffer included.
struct Members;

impl<'de> Visitor<'de> for Members {
    type Value = Map<String, Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a loop stop: an object tagged by `kind`")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut members = Map::new();
        while let Some(name) = access.next_key::<String>()? {
            if members.contains_key(&name) {
                return Err(de::Error::custom(format!("duplicate field `{name}`")));
            }
            let value = access.next_value::<Value>()?;
            members.insert(name, value);
        }
        Ok(members)
    }
}

fn read(mut members: Map<String, Value>) -> Result<LoopStop, String> {
    let tag = match members.remove(TAG) {
        Some(Value::String(tag)) => tag,
        Some(other) => return Err(format!("`{TAG}` must be a string, not {other}")),
        None => return Err(format!("missing field `{TAG}`")),
    };
    let fields = &mut members;
    let stop = match tag.as_str() {
        "completed" => LoopStop::Completed,
        "max-turns" => LoopStop::MaxTurns(LoopStopMaxTurns {
            limit: read_figure(fields, "limit")?,
        }),
        "max-input-tokens" => LoopStop::MaxInputTokens(LoopStopMaxInputTokens {
            limit: read_figure(fields, "limit")?,
            reported: read_figure(fields, "reported")?,
        }),
        "max-output-tokens" => LoopStop::MaxOutputTokens(LoopStopMaxOutputTokens {
            limit: read_figure(fields, "limit")?,
            reported: read_figure(fields, "reported")?,
        }),
        "max-cost" => LoopStop::MaxCost(LoopStopMaxCost {
            limit_micro_usd: read_figure(fields, "limit_micro_usd")?,
            spent_micro_usd: read_figure(fields, "spent_micro_usd")?,
        }),
        "budget-unobservable" => LoopStop::BudgetUnobservable(LoopStopBudgetUnobservable {
            name: read_text(fields, "name")?,
            reason: read_text(fields, "reason")?,
        }),
        "deadline" => LoopStop::Deadline(LoopStopDeadline {
            limit_ms: read_figure(fields, "limit_ms")?,
        }),
        "cancelled" => LoopStop::Cancelled(LoopStopCancelled {
            reason: read_text(fields, "reason")?,
        }),
        "awaiting-approval" => LoopStop::AwaitingApproval(LoopStopAwaitingApproval {
            checkpoint_id: read_text(fields, "checkpoint_id")?,
        }),
        "provider-incomplete" => LoopStop::ProviderIncomplete(LoopStopProviderIncomplete {
            reason: read_text(fields, "reason")?,
        }),
        "unstructured" => LoopStop::Unstructured(LoopStopUnstructured {
            asked_again: read_figure(fields, "asked_again")?,
        }),
        "context-above-trigger" => LoopStop::ContextAboveTrigger(LoopStopContextAboveTrigger {
            window: read_figure(fields, "window")?,
            target: read_figure(fields, "target")?,
            occupied: read_figure(fields, "occupied")?,
        }),
        other => {
            return Err(format!(
                "unknown variant `{other}`, expected one of {}",
                TAGS.map(|tag| format!("`{tag}`")).join(", ")
            ));
        }
    };
    if let Some(name) = members.keys().next() {
        return Err(format!("unknown field `{name}` in a `{tag}` stop"));
    }
    Ok(stop)
}

fn read_figure(members: &mut Map<String, Value>, name: &str) -> Result<i64, String> {
    match members.remove(name) {
        None => Err(format!("missing field `{name}`")),
        Some(Value::Number(number)) => {
            number.as_i64().filter(|value| *value >= 0).ok_or_else(|| {
                format!(
                    "`{name}` must be a whole number from 0 to {}, not {number}",
                    i64::MAX
                )
            })
        }
        Some(other) => Err(format!("`{name}` must be a number, not {other}")),
    }
}

fn read_text(members: &mut Map<String, Value>, name: &str) -> Result<String, String> {
    match members.remove(name) {
        None => Err(format!("missing field `{name}`")),
        Some(Value::String(text)) => Ok(text),
        Some(other) => Err(format!("`{name}` must be a string, not {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tag_is_one_the_reader_knows() {
        // `TAGS` only feeds the message an unknown tag gets; a tag listed there that the reader
        // does not match would send a reader to a tag that is refused.
        for tag in TAGS {
            let mut members = Map::new();
            members.insert(TAG.to_owned(), Value::from(tag));
            match read(members) {
                Ok(stop) => assert_eq!(super::tag(&stop), tag),
                Err(message) => assert!(message.starts_with("missing field"), "{tag}: {message}"),
            }
        }
    }

    #[test]
    fn a_negative_figure_is_not_written() {
        let stop = LoopStop::MaxTurns(LoopStopMaxTurns { limit: -1 });
        let written = to_value(&stop);
        assert!(written.is_err(), "{written:?}");
    }

    #[test]
    fn a_member_named_twice_is_refused() {
        let read =
            serde_json::from_str::<Wrapped>(r#"{"stop":{"kind":"max-turns","limit":1,"limit":2}}"#);
        assert!(read.is_err(), "{read:?}");
    }

    #[test]
    fn a_figure_above_i64_is_refused() {
        let read = serde_json::from_str::<Wrapped>(
            r#"{"stop":{"kind":"deadline","limit_ms":9223372036854775808}}"#,
        );
        assert!(read.is_err(), "{read:?}");
    }

    #[derive(Debug, serde::Deserialize)]
    struct Wrapped {
        #[serde(with = "super")]
        #[allow(dead_code)]
        stop: LoopStop,
    }
}
