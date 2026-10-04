// generated from commission v1
// model digest 8baad8a2a232f1823d8fce586ddd35c901af8923715a1eb1fbc3f62817da8e4f
// contract digest c27daebab1de8a70c1c4985e2a48176db7d5700784cedf99b11902b83a5dea7a
// do not edit: regenerate with `ess synthesize --layout crate`

//! JSON at this system's boundary: a reader, a writer, and the base64 codec `Bytes` needs.
//!
//! Written here rather than taken from a crate because this workspace has no dependencies: it is
//! built inside a gate that reaches no network. The surface is exactly what the generated `wire`
//! module beside it uses — nothing general, nothing speculative.

use std::fmt;

/// How deep a document from a caller may nest before it is refused.
///
/// A caller can send anything, and a recursive reader with no limit turns a hostile document
/// into a stack overflow — which is a crash the process cannot catch and cannot report. Sixty-four is far past any model this emitter can produce: the specification's own
/// type references are refused past thirty-two.
const DEPTH: usize = 64;

/// A JSON value, as it arrived.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, kept in the spelling it arrived in rather than parsed into a float.
    ///
    /// A float would round a decimal the model deliberately carries as text, and this reader has
    /// no business deciding that `10.50` is `10.5`.
    Number(String),
    /// A string, with its escapes resolved.
    Text(String),
    /// An array, in order.
    Array(Vec<Value>),
    /// An object, in the order its members arrived.
    ///
    /// A list of pairs rather than a map: order is what makes a rendering reproducible, and a
    /// duplicate key is a fact about the document rather than something to silently collapse.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member with this name, or `None` — including when this is not an object at all.
    pub fn member(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// What this value is, in one word, for a message about what was expected instead.
    pub fn describes(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Number(_) => "a number",
            Self::Text(_) => "a string",
            Self::Array(_) => "an array",
            Self::Object(_) => "an object",
        }
    }
}

/// A document the reader refused, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The byte offset it stopped at.
    pub at: usize,
    /// What would have been legal there.
    pub expected: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at byte {}: expected {}", self.at, self.expected)
    }
}

/// Reads one JSON document.
///
/// # Errors
///
/// [`ParseError`] naming the offset and what was expected there, for anything that is not exactly
/// one well-formed value followed by whitespace.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let bytes = text.as_bytes();
    let mut at = 0;
    skip(bytes, &mut at);
    let value = read(bytes, &mut at, 0)?;
    skip(bytes, &mut at);
    if at != bytes.len() {
        return Err(ParseError {
            at,
            expected: "the end of the document".to_owned(),
        });
    }
    Ok(value)
}

/// Advances past whitespace.
fn skip(bytes: &[u8], at: &mut usize) {
    while let Some(byte) = bytes.get(*at) {
        if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
            *at += 1;
        } else {
            break;
        }
    }
}

/// Reads one value at `at`, which is already past any leading whitespace.
fn read(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Value, ParseError> {
    if depth > DEPTH {
        return Err(ParseError {
            at: *at,
            expected: "a document nested less than 64 deep".to_owned(),
        });
    }
    match bytes.get(*at) {
        Some(b'{') => object(bytes, at, depth),
        Some(b'[') => array(bytes, at, depth),
        Some(b'"') => text(bytes, at).map(Value::Text),
        Some(b't') => literal(bytes, at, b"true", Value::Bool(true)),
        Some(b'f') => literal(bytes, at, b"false", Value::Bool(false)),
        Some(b'n') => literal(bytes, at, b"null", Value::Null),
        Some(byte) if *byte == b'-' || byte.is_ascii_digit() => number(bytes, at),
        _ => Err(ParseError {
            at: *at,
            expected: "a value".to_owned(),
        }),
    }
}

/// Reads one of the three bare words.
fn literal(bytes: &[u8], at: &mut usize, word: &[u8], value: Value) -> Result<Value, ParseError> {
    if bytes.len() < *at + word.len() || &bytes[*at..*at + word.len()] != word {
        return Err(ParseError {
            at: *at,
            expected: format!("`{}`", String::from_utf8_lossy(word)),
        });
    }
    *at += word.len();
    Ok(value)
}

/// Reads a number, keeping its spelling.
fn number(bytes: &[u8], at: &mut usize) -> Result<Value, ParseError> {
    let start = *at;
    if bytes.get(*at) == Some(&b'-') {
        *at += 1;
    }
    let digits = *at;
    while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
        *at += 1;
    }
    if *at == digits {
        return Err(ParseError {
            at: *at,
            expected: "a digit".to_owned(),
        });
    }
    if bytes.get(*at) == Some(&b'.') {
        *at += 1;
        let fraction = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        if *at == fraction {
            return Err(ParseError {
                at: *at,
                expected: "a digit after the decimal point".to_owned(),
            });
        }
    }
    if matches!(bytes.get(*at), Some(b'e' | b'E')) {
        *at += 1;
        if matches!(bytes.get(*at), Some(b'+' | b'-')) {
            *at += 1;
        }
        let exponent = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        if *at == exponent {
            return Err(ParseError {
                at: *at,
                expected: "a digit in the exponent".to_owned(),
            });
        }
    }
    Ok(Value::Number(
        String::from_utf8_lossy(&bytes[start..*at]).into_owned(),
    ))
}

