//! The Connectors invoke admission: the body is one JSON object whose top level names
//! `operation` once, as a string, and that string is a declared operation.
//!
//! The body is read as JSON Connectors reads it (escapes decoded, so `"operation"` is the
//! key `operation`), and every top-level key must be unique: a parser that kept the first of two
//! `operation` keys and one that kept the last would disagree on what is invoked. The admitted
//! body is forwarded as received, never re-serialized.

use std::collections::{BTreeSet, HashSet};
use std::fmt;

use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};

/// Why an invoke body was refused. Each message is fixed text: none repeats a request value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvokeRefusal {
    NotAnObject,
    DuplicatedKey,
    NoOperation,
    OperationNotAString,
    OperationNotDeclared,
}

impl InvokeRefusal {
    pub(crate) fn message(self) -> &'static str {
        match self {
            InvokeRefusal::NotAnObject => "the invoke body is not one JSON object",
            InvokeRefusal::DuplicatedKey => "the invoke body repeats a top-level key",
            InvokeRefusal::NoOperation => "the invoke body names no operation",
            InvokeRefusal::OperationNotAString => "the invoke body's operation is not a string",
            InvokeRefusal::OperationNotDeclared => {
                "the operation is not one this boundary declares"
            }
        }
    }
}

/// Admits `body` when its top-level `operation` is one of `declared`.
pub(crate) fn admit(body: &[u8], declared: &BTreeSet<String>) -> Result<(), InvokeRefusal> {
    let top: TopLevel = serde_json::from_slice(body).map_err(|_| InvokeRefusal::NotAnObject)?;
    if top.duplicated {
        return Err(InvokeRefusal::DuplicatedKey);
    }
    match top.operation {
        None => Err(InvokeRefusal::NoOperation),
        Some(serde_json::Value::String(operation)) if declared.contains(&operation) => Ok(()),
        Some(serde_json::Value::String(_)) => Err(InvokeRefusal::OperationNotDeclared),
        Some(_) => Err(InvokeRefusal::OperationNotAString),
    }
}

/// The top level of an invoke body: its `operation` value and whether any key repeats.
struct TopLevel {
    operation: Option<serde_json::Value>,
    duplicated: bool,
}

impl<'de> Deserialize<'de> for TopLevel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(TopLevelVisitor)
    }
}

struct TopLevelVisitor;

impl<'de> Visitor<'de> for TopLevelVisitor {
    type Value = TopLevel;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<TopLevel, A::Error> {
        let mut seen = HashSet::new();
        let mut top = TopLevel {
            operation: None,
            duplicated: false,
        };
        while let Some(key) = map.next_key::<String>()? {
            if key == "operation" && !seen.contains(&key) {
                top.operation = Some(map.next_value()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
            if !seen.insert(key) {
                top.duplicated = true;
            }
        }
        Ok(top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> BTreeSet<String> {
        ["merge_request.get".to_owned()].into()
    }

    #[test]
    fn each_refusal_has_its_reason() {
        let cases: [(&str, Result<(), InvokeRefusal>); 8] = [
            (r#"{"operation":"merge_request.get"}"#, Ok(())),
            (r#" {"input":{},"operation":"merge_request.get"} "#, Ok(())),
            (
                r#"{"operation":"other"}"#,
                Err(InvokeRefusal::OperationNotDeclared),
            ),
            (
                r#"{"operation":7}"#,
                Err(InvokeRefusal::OperationNotAString),
            ),
            (r#"{"input":{}}"#, Err(InvokeRefusal::NoOperation)),
            (
                r#"{"a":1,"a":2,"operation":"merge_request.get"}"#,
                Err(InvokeRefusal::DuplicatedKey),
            ),
            (r#"[1]"#, Err(InvokeRefusal::NotAnObject)),
            (
                r#"{"operation":"merge_request.get"}x"#,
                Err(InvokeRefusal::NotAnObject),
            ),
        ];
        for (body, expected) in cases {
            assert_eq!(admit(body.as_bytes(), &declared()), expected, "{body}");
        }
        // A lone surrogate is not text Connectors could decode as a declared operation.
        assert!(admit(br#"{"operation":"\ud800"}"#, &declared()).is_err());
    }
}
