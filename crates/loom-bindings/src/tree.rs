//! The text of a document as a tree, read from YAML under the bounds a bindings document needs.
//!
//! The parser (`serde_yaml_ng`) reads a whole document before it hands a value back, and expands
//! an alias each time it is used. Structure is bounded on the values as they are produced: nesting
//! ([`MAX_NESTING`]), the node count and the bytes of the scalars, which aliases cannot get past.
//! The text is checked before the parse for one thing only ([`bounded`]): flow openers net of
//! closers past [`MAX_OPEN_BRACKETS`], a guard against a parser cost that counts brackets in
//! comments and strings too. A mapping key is read as the text the operator wrote, so `0` and
//! `"0"` are one key and `1.50` is not `1.5`.

use crate::{MAX_DOCUMENT_BYTES, MAX_NESTING, MAX_NODES, MAX_OPEN_BRACKETS};
use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, IgnoredAny, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;

/// One value of the document.
#[derive(Debug, Clone)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    Int(i64),
    /// A whole number above `i64::MAX`, as its digits.
    Big(String),
    Real(f64),
    String(String),
    Sequence(Vec<Node>),
    Mapping(Map),
}

impl Node {
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn as_mapping(&self) -> Option<&Map> {
        match self {
            Self::Mapping(map) => Some(map),
            _ => None,
        }
    }
}

/// A mapping: its members in the order written, each key once.
#[derive(Debug, Clone, Default)]
pub(crate) struct Map(Vec<(String, Node)>);

impl Map {
    pub(crate) fn get(&self, key: &str) -> Option<&Node> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, node)| node)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &Node)> {
        self.0.iter().map(|(key, node)| (key, node))
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(key, _)| key)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Reads `text` without a leading byte order mark: refused when its nesting is past
/// [`MAX_NESTING`], when it holds a tag, a `<<` merge key or a key twice, or when it expands to
/// more than [`MAX_NODES`] values or [`MAX_DOCUMENT_BYTES`] bytes of scalars.
pub(crate) fn parse(text: &str) -> Result<Node, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    bounded(text)?;
    let state = RefCell::new(State::default());
    let deserializer = serde_yaml_ng::Deserializer::from_str(text);
    Seed { state: &state }
        .deserialize(deserializer)
        .map_err(|e| e.to_string())
}

/// Refuses text that holds more than [`MAX_OPEN_BRACKETS`] flow openers net of closers, before any
/// parse.
fn bounded(text: &str) -> Result<(), String> {
    let mut open = 0usize;
    for c in text.chars() {
        match c {
            '[' | '{' => {
                open += 1;
                if open > MAX_OPEN_BRACKETS {
                    return Err(format!(
                        "the text holds more than {MAX_OPEN_BRACKETS} open brackets or braces"
                    ));
                }
            }
            ']' | '}' => open = open.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

#[derive(Default)]
struct State {
    nodes: usize,
    bytes: usize,
    depth: usize,
    path: Vec<String>,
}

impl State {
    fn at(&self) -> String {
        if self.path.is_empty() {
            "the document".to_owned()
        } else {
            self.path.join(".")
        }
    }

    fn count<E: de::Error>(&mut self) -> Result<(), E> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(E::custom(format!(
                "the document holds more than {MAX_NODES} values, aliases expanded"
            )));
        }
        Ok(())
    }

    fn text<E: de::Error>(&mut self, length: usize) -> Result<(), E> {
        self.bytes += length;
        if self.bytes > MAX_DOCUMENT_BYTES {
            return Err(E::custom(format!(
                "the document holds more than {MAX_DOCUMENT_BYTES} bytes of text, aliases expanded"
            )));
        }
        Ok(())
    }

    fn descend<E: de::Error>(&mut self) -> Result<(), E> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(E::custom(format!(
                "{} nests deeper than {MAX_NESTING} levels",
                self.at()
            )));
        }
        Ok(())
    }
}

struct Seed<'s> {
    state: &'s RefCell<State>,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Node;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
        self.state.borrow_mut().count()?;
        deserializer.deserialize_any(NodeVisitor { state: self.state })
    }
}

struct NodeVisitor<'s> {
    state: &'s RefCell<State>,
}

impl<'de> Visitor<'de> for NodeVisitor<'_> {
    type Value = Node;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a YAML value without a tag")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Node, E> {
        Ok(Node::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Node, E> {
        Ok(Node::Int(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Node, E> {
        Ok(i64::try_from(value).map_or_else(|_| Node::Big(value.to_string()), Node::Int))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Node, E> {
        Ok(Node::Real(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Node, E> {
        self.state.borrow_mut().text(value.len())?;
        Ok(Node::String(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        self.state.borrow_mut().descend()?;
        let mut items = Vec::new();
        loop {
            let index = items.len();
            self.state.borrow_mut().path.push(format!("[{index}]"));
            let next = seq.next_element_seed(Seed { state: self.state })?;
            self.state.borrow_mut().path.pop();
            match next {
                Some(node) => items.push(node),
                None => break,
            }
        }
        self.state.borrow_mut().depth -= 1;
        Ok(Node::Sequence(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        self.state.borrow_mut().descend()?;
        let mut seen = HashSet::new();
        let mut members = Vec::new();
        while let Some(key) = map.next_key_seed(KeySeed)? {
            if key == "<<" {
                return Err(de::Error::custom(format!(
                    "{} holds a merge key (<<), which is not supported",
                    self.state.borrow().at()
                )));
            }
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "{} names the key {key:?} twice",
                    self.state.borrow().at()
                )));
            }
            self.state.borrow_mut().path.push(key.clone());
            let value = map.next_value_seed(Seed { state: self.state })?;
            self.state.borrow_mut().path.pop();
            members.push((key, value));
        }
        self.state.borrow_mut().depth -= 1;
        Ok(Node::Mapping(Map(members)))
    }
}

/// A mapping key as the text written: `0`, `"0"` and `1.50` are the keys `0`, `0` and `1.50`. A
/// key with a tag, or one that is not a scalar, is refused.
struct KeySeed;

impl<'de> DeserializeSeed<'de> for KeySeed {
    type Value = String;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<String, D::Error> {
        deserializer.deserialize_enum("Key", &[], KeyVisitor)
    }
}

struct KeyVisitor;

impl<'de> Visitor<'de> for KeyVisitor {
    type Value = String;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a scalar key without a tag")
    }

    /// The parser offers an untagged scalar as a unit variant named by the text written, and a
    /// tagged one as a variant that carries a value.
    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<String, A::Error> {
        let (name, variant) = data.variant::<String>()?;
        match variant.newtype_variant::<IgnoredAny>() {
            Ok(_) => Err(de::Error::custom("a key with a tag is not supported")),
            Err(_) => Ok(name),
        }
    }
}