/// Reads a string, resolving its escapes.
fn text(bytes: &[u8], at: &mut usize) -> Result<String, ParseError> {
    *at += 1;
    let mut out = String::new();
    loop {
        let Some(byte) = bytes.get(*at).copied() else {
            return Err(ParseError {
                at: *at,
                expected: "a closing quote".to_owned(),
            });
        };
        match byte {
            b'"' => {
                *at += 1;
                return Ok(out);
            }
            b'\\' => {
                *at += 1;
                let escaped = escape(bytes, at)?;
                out.push(escaped);
            }
            _ => {
                let start = *at;
                while bytes
                    .get(*at)
                    .is_some_and(|byte| *byte != b'"' && *byte != b'\\')
                {
                    *at += 1;
                }
                out.push_str(&String::from_utf8_lossy(&bytes[start..*at]));
            }
        }
    }
}

/// Reads one escape sequence, the backslash already consumed.
fn escape(bytes: &[u8], at: &mut usize) -> Result<char, ParseError> {
    let Some(byte) = bytes.get(*at).copied() else {
        return Err(ParseError {
            at: *at,
            expected: "an escape".to_owned(),
        });
    };
    *at += 1;
    Ok(match byte {
        b'"' => '"',
        b'\\' => '\\',
        b'/' => '/',
        b'b' => '\u{8}',
        b'f' => '\u{c}',
        b'n' => '\n',
        b'r' => '\r',
        b't' => '\t',
        b'u' => return unicode(bytes, at),
        _ => {
            return Err(ParseError {
                at: *at,
                expected: "one of \" \\ / b f n r t u".to_owned(),
            })
        }
    })
}

/// Reads a `\u` escape, joining a surrogate pair when it finds one.
fn unicode(bytes: &[u8], at: &mut usize) -> Result<char, ParseError> {
    let first = hex(bytes, at)?;
    if (0xD800..0xDC00).contains(&first) {
        // A lone high surrogate is not a character; the low half has to follow, escaped the same
        // way. Refusing here rather than substituting a replacement character keeps a truncated
        // document a refusal instead of a value nobody sent.
        if bytes.get(*at) != Some(&b'\\') || bytes.get(*at + 1) != Some(&b'u') {
            return Err(ParseError {
                at: *at,
                expected: "the low half of a surrogate pair".to_owned(),
            });
        }
        *at += 2;
        let second = hex(bytes, at)?;
        if !(0xDC00..0xE000).contains(&second) {
            return Err(ParseError {
                at: *at,
                expected: "the low half of a surrogate pair".to_owned(),
            });
        }
        let joined = 0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00);
        return char::from_u32(joined).ok_or(ParseError {
            at: *at,
            expected: "a character".to_owned(),
        });
    }
    char::from_u32(first).ok_or(ParseError {
        at: *at,
        expected: "a character".to_owned(),
    })
}

/// Reads exactly four hexadecimal digits.
fn hex(bytes: &[u8], at: &mut usize) -> Result<u32, ParseError> {
    let mut value = 0;
    for _ in 0..4 {
        let Some(byte) = bytes.get(*at).copied() else {
            return Err(ParseError {
                at: *at,
                expected: "four hexadecimal digits".to_owned(),
            });
        };
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a') + 10,
            b'A'..=b'F' => u32::from(byte - b'A') + 10,
            _ => {
                return Err(ParseError {
                    at: *at,
                    expected: "four hexadecimal digits".to_owned(),
                })
            }
        };
        value = value * 16 + digit;
        *at += 1;
    }
    Ok(value)
}

