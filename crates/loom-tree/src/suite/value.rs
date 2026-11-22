//! The values a suite writes and a selection reads: the generated `Json` of `loom.tree` and the
//! `serde_json` value a file reads as, and how two of them compare.

use loom::json::Value as Written;
use serde_json::Value;

/// `value` as the generated `Json` a suite type holds.
pub(crate) fn written(value: &Value) -> Written {
    match value {
        Value::Null => Written::Null,
        Value::Bool(boolean) => Written::Bool(*boolean),
        Value::Number(number) => Written::Number(number.to_string()),
        Value::String(text) => Written::Text(text.clone()),
        Value::Array(items) => Written::Array(items.iter().map(written).collect()),
        Value::Object(members) => Written::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), written(member)))
                .collect(),
        ),
    }
}

/// The generated `Json` `value` as the value a selection compares it with.
pub(crate) fn read(value: &Written) -> Value {
    match value {
        Written::Null => Value::Null,
        Written::Bool(boolean) => Value::Bool(*boolean),
        Written::Number(spelling) => serde_json::from_str::<Value>(spelling)
            .ok()
            .filter(Value::is_number)
            .unwrap_or_else(|| Value::String(spelling.clone())),
        Written::Text(text) => Value::String(text.clone()),
        Written::Array(items) => Value::Array(items.iter().map(read).collect()),
        Written::Object(members) => Value::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), read(member)))
                .collect(),
        ),
    }
}

/// Whether `left` and `right` are the same value; two numbers are the same when they are equal as
/// numbers (`1` and `1.0`).
pub(crate) fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => match (a.as_i64(), b.as_i64()) {
            (Some(a), Some(b)) => a == b,
            _ => a.as_f64() == b.as_f64(),
        },
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(name, member)| b.get(name).is_some_and(|other| same(member, other)))
        }
        _ => left == right,
    }
}

/// The number `value` is, when it is one.
pub(crate) fn number(value: &Value) -> Option<f64> {
    value.as_f64()
}

/// `value` as the suite writes it, for the expected side of a difference: a text as itself, any
/// other value as compact JSON.
pub(crate) fn as_written(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The SHA-256 of `bytes` as 64 lower-case hex digits.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
