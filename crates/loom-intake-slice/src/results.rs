//! Run-local immutable observations. References select bytes, never authority or expressions.
use intake_model::results::{
    Capture, Rendering, ResultReference, ResultSelector, SelectionKind, StoredResult,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const STORE_BYTES: usize = 64 * 1024 * 1024;
const ARTIFACT_COUNT: usize = 1024;
const ORIGIN_BYTES: usize = 4096;
const JSON_DEPTH: usize = 128;
const JSON_NODES: usize = 65_536;
const COMPOSITION_SEGMENTS: usize = 256;
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Entry {
    descriptor: StoredResult,
    text: String,
}

/// An in-memory capability scoped to one host-created briefing/run.
/// Identical payloads receive distinct IDs so their observation provenance survives.
#[derive(Debug)]
pub struct ResultStore {
    scope: String,
    entries: HashMap<String, Entry>,
    insertion_order: Vec<String>,
    bytes: usize,
    capacity_bytes: usize,
    capacity_count: usize,
}
impl Default for ResultStore {
    fn default() -> Self {
        Self::new()
    }
}
impl ResultStore {
    /// The host, never model input, creates the scope. No payload is written to disk.
    pub fn new() -> Self {
        Self::with_capacity(STORE_BYTES, ARTIFACT_COUNT)
    }

    /// Separate run-local stores share reference semantics, not capacity or identifiers.
    pub(crate) fn with_capacity(capacity_bytes: usize, capacity_count: usize) -> Self {
        let instant = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let sequence = NEXT_SCOPE.fetch_add(1, Ordering::Relaxed);
        Self {
            scope: format!("{instant:x}-{:x}-{sequence:x}", std::process::id()),
            entries: HashMap::new(),
            insertion_order: Vec::new(),
            bytes: 0,
            capacity_bytes,
            capacity_count,
        }
    }

    pub fn insert(
        &mut self,
        origin: &str,
        text: String,
        capture: Capture,
    ) -> Result<StoredResult, String> {
        let next_bytes = self.preflight(origin, text.len())?;
        Ok(self.insert_checked(origin, text, capture, next_bytes))
    }

    /// Check admission before allocating an owned copy of borrowed source content.
    pub fn insert_text(
        &mut self,
        origin: &str,
        text: &str,
        capture: Capture,
    ) -> Result<StoredResult, String> {
        let next_bytes = self.preflight(origin, text.len())?;
        Ok(self.insert_checked(origin, text.to_owned(), capture, next_bytes))
    }

    fn preflight(&self, origin: &str, text_bytes: usize) -> Result<usize, String> {
        if text_bytes > ARTIFACT_BYTES {
            return Err("result exceeds 16 MiB artifact limit".into());
        }
        if origin.len() > ORIGIN_BYTES {
            return Err("result origin exceeds 4096 byte metadata limit".into());
        }
        if self.entries.len() >= self.capacity_count {
            return Err(format!(
                "result store exceeds {} artifact limit",
                self.capacity_count
            ));
        }
        self.bytes
            .checked_add(text_bytes)
            .filter(|n| *n <= self.capacity_bytes)
            .ok_or_else(|| {
                format!(
                    "result store exceeds {} byte payload limit",
                    self.capacity_bytes
                )
            })
    }

    fn insert_checked(
        &mut self,
        origin: &str,
        text: String,
        capture: Capture,
        next_bytes: usize,
    ) -> StoredResult {
        let result_id = format!("result-{}-{}", self.scope, self.entries.len());
        let stored = StoredResult {
            result_id: result_id.clone(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            origin: origin.into(),
            capture,
            utf8_bytes: text.len() as i64,
        };
        self.insertion_order.push(result_id.clone());
        self.entries.insert(
            result_id,
            Entry {
                descriptor: stored.clone(),
                text,
            },
        );
        self.bytes = next_bytes;
        stored
    }

    /// List retained metadata in insertion order, including results omitted from a transcript.
    /// Page size clamps to 1..=8. Out-of-range offsets return an empty terminal page.
    /// Compact encoding stays within 8 KiB; display origins truncate at 256 UTF-8 bytes.
    pub fn list_results(&self, offset: usize, limit: usize) -> Value {
        let start = offset.min(self.insertion_order.len());
        let end = start
            .saturating_add(limit.clamp(1, 8))
            .min(self.insertion_order.len());
        let mut results = Vec::new();
        let mut encoded_bytes = 64; // Envelope, separators, and next_offset (at most 1024).
        for id in &self.insertion_order[start..end] {
            let stored = &self.entries[id].descriptor;
            let mut origin_end = stored.origin.len().min(256);
            while !stored.origin.is_char_boundary(origin_end) {
                origin_end -= 1;
            }
            let item = json!({
                "origin":&stored.origin[..origin_end],
                "origin_truncated":origin_end < stored.origin.len(),
                "capture":match stored.capture {Capture::Complete=>"complete",Capture::Partial=>"partial"},
                "utf8_bytes":stored.utf8_bytes,
                "reference":{"result":stored.result_id,"sha256":stored.sha256,
                    "select":{"kind":"whole"},"rendering":"text"}
            });
            let item_bytes = item.to_string().len() + 1;
            if encoded_bytes + item_bytes > 8192 {
                break;
            }
            encoded_bytes += item_bytes;
            results.push(item);
        }
        let end = start + results.len();
        let next_offset = (end < self.insertion_order.len()).then_some(end);
        json!({"results":results,"next_offset":next_offset})
    }

    pub fn stats(&self) -> Value {
        json!({"artifacts":self.entries.len(),"utf8_bytes":self.bytes,
            "artifact_limit_bytes":ARTIFACT_BYTES,"store_limit_bytes":STORE_BYTES,
            "artifact_count_limit":ARTIFACT_COUNT})
    }

    pub fn select(&self, reference: &Value, max_bytes: usize) -> Result<String, String> {
        let reference = parse_reference(reference)?;
        let entry = self
            .entries
            .get(&reference.result)
            .ok_or("unknown result in this run")?;
        if reference.sha256 != entry.descriptor.sha256 {
            return Err("result SHA-256 does not match retained observation".into());
        }
        let text = &entry.text;
        let selector = &reference.select;
        match selector.kind {
            SelectionKind::Whole => bounded(text, max_bytes),
            SelectionKind::Bytes => {
                let start = index(selector.start)?;
                let end = index(selector.end)?;
                if start > end || end > text.len() {
                    return Err("byte range is outside retained result".into());
                }
                let slice = text
                    .get(start..end)
                    .ok_or("byte range splits a UTF-8 code point")?;
                bounded(slice, max_bytes)
            }
            SelectionKind::Lines => {
                let first = index(selector.start)?;
                let last = index(selector.end)?;
                if first == 0 || last < first {
                    return Err("line range must be one-based and inclusive".into());
                }
                let mut position = 0;
                let mut range = None;
                for (i, line) in text.split_inclusive('\n').enumerate() {
                    let number = i + 1;
                    if number == first {
                        range = Some(position..position);
                    }
                    position += line.len();
                    if number == last {
                        let mut range = range.ok_or("line range is outside retained result")?;
                        range.end = position;
                        return bounded(&text[range], max_bytes);
                    }
                }
                Err("line range is outside retained result".into())
            }
            SelectionKind::JsonPointer => {
                let root = JsonParser::parse(text)?;
                let selected = root.pointer(selector.pointer.as_deref().unwrap_or(""))?;
                match reference.rendering {
                    Rendering::Json => bounded(&text[selected.span.clone()], max_bytes),
                    Rendering::Text => match &selected.kind {
                        JsonKind::String(value) => bounded(value, max_bytes),
                        _ => Err("text rendering requires a JSON string value".into()),
                    },
                }
            }
        }
    }

    /// Resolve a literal string or a closed composition of literals and references.
    /// No evaluation, shell expansion, filesystem access, or source re-execution occurs.
    pub fn resolve_content(&self, value: &Value, max_bytes: usize) -> Result<String, String> {
        if let Some(literal) = value.as_str() {
            return bounded(literal, max_bytes);
        }
        let object = closed(value, &["segments"], &[])?;
        let segments = object["segments"]
            .as_array()
            .ok_or("segments must be an array")?;
        if segments.len() > COMPOSITION_SEGMENTS {
            return Err("composition exceeds 256 segments".into());
        }
        let mut output = String::new();
        for segment in segments {
            let object = segment.as_object().ok_or("segment must be an object")?;
            if object.len() != 1 {
                return Err("segment must contain exactly literal or ref".into());
            }
            let remaining = max_bytes.saturating_sub(output.len());
            if let Some(value) = object.get("literal") {
                let text = value.as_str().ok_or("literal must be a string")?;
                if text.len() > remaining {
                    return Err("composed content exceeds output byte budget".into());
                }
                output.push_str(text);
            } else if let Some(reference) = object.get("ref") {
                let selected = self.select(reference, remaining)?;
                output.push_str(&selected);
            } else {
                return Err("unknown composition segment".into());
            }
        }
        Ok(output)
    }
}

/// A host-selected preview is descriptive only; authoritative bytes remain in the store.
pub fn descriptor(stored: &StoredResult, preview: &str) -> Value {
    json!({"result":stored.result_id,"sha256":stored.sha256,"origin":stored.origin,
        "capture":match stored.capture {Capture::Complete=>"complete",Capture::Partial=>"partial"},
        "utf8_bytes":stored.utf8_bytes,"preview":preview})
}

pub fn reference_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["result","sha256","select","rendering"],
    "properties":{
        "result":{"type":"string","maxLength":256},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},
        "rendering":{"type":"string","enum":["text","json"]},
        "select":{"oneOf":[
            {"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"whole"}}},
            {"type":"object","additionalProperties":false,"required":["kind","start","end"],"properties":{"kind":{"const":"bytes"},"start":{"type":"integer","minimum":0},"end":{"type":"integer","minimum":0}}},
            {"type":"object","additionalProperties":false,"required":["kind","start","end"],"properties":{"kind":{"const":"lines"},"start":{"type":"integer","minimum":1},"end":{"type":"integer","minimum":1}}},
            {"type":"object","additionalProperties":false,"required":["kind","pointer"],"properties":{"kind":{"const":"json_pointer"},"pointer":{"type":"string","maxLength":4096}}}
        ]}
    }})
}