/// Reads an array.
fn array(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Value, ParseError> {
    *at += 1;
    let mut items = Vec::new();
    skip(bytes, at);
    if bytes.get(*at) == Some(&b']') {
        *at += 1;
        return Ok(Value::Array(items));
    }
    loop {
        skip(bytes, at);
        items.push(read(bytes, at, depth + 1)?);
        skip(bytes, at);
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b']') => {
                *at += 1;
                return Ok(Value::Array(items));
            }
            _ => {
                return Err(ParseError {
                    at: *at,
                    expected: "`,` or `]`".to_owned(),
                })
            }
        }
    }
}

/// Reads an object.
fn object(bytes: &[u8], at: &mut usize, depth: usize) -> Result<Value, ParseError> {
    *at += 1;
    let mut members = Vec::new();
    skip(bytes, at);
    if bytes.get(*at) == Some(&b'}') {
        *at += 1;
        return Ok(Value::Object(members));
    }
    loop {
        skip(bytes, at);
        if bytes.get(*at) != Some(&b'"') {
            return Err(ParseError {
                at: *at,
                expected: "a member name".to_owned(),
            });
        }
        let name = text(bytes, at)?;
        skip(bytes, at);
        if bytes.get(*at) != Some(&b':') {
            return Err(ParseError {
                at: *at,
                expected: "`:`".to_owned(),
            });
        }
        *at += 1;
        skip(bytes, at);
        members.push((name, read(bytes, at, depth + 1)?));
        skip(bytes, at);
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;
                return Ok(Value::Object(members));
            }
            _ => {
                return Err(ParseError {
                    at: *at,
                    expected: "`,` or `}`".to_owned(),
                })
            }
        }
    }
}

// ---- writing -----------------------------------------------------------------------------------

/// Appends a JSON string, escaped.
pub fn push_text(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Every other control character has to be escaped for the document to be legal, and
            // `\u` is the only form JSON has for one.
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

/// Appends a JSON number.
pub fn push_integer(out: &mut String, value: i64) {
    out.push_str(&value.to_string());
}

/// Appends `true` or `false`.
pub fn push_bool(out: &mut String, value: bool) {
    out.push_str(if value { "true" } else { "false" });
}

/// Appends opaque bytes as the base64 string the published wire contracts fix.
pub fn push_base64(out: &mut String, bytes: &[u8]) {
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = chunk.get(1).copied().map_or(0, u32::from);
        let c = chunk.get(2).copied().map_or(0, u32::from);
        let packed = (a << 16) | (b << 8) | c;
        encoded.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        encoded.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[(packed >> 6) as usize & 63] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[packed as usize & 63] as char
        } else {
            '='
        });
    }
    push_text(out, &encoded);
}

/// The standard base64 alphabet, padded — the encoding the JSON Schema projection's pattern checks.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

// ---- reading into the model's own shapes ---------------------------------------------------------

/// A value that was not what the model's declaration says it is.
///
/// Carries the path it was reached at, because a caller sending a nested command input gets one
/// message and "expected a string" without a path is a message nobody can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    /// Where in the document, as a dotted path from its root.
    pub at: String,
    /// What the declaration says belongs there.
    pub expected: String,
    /// What was there instead.
    pub found: String,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: expected {}, found {}", self.at, self.expected, self.found)
    }
}

impl DecodeError {
    /// A refusal at one path, with what was expected and what arrived.
    pub fn of(at: &str, expected: &str, found: &Value) -> Self {
        Self {
            at: at.to_owned(),
            expected: expected.to_owned(),
            found: found.describes().to_owned(),
        }
    }
}

/// One step further into a document, for a message a reader can follow back.
pub fn nested(at: &str, step: &str) -> String {
    if at.is_empty() {
        step.to_owned()
    } else {
        format!("{at}.{step}")
    }
}

/// The string at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is anything else.
pub fn text_at<'a>(value: &'a Value, at: &str, expected: &str) -> Result<&'a str, DecodeError> {
    match value {
        Value::Text(text) => Ok(text),
        other => Err(DecodeError::of(at, expected, other)),
    }
}

