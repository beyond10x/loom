//! The first credential a `loom.agent/1` document holds, found by walking the YAML the parser
//! reads, before the document is decoded.
//!
//! The walk visits every node of the document as the parser presents it: every scalar, every
//! mapping key, every YAML tag, in document order, whatever member it stands in and whether or
//! not the domain declares that member. A value the decoder would refuse (an enum spelling it does
//! not declare, an unknown key) is walked like any other, so a credential is found before the
//! decoder can describe it. Nothing is kept but the first credential's member and shape.
//!
//! A member is spelt as the document spells it: `bindings[1].base_url`, `channel.allow[0]`,
//! `writes.mode`. A credential spelt as a key is named by the mapping that holds it, `document` at
//! the top, so the member never repeats it.

use std::cell::{Cell, RefCell};
use std::fmt;

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};

use crate::credential;

/// What one node is charged beyond its text, as the decoder's alias bound charges it
/// (`loom_sdk::models`, crate-private there).
const NODE_BYTES: usize = 64;

/// What a document may cost per byte of its source, as the decoder's alias bound allows it.
const EXPANDED_PER_SOURCE_BYTE: usize = NODE_BYTES + 2;

/// The budget of the shortest documents, as the decoder's alias bound allows it.
const MIN_EXPANDED_BYTES: usize = 64 * 1024;

/// The decoder's byte bound: a longer text is refused before it is read, and is not walked.
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// What a walk of `text` may cost, every alias replayed, when the decoder refused it: the
/// decoder's own alias bound, so the walk reads no more than the decoder would have. `None` when
/// the decoder refuses the text before reading it: beyond its byte bound, or opening more flow
/// collections than it reads.
pub(crate) fn bound(text: &str) -> Option<usize> {
    let openers = text
        .bytes()
        .filter(|byte| matches!(byte, b'[' | b'{'))
        .count();
    if text.len() > MAX_BYTES || openers > loom_sdk::agent::MAX_FLOW_OPENERS {
        return None;
    }
    Some(
        text.len()
            .saturating_mul(EXPANDED_PER_SOURCE_BYTE)
            .max(MIN_EXPANDED_BYTES),
    )
}

/// The member and shape of the first credential `text` holds, walking at most `budget`. `None`
/// when it holds none, and when `text` is not one YAML document (a scan error, an empty stream,
/// several documents) or costs more than `budget`: such a text is the decoder's to refuse.
pub(crate) fn first(text: &str, budget: usize) -> Option<(String, String)> {
    let walk = Walk {
        left: Cell::new(budget),
        found: RefCell::new(None),
    };
    let root = Node {
        walk: &walk,
        at: String::new(),
    };
    match root.deserialize(serde_yaml_ng::Deserializer::from_str(text)) {
        Ok(()) => walk.found.into_inner(),
        Err(_) => None,
    }
}

/// One walk: what it may still spend and the first credential it found.
struct Walk {
    left: Cell<usize>,
    found: RefCell<Option<(String, String)>>,
}

impl Walk {
    fn charge<E: de::Error>(&self, text: usize) -> Result<(), E> {
        let left = self
            .left
            .get()
            .checked_sub(text.saturating_add(NODE_BYTES))
            .ok_or_else(|| E::custom("the walk's budget is spent"))?;
        self.left.set(left);
        Ok(())
    }

    /// Charges `value` and keeps it as the credential found when it is the first.
    fn text<E: de::Error>(&self, member: &str, value: &str) -> Result<(), E> {
        self.charge(value.len())?;
        let mut found = self.found.borrow_mut();
        if found.is_none()
            && let Some(shape) = credential(value)
        {
            let member = if member.is_empty() {
                "document"
            } else {
                member
            };
            *found = Some((member.to_owned(), shape));
        }
        Ok(())
    }
}

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}.{name}")
    }
}

/// A node standing at the member `at` (empty at the top).
struct Node<'w> {
    walk: &'w Walk,
    at: String,
}

impl Node<'_> {
    fn child(&self, at: String) -> Self {
        Node {
            walk: self.walk,
            at,
        }
    }
}

impl<'de> DeserializeSeed<'de> for Node<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML node")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_i128<E: de::Error>(self, _: i128) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_u128<E: de::Error>(self, _: u128) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<(), E> {
        self.walk.text(&self.at, value)
    }
    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<(), E> {
        self.walk.text(&self.at, &String::from_utf8_lossy(value))
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        self.walk.charge(0)
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.walk.charge(0)?;
        deserializer.deserialize_any(self)
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.walk.charge(0)?;
        deserializer.deserialize_any(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        self.walk.charge(0)?;
        let mut index = 0usize;
        while seq
            .next_element_seed(self.child(format!("{}[{index}]", self.at)))?
            .is_some()
        {
            index += 1;
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        self.walk.charge(0)?;
        while let Some(name) = map.next_key_seed(Key(self.child(self.at.clone())))? {
            map.next_value_seed(self.child(join(&self.at, &name)))?;
        }
        Ok(())
    }
    /// A tagged node (`!tag value`): the tag is walked as a key of the node, then the value.
    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<(), A::Error> {
        let (_, value) = data.variant_seed(Key(self.child(self.at.clone())))?;
        value.newtype_variant_seed(self)
    }
}

/// A mapping key (or a tag) of the node at `.0.at`: walked like a value, and answering how the
/// member it opens is spelt, `?` when it is not text. A credential in it is named by the node.
struct Key<'w>(Node<'w>);

impl<'de> DeserializeSeed<'de> for Key<'_> {
    type Value = String;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<String, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Key<'_> {
    type Value = String;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML key")
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
        self.0.walk.text(&self.0.at, value)?;
        Ok(value.to_owned())
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<String, E> {
        self.0.visit_bool(value).map(|()| "?".to_owned())
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<String, E> {
        self.0.visit_i64(value).map(|()| "?".to_owned())
    }
    fn visit_i128<E: de::Error>(self, value: i128) -> Result<String, E> {
        self.0.visit_i128(value).map(|()| "?".to_owned())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<String, E> {
        self.0.visit_u64(value).map(|()| "?".to_owned())
    }
    fn visit_u128<E: de::Error>(self, value: u128) -> Result<String, E> {
        self.0.visit_u128(value).map(|()| "?".to_owned())
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<String, E> {
        self.0.visit_f64(value).map(|()| "?".to_owned())
    }
    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<String, E> {
        self.0.visit_bytes(value).map(|()| "?".to_owned())
    }
    fn visit_none<E: de::Error>(self) -> Result<String, E> {
        self.0.visit_none().map(|()| "?".to_owned())
    }
    fn visit_unit<E: de::Error>(self) -> Result<String, E> {
        self.0.visit_unit().map(|()| "?".to_owned())
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<String, D::Error> {
        self.0.visit_some(deserializer).map(|()| "?".to_owned())
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<String, D::Error> {
        self.0
            .visit_newtype_struct(deserializer)
            .map(|()| "?".to_owned())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<String, A::Error> {
        self.0.visit_seq(seq).map(|()| "?".to_owned())
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<String, A::Error> {
        self.0.visit_map(map).map(|()| "?".to_owned())
    }
    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<String, A::Error> {
        self.0.visit_enum(data).map(|()| "?".to_owned())
    }
}