fn bounded(text: &str, max_bytes: usize) -> Result<String, String> {
    if text.len() > max_bytes {
        Err("selected content exceeds output byte budget".into())
    } else {
        Ok(text.to_owned())
    }
}
fn closed<'a>(
    value: &'a Value,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a Map<String, Value>, String> {
    let object = value.as_object().ok_or("expected an object")?;
    if required.iter().any(|key| !object.contains_key(*key)) {
        return Err("missing required object field".into());
    }
    if object
        .keys()
        .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err("unknown object field".into());
    }
    Ok(object)
}
fn index(value: Option<i64>) -> Result<usize, String> {
    value
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "range bounds must be nonnegative integers".into())
}
fn parse_reference(value: &Value) -> Result<ResultReference, String> {
    let object = closed(value, &["result", "sha256", "select", "rendering"], &[])?;
    let string = |key: &str| {
        let value = object[key]
            .as_str()
            .ok_or_else(|| format!("{key} must be a string"))?;
        let limit = match key {
            "result" => 256,
            "sha256" => 64,
            "rendering" => 4,
            _ => unreachable!(),
        };
        if value.len() > limit {
            return Err(format!("{key} exceeds its byte limit"));
        }
        Ok(value.to_owned())
    };
    let rendering = match string("rendering")?.as_str() {
        "text" => Rendering::Text,
        "json" => Rendering::Json,
        _ => return Err("unsupported rendering".into()),
    };
    let selector = object["select"]
        .as_object()
        .ok_or("select must be an object")?;
    let kind = match selector.get("kind").and_then(Value::as_str) {
        Some("whole") => SelectionKind::Whole,
        Some("bytes") => SelectionKind::Bytes,
        Some("lines") => SelectionKind::Lines,
        Some("json_pointer") => SelectionKind::JsonPointer,
        _ => return Err("unsupported selector kind".into()),
    };
    let mut select = ResultSelector {
        kind,
        start: None,
        end: None,
        pointer: None,
    };
    match kind {
        SelectionKind::Whole => {
            closed(&object["select"], &["kind"], &[])?;
        }
        SelectionKind::Bytes | SelectionKind::Lines => {
            closed(&object["select"], &["kind", "start", "end"], &[])?;
            select.start = Some(
                selector["start"]
                    .as_i64()
                    .ok_or("start must be an integer")?,
            );
            select.end = Some(selector["end"].as_i64().ok_or("end must be an integer")?);
            index(select.start)?;
            index(select.end)?;
        }
        SelectionKind::JsonPointer => {
            closed(&object["select"], &["kind", "pointer"], &[])?;
            let pointer = selector["pointer"]
                .as_str()
                .ok_or("pointer must be a string")?;
            if pointer.len() > 4096 {
                return Err("pointer exceeds 4096 bytes".into());
            }
            select.pointer = Some(pointer.to_owned());
        }
    }
    if kind != SelectionKind::JsonPointer && rendering != Rendering::Text {
        return Err("whole, byte, and line selectors require text rendering".into());
    }
    let sha256 = string("sha256")?;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("sha256 must contain 64 lowercase hexadecimal digits".into());
    }
    Ok(ResultReference {
        result: string("result")?,
        sha256,
        select,
        rendering,
    })
}