/// The integer at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is not a number, or is one no `Integer` can hold — a fraction,
/// an exponent, or a magnitude past 64 bits. Refused rather than rounded: a silently truncated
/// identifier is worse than a rejected command.
pub fn integer_at(value: &Value, at: &str, expected: &str) -> Result<i64, DecodeError> {
    match value {
        Value::Number(number) => number.parse::<i64>().map_err(|_| DecodeError {
            at: at.to_owned(),
            expected: expected.to_owned(),
            found: format!("the number {number}"),
        }),
        other => Err(DecodeError::of(at, expected, other)),
    }
}

/// The decimal string at this path, in the published pattern: an optional `-`, digits without a
/// leading zero, then an optional `.` and digits.
///
/// # Errors
///
/// [`DecodeError`] when the value is not a string, or is one the pattern refuses — refused as the
/// contract refuses it, rather than handed on as a decimal nobody can read.
pub fn decimal_at<'a>(value: &'a Value, at: &str, expected: &str) -> Result<&'a str, DecodeError> {
    let text = text_at(value, at, expected)?;
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (unsigned, None),
    };
    if digits(whole) && !(whole.len() > 1 && whole.starts_with('0')) && fraction.map_or(true, digits) {
        Ok(text)
    } else {
        Err(DecodeError { at: at.to_owned(), expected: expected.to_owned(), found: format!("`{text}`") })
    }
}

/// The UUID at this path, in the published pattern: the canonical hyphenated form, in either case.
///
/// # Errors
///
/// [`DecodeError`] when the value is not a string, or is one the pattern refuses.
pub fn uuid_at<'a>(value: &'a Value, at: &str, expected: &str) -> Result<&'a str, DecodeError> {
    let text = text_at(value, at, expected)?;
    let valid = text.len() == 36
        && text.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    if valid {
        Ok(text)
    } else {
        Err(DecodeError { at: at.to_owned(), expected: expected.to_owned(), found: format!("`{text}`") })
    }
}

/// The boolean at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is anything else.
pub fn bool_at(value: &Value, at: &str, expected: &str) -> Result<bool, DecodeError> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        other => Err(DecodeError::of(at, expected, other)),
    }
}

/// The bytes at this path, read from base64.
///
/// # Errors
///
/// [`DecodeError`] when the value is not a string, or not base64.
pub fn bytes_at(value: &Value, at: &str, expected: &str) -> Result<Vec<u8>, DecodeError> {
    let text = text_at(value, at, expected)?;
    base64(text, at, expected)
}

/// Base64 text as bytes.
///
/// # Errors
///
/// [`DecodeError`] when the text is not in the published pattern: whole groups of four characters
/// of the alphabet, the last of them padded with one or two `=` at most — nothing unpadded,
/// nothing after padding, no whitespace.
fn base64(text: &str, at: &str, expected: &str) -> Result<Vec<u8>, DecodeError> {
    if !base64_text(text) {
        return Err(DecodeError {
            at: at.to_owned(),
            expected: expected.to_owned(),
            found: "a string that is not base64".to_owned(),
        });
    }
    let mut packed = 0_u32;
    let mut held = 0_u32;
    let mut out = Vec::new();
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let Some(index) = ALPHABET.iter().position(|candidate| *candidate == byte) else {
            return Err(DecodeError {
                at: at.to_owned(),
                expected: expected.to_owned(),
                found: "a string that is not base64".to_owned(),
            });
        };
        packed = (packed << 6) | index as u32;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(((packed >> held) & 0xFF) as u8);
        }
    }
    Ok(out)
}

/// `true` where `text` is in the published base64 pattern,
/// `^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$`.
fn base64_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return false;
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    padding <= 2
        && bytes[..bytes.len() - padding]
            .iter()
            .all(|byte| ALPHABET.contains(byte))
}