// A bounded original-span JSON reader. Values are never converted to machine
// numbers: exact JSON lexemes (including -0 and large exponents) survive selection.
#[derive(Debug)]
struct JsonValue {
    span: Range<usize>,
    kind: JsonKind,
}
#[derive(Debug)]
enum JsonKind {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Scalar,
}
impl JsonValue {
    fn pointer(&self, pointer: &str) -> Result<&Self, String> {
        if pointer.is_empty() {
            return Ok(self);
        }
        if !pointer.starts_with('/') {
            return Err("JSON pointer must be empty or start with slash".into());
        }
        let mut current = self;
        for raw in pointer[1..].split('/') {
            let mut token = String::new();
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    token.push(match chars.next() {
                        Some('0') => '~',
                        Some('1') => '/',
                        _ => return Err("invalid JSON pointer escape".into()),
                    });
                } else {
                    token.push(c);
                }
            }
            current = match &current.kind {
                JsonKind::Object(fields) => fields
                    .iter()
                    .find(|(key, _)| key == &token)
                    .map(|(_, value)| value)
                    .ok_or("JSON pointer object member does not exist")?,
                JsonKind::Array(values) => {
                    if token.is_empty()
                        || (token.len() > 1 && token.starts_with('0'))
                        || !token.bytes().all(|b| b.is_ascii_digit())
                    {
                        return Err(
                            "JSON pointer array index must be canonical nonnegative digits".into(),
                        );
                    }
                    let index = token
                        .parse::<usize>()
                        .map_err(|_| "JSON pointer array index is too large")?;
                    values
                        .get(index)
                        .ok_or("JSON pointer array index is outside array")?
                }
                _ => return Err("JSON pointer traverses a scalar value".into()),
            };
        }
        Ok(current)
    }
}
struct JsonParser<'a> {
    source: &'a str,
    pos: usize,
    nodes: usize,
}
impl<'a> JsonParser<'a> {
    fn parse(source: &'a str) -> Result<JsonValue, String> {
        let mut parser = Self {
            source,
            pos: 0,
            nodes: 0,
        };
        let value = parser.value(0)?;
        parser.whitespace();
        if parser.pos != source.len() {
            return Err("JSON has trailing content".into());
        }
        Ok(value)
    }
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }
    fn whitespace(&mut self) {
        while self
            .peek()
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.pos += 1;
        }
    }
    fn consume(&mut self, expected: u8) -> Result<(), String> {
        if self.peek() != Some(expected) {
            return Err("invalid JSON syntax".into());
        }
        self.pos += 1;
        Ok(())
    }
    fn value(&mut self, depth: usize) -> Result<JsonValue, String> {
        if depth > JSON_DEPTH || self.nodes >= JSON_NODES {
            return Err("JSON exceeds depth or node budget".into());
        }
        self.nodes += 1;
        self.whitespace();
        let start = self.pos;
        let kind = match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                self.whitespace();
                let mut fields = Vec::new();
                let mut keys = HashSet::new();
                if self.peek() != Some(b'}') {
                    loop {
                        self.whitespace();
                        let key = self.string()?;
                        if !keys.insert(key.clone()) {
                            return Err("duplicate JSON object key".into());
                        }
                        self.whitespace();
                        self.consume(b':')?;
                        let value = self.value(depth + 1)?;
                        fields.push((key, value));
                        self.whitespace();
                        if self.peek() != Some(b',') {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                self.consume(b'}')?;
                JsonKind::Object(fields)
            }
            Some(b'[') => {
                self.pos += 1;
                self.whitespace();
                let mut values = Vec::new();
                if self.peek() != Some(b']') {
                    loop {
                        values.push(self.value(depth + 1)?);
                        self.whitespace();
                        if self.peek() != Some(b',') {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                self.consume(b']')?;
                JsonKind::Array(values)
            }
            Some(b'"') => JsonKind::String(self.string()?),
            Some(b't') => {
                self.keyword("true")?;
                JsonKind::Scalar
            }
            Some(b'f') => {
                self.keyword("false")?;
                JsonKind::Scalar
            }
            Some(b'n') => {
                self.keyword("null")?;
                JsonKind::Scalar
            }
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                JsonKind::Scalar
            }
            _ => return Err("invalid JSON value".into()),
        };
        Ok(JsonValue {
            span: start..self.pos,
            kind,
        })
    }
    fn keyword(&mut self, word: &str) -> Result<(), String> {
        if !self.source[self.pos..].starts_with(word) {
            return Err("invalid JSON keyword".into());
        }
        self.pos += word.len();
        Ok(())
    }
    fn string(&mut self) -> Result<String, String> {
        let start = self.pos;
        self.consume(b'"')?;
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return serde_json::from_str(&self.source[start..self.pos])
                        .map_err(|_| "invalid JSON string".into());
                }
                Some(b'\\') => {
                    self.pos += 1;
                    if self.peek().is_none() {
                        return Err("unterminated JSON string".into());
                    }
                    self.pos += 1;
                }
                Some(_) => self.pos += 1,
                None => return Err("unterminated JSON string".into()),
            }
        }
    }
    fn number(&mut self) -> Result<(), String> {
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.pos += 1;
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return Err("invalid JSON number".into()),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.digits()?;
        }
        Ok(())
    }
    fn digits(&mut self) -> Result<(), String> {
        let start = self.pos;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        if start == self.pos {
            Err("invalid JSON number digits".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stored(text: &str) -> (ResultStore, StoredResult) {
        let mut store = ResultStore::new();
        let metadata = store
            .insert("test observation", text.into(), Capture::Complete)
            .unwrap();
        (store, metadata)
    }
    fn reference(stored: &StoredResult, select: Value, rendering: &str) -> Value {
        json!({"result":stored.result_id,"sha256":stored.sha256,"select":select,"rendering":rendering})
    }
    #[test]
    fn identities_are_immutable_and_isolated_even_for_identical_text() {
        let (mut a, first) = stored("same");
        let second = a
            .insert("second observation", "same".into(), Capture::Partial)
            .unwrap();
        let (b, foreign) = stored("same");
        assert_ne!(first.result_id, second.result_id);
        assert_ne!(first.result_id, foreign.result_id);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(second.capture, Capture::Partial);
        let mut r = reference(&first, json!({"kind":"whole"}), "text");
        assert_eq!(a.select(&r, 4).unwrap(), "same");
        assert!(b.select(&r, 100).is_err());
        r["sha256"] = json!("0".repeat(64));
        assert!(a.select(&r, 100).is_err());
        assert_eq!(a.stats()["artifacts"], 2);
    }
    #[test]
    fn bytes_are_half_open_and_unicode_safe() {
        let (s, m) = stored("aé🦀z");
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"bytes","start":1,"end":7}), "text"),
                6
            )
            .unwrap(),
            "é🦀"
        );
        for (start, end) in [(2, 3), (3, 4), (8, 9), (7, 6)] {
            assert!(
                s.select(
                    &reference(&m, json!({"kind":"bytes","start":start,"end":end}), "text"),
                    100
                )
                .is_err()
            );
        }
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"bytes","start":8,"end":8}), "text"),
                0
            )
            .unwrap(),
            ""
        );
    }
    #[test]
    fn line_selection_preserves_endings_and_has_no_phantom_last_line() {
        let (s, m) = stored("α\r\n\nlast\r\n");
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"lines","start":1,"end":2}), "text"),
                100
            )
            .unwrap(),
            "α\r\n\n"
        );
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"lines","start":3,"end":3}), "text"),
                100
            )
            .unwrap(),
            "last\r\n"
        );
        for (a, b) in [(0, 1), (4, 4), (2, 1)] {
            assert!(
                s.select(
                    &reference(&m, json!({"kind":"lines","start":a,"end":b}), "text"),
                    100
                )
                .is_err()
            );
        }
        let (s, m) = stored("");
        assert!(
            s.select(
                &reference(&m, json!({"kind":"lines","start":1,"end":1}), "text"),
                100
            )
            .is_err()
        );
        let (s, m) = stored("last");
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"lines","start":1,"end":1}), "text"),
                100
            )
            .unwrap(),
            "last"
        );
    }
    #[test]
    fn json_selection_preserves_numbers_whitespace_and_decodes_only_string_text() {
        let text = r#" { "big":184467440737095516160000, "e":1.2300e+400, "negative":-0, "obj":{ "x" : [true, null] }, "s":"\u00e9\n" } "#;
        let (s, m) = stored(text);
        for (pointer, want) in [
            ("/big", "184467440737095516160000"),
            ("/e", "1.2300e+400"),
            ("/negative", "-0"),
            ("/obj", r#"{ "x" : [true, null] }"#),
        ] {
            assert_eq!(
                s.select(
                    &reference(&m, json!({"kind":"json_pointer","pointer":pointer}), "json"),
                    1000
                )
                .unwrap(),
                want
            );
        }
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"json_pointer","pointer":"/s"}), "text"),
                100
            )
            .unwrap(),
            "é\n"
        );
        assert!(
            s.select(
                &reference(&m, json!({"kind":"json_pointer","pointer":"/big"}), "text"),
                100
            )
            .is_err()
        );
        assert_eq!(
            s.select(
                &reference(&m, json!({"kind":"json_pointer","pointer":""}), "json"),
                1000
            )
            .unwrap(),
            text.trim()
        );
    }
    #[test]
    fn pointer_escapes_and_indices_are_strict() {
        let (s, m) = stored(r#"{"a/b":{"~x":["value"]},"":"empty","01":"object key"}"#);
        for (pointer, want) in [
            ("/a~1b/~0x/0", "value"),
            ("/", "empty"),
            ("/01", "object key"),
        ] {
            assert_eq!(
                s.select(
                    &reference(&m, json!({"kind":"json_pointer","pointer":pointer}), "text"),
                    100
                )
                .unwrap(),
                want
            );
        }
        for pointer in [
            "a",
            "#/a",
            "/a~2b",
            "/a~",
            "/a~1b/~0x/-",
            "/a~1b/~0x/00",
            "/a~1b/~0x/+0",
            "/a~1b/~0x/1",
            "/missing",
        ] {
            assert!(
                s.select(
                    &reference(&m, json!({"kind":"json_pointer","pointer":pointer}), "text"),
                    100
                )
                .is_err(),
                "{pointer}"
            );
        }
    }
    #[test]
    fn duplicate_keys_and_invalid_json_anywhere_refuse_even_unselected_branches() {
        for text in [
            r#"{"ok":1,"bad":{"x":0,"\u0078":1}}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"ok":1,"bad":[0,]}"#,
            r#"{"ok":1,"bad":01}"#,
            r#"{"ok":1,"bad":"\uD800"}"#,
            r#"{"ok":1} trailing"#,
            r#"{"ok":1,"bad":1e}"#,
        ] {
            let (s, m) = stored(text);
            assert!(
                s.select(
                    &reference(&m, json!({"kind":"json_pointer","pointer":"/ok"}), "json"),
                    100
                )
                .is_err(),
                "{text}"
            );
        }
    }
    #[test]
    fn schema_is_closed_and_selector_fields_cannot_be_ignored() {
        let (s, m) = stored("text");
        let valid = reference(&m, json!({"kind":"whole"}), "text");
        for bad in [
            json!({"result":m.result_id,"select":{"kind":"whole"},"rendering":"text"}),
            reference(&m, json!({"kind":"whole","start":0}), "text"),
            reference(&m, json!({"kind":"bytes","start":-1,"end":2}), "text"),
            reference(&m, json!({"kind":"bytes","start":0.5,"end":2}), "text"),
            reference(
                &m,
                json!({"kind":"bytes","start":0,"end":2,"pointer":""}),
                "text",
            ),
            reference(&m, json!({"kind":"whole"}), "json"),
        ] {
            assert!(s.select(&bad, 100).is_err());
        }
        let mut extra = valid.clone();
        extra["scope"] = json!("other");
        assert!(s.select(&extra, 100).is_err());
        assert!(s.select(&valid, 3).is_err());
        assert_eq!(reference_schema()["additionalProperties"], false);
    }
    #[test]
    fn metadata_pages_cover_all_results_in_stable_order_without_payloads() {
        let mut store = ResultStore::new();
        assert_eq!(
            store.list_results(0, 8),
            json!({"results":[],"next_offset":null})
        );
        let mut ids = Vec::new();
        for i in 0..20 {
            ids.push(
                store
                    .insert_text(&format!("file-{i}"), "private payload", Capture::Partial)
                    .unwrap()
                    .result_id,
            );
        }
        let mut offset = 0;
        let mut actual = Vec::new();
        loop {
            let page = store.list_results(offset, usize::MAX);
            let results = page["results"].as_array().unwrap();
            assert!(results.len() <= 8);
            for item in results {
                assert_eq!(item["origin"], format!("file-{}", actual.len()));
                assert_eq!(item["capture"], "partial");
                assert_eq!(item["utf8_bytes"], 15);
                assert!(item.get("preview").is_none());
                assert_eq!(
                    store.select(&item["reference"], 15).unwrap(),
                    "private payload"
                );
                actual.push(item["reference"]["result"].as_str().unwrap().to_owned());
            }
            match page["next_offset"].as_u64() {
                Some(next) => {
                    assert!(next as usize > offset);
                    offset = next as usize;
                }
                None => break,
            }
        }
        assert_eq!(actual, ids);
        assert_eq!(store.list_results(19, 8)["results"][0]["origin"], "file-19");
        assert_eq!(store.list_results(0, 0)["next_offset"], 1);
        assert_eq!(
            store.list_results(usize::MAX, 8),
            json!({"results":[],"next_offset":null})
        );
    }
    #[test]
    fn metadata_pages_bound_escaped_origins_without_losing_references() {
        let mut store = ResultStore::new();
        for _ in 0..20 {
            store
                .insert_text(&"\u{0}".repeat(4096), "", Capture::Complete)
                .unwrap();
        }
        let page = store.list_results(0, 8);
        assert!(page.to_string().len() <= 8192);
        let results = page["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert!(results.len() < 8);
        assert_eq!(page["next_offset"], results.len());
        for item in results {
            assert_eq!(item["origin"].as_str().unwrap().len(), 256);
            assert_eq!(item["origin_truncated"], true);
            assert_eq!(store.select(&item["reference"], 0).unwrap(), "");
        }
        store
            .insert_text(&"é".repeat(200), "", Capture::Complete)
            .unwrap();
        let page = store.list_results(20, 1);
        assert_eq!(page["results"][0]["origin"], "é".repeat(128));
    }
    #[test]
    fn borrowed_admission_rejects_limits_without_changing_inventory() {
        let mut store = ResultStore::new();
        let oversized = "x".repeat(ARTIFACT_BYTES + 1);
        assert!(
            store
                .insert_text("large", &oversized, Capture::Complete)
                .is_err()
        );
        assert!(
            store
                .insert_text(&"x".repeat(ORIGIN_BYTES + 1), "", Capture::Complete)
                .is_err()
        );
        assert_eq!(store.stats()["artifacts"], 0);
        let text = "x".repeat(ARTIFACT_BYTES);
        for _ in 0..4 {
            store.insert_text("full", &text, Capture::Complete).unwrap();
        }
        let before = store.list_results(0, 8);
        assert!(
            store
                .insert_text("overflow", "x", Capture::Complete)
                .is_err()
        );
        assert_eq!(store.list_results(0, 8), before);
        assert_eq!(store.stats()["utf8_bytes"], STORE_BYTES);
        let mut store = ResultStore::new();
        for _ in 0..ARTIFACT_COUNT {
            store.insert_text("empty", "", Capture::Complete).unwrap();
        }
        assert!(
            store
                .insert_text("too many", "", Capture::Complete)
                .is_err()
        );
        assert_eq!(store.stats()["artifacts"], ARTIFACT_COUNT);
    }
    #[test]
    fn reference_metadata_is_bounded_before_copying() {
        let (_, m) = stored("null");
        let valid = reference(&m, json!({"kind":"whole"}), "text");
        for (key, length) in [("result", 257), ("sha256", 65), ("rendering", 5)] {
            let mut oversized = valid.clone();
            oversized[key] = json!("a".repeat(length));
            assert!(
                parse_reference(&oversized)
                    .unwrap_err()
                    .contains("byte limit")
            );
        }
        let oversized = reference(
            &m,
            json!({"kind":"json_pointer","pointer":"/".repeat(4097)}),
            "json",
        );
        assert!(parse_reference(&oversized).unwrap_err().contains("4096"));
        assert_eq!(
            reference_schema()["properties"]["select"]["oneOf"][2]["properties"]["kind"]["const"],
            "lines"
        );
    }
    #[test]
    fn composition_resolves_without_evaluation_and_checks_joint_byte_budget() {
        let (s, m) = stored("é");
        let r = reference(&m, json!({"kind":"whole"}), "text");
        let composed = json!({"segments":[{"literal":"$(never-run)"},{"ref":r}]});
        assert_eq!(s.resolve_content(&composed, 14).unwrap(), "$(never-run)é");
        assert!(s.resolve_content(&composed, 12).is_err());
        for bad in [
            json!({"segments":[{"literal":"x","ref":r}]}),
            json!({"segments":[{"literal":2}]}),
            json!({"segments":[],"eval":true}),
            json!({"segments":[{"expression":"1+1"}]}),
        ] {
            assert!(s.resolve_content(&bad, 100).is_err());
        }
        assert!(
            s.resolve_content(&json!({"segments":vec![json!({"literal":""});257]}), 100)
                .is_err()
        );
        assert_eq!(s.resolve_content(&json!("literal"), 7).unwrap(), "literal");
    }
    #[test]
    fn storage_limits_refuse_without_eviction_or_partial_mutation() {
        let mut s = ResultStore::new();
        let first = s.insert("first", String::new(), Capture::Complete).unwrap();
        for _ in 1..ARTIFACT_COUNT {
            s.insert("count", String::new(), Capture::Complete).unwrap();
        }
        assert!(
            s.insert("overflow", String::new(), Capture::Complete)
                .is_err()
        );
        assert_eq!(
            s.select(&reference(&first, json!({"kind":"whole"}), "text"), 0)
                .unwrap(),
            ""
        );
        let mut s = ResultStore::new();
        assert!(
            s.insert(
                "too large",
                "x".repeat(ARTIFACT_BYTES + 1),
                Capture::Complete
            )
            .is_err()
        );
        for _ in 0..4 {
            s.insert("limit", "x".repeat(ARTIFACT_BYTES), Capture::Complete)
                .unwrap();
        }
        assert!(s.insert("overflow", "x".into(), Capture::Complete).is_err());
        assert_eq!(s.stats()["utf8_bytes"], STORE_BYTES);
    }
    #[test]
    fn json_depth_and_node_counts_are_bounded() {
        let text = format!(
            "{}0{}",
            "[".repeat(JSON_DEPTH + 1),
            "]".repeat(JSON_DEPTH + 1)
        );
        assert!(JsonParser::parse(&text).is_err());
        let text = format!("[{}]", vec!["0"; JSON_NODES].join(","));
        assert!(JsonParser::parse(&text).is_err());
    }
}