/// The integer a map key spells.
///
/// JSON has only string keys, and the published wire contract constrains a non-string map's
/// `propertyNames` to text of the declared primitive's shape. This is that constraint, applied
/// where the value is actually built.
///
/// # Errors
///
/// [`DecodeError`] when the key is not an integer's spelling in the published pattern,
/// `^-?(0|[1-9][0-9]*)$` — no sign but `-`, no leading zero — or is one past 64 bits.
pub fn key_integer(key: &str, at: &str) -> Result<i64, DecodeError> {
    let digits = key.strip_prefix('-').unwrap_or(key);
    let spelled = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    spelled
        .then(|| key.parse::<i64>().ok())
        .flatten()
        .ok_or_else(|| DecodeError {
            at: at.to_owned(),
            expected: "a key spelling an integer".to_owned(),
            found: format!("the key `{key}`"),
        })
}

/// The boolean a map key spells.
///
/// # Errors
///
/// [`DecodeError`] when the key is neither `true` nor `false`.
pub fn key_bool(key: &str, at: &str) -> Result<bool, DecodeError> {
    match key {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DecodeError {
            at: at.to_owned(),
            expected: "a key spelling `true` or `false`".to_owned(),
            found: format!("the key `{other}`"),
        }),
    }
}

/// The bytes a base64 map key spells.
///
/// # Errors
///
/// [`DecodeError`] when the key is not base64.
pub fn key_bytes(key: &str, at: &str) -> Result<Vec<u8>, DecodeError> {
    base64(key, at, "a base64 key")
}

/// Opens a member of the object currently being written, with the separator it needs.
///
/// The separator is decided from the buffer rather than from a flag every generated writer would
/// have to thread through every branch: an absent optional field is *omitted*, exactly as the
/// published contract's `required` list says, and a comma written before nothing is a document no
/// reader accepts. A complete JSON value never ends with `{`, so the buffer ending with one means
/// this is the first member and nothing else.
pub fn member(out: &mut String, name: &str) {
    if !out.ends_with('{') {
        out.push(',');
    }
    push_text(out, name);
    out.push(':');
}

/// The array at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is anything else.
pub fn items_at<'a>(value: &'a Value, at: &str, expected: &str) -> Result<&'a [Value], DecodeError> {
    match value {
        Value::Array(items) => Ok(items),
        other => Err(DecodeError::of(at, expected, other)),
    }
}

/// The object members at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is anything else.
pub fn members_at<'a>(
    value: &'a Value,
    at: &str,
    expected: &str,
) -> Result<&'a [(String, Value)], DecodeError> {
    match value {
        Value::Object(members) => Ok(members),
        other => Err(DecodeError::of(at, expected, other)),
    }
}

/// The named member of the object at this path.
///
/// # Errors
///
/// [`DecodeError`] when the value is not an object, or the member is absent — absence of a
/// required field is a refusal, never a default.
pub fn member_at<'a>(value: &'a Value, at: &str, name: &str) -> Result<&'a Value, DecodeError> {
    let members = members_at(value, at, "an object")?;
    members
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, member)| member)
        .ok_or_else(|| DecodeError {
            at: nested(at, name),
            expected: "a value".to_owned(),
            found: "nothing".to_owned(),
        })
}

// ---- a `Json` value, as the model carries it -----------------------------------------------------

/// Equality is total: a `Value` holds no float — a number is its spelling — so every generated type
/// holding one keeps deriving `Eq`. It follows that `1.0` and `1` are different values here, as
/// `1.5` and `1.50` are different `Decimal`s.
impl Eq for Value {}

/// The `Json` value at this path, as it arrived.
///
/// Every JSON value is a `Json` value, so this never refuses. It takes the path anyway so a
/// generated decoder reads a `Json` leaf the way it reads every other leaf, and binds no position
/// it then leaves unused.
pub fn value_at(value: &Value, _at: &str) -> Value {
    value.clone()
}

/// Appends a JSON value exactly as it was read: members in their order, numbers in their spelling,
/// strings escaped the way [`push_text`] escapes them.
///
/// A `Number` is written as it is spelled; the reader only ever produces legal spellings, and a
/// caller building one by hand owns its spelling.
pub fn push_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(value) => push_bool(out, *value),
        Value::Number(spelling) => out.push_str(spelling),
        Value::Text(text) => push_text(out, text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_value(out, item);
            }
            out.push(']');
        }
        Value::Object(members) => {
            out.push('{');
            for (index, (name, item)) in members.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_text(out, name);
                out.push(':');
                push_value(out, item);
            }
            out.push('}');
        }
    }
}
