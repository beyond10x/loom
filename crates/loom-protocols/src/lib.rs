//! Trusted, immutable-per-run protocol catalogs and offline installed definitions.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod git;
mod store;
use b10x_canon::model::Protocol;
pub use intake::protocols::{ProtocolDefinition, ProtocolSource, SourceKind};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub use store::InstallStore;

/// Maximum size of a protocol document, before parsing.
pub const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
/// Loom's built-in read-only clock protocol.
pub const SYSTEM_QUERY_YAML: &str = include_str!("../../../protocols/system-query/1.yaml");
/// Loom's read-only protocol a plugin turn answers an inbound item under. Plugin catalog only.
pub const INBOUND_ANSWER_YAML: &str = include_str!("../../../protocols/inbound-answer/1.yaml");

/// A definition and its validated Canon model. Definition availability grants no authority.
#[derive(Debug, Clone)]
pub struct ProtocolEntry {
    /// Exact definition bytes and their provenance.
    pub definition: ProtocolDefinition,
    /// Canon's validated and compilable model.
    pub model: Protocol,
}
impl ProtocolEntry {
    /// Registration identity, as `name@major`.
    pub fn name(&self) -> &str {
        &self.definition.name
    }
    /// Human description offered to intake.
    pub fn description(&self) -> &str {
        self.model
            .protocol
            .description
            .as_deref()
            .unwrap_or("(no description)")
    }
    /// Artifact names and descriptions declared by this definition.
    pub fn artifacts(&self) -> Vec<(String, String)> {
        self.model
            .artifacts
            .iter()
            .map(|(id, item)| (id.to_string(), item.description.clone().unwrap_or_default()))
            .collect()
    }
    /// The case inputs this definition declares, in declaration order; empty when it declares
    /// none.
    pub fn inputs(&self) -> Vec<String> {
        self.model
            .case
            .as_ref()
            .map(|case| case.inputs.ids().map(ToString::to_string).collect())
            .unwrap_or_default()
    }
    /// The actions this definition declares, in declaration order: what a workflow definition
    /// over it may name (`b10x_loom_executor::workflow::load`).
    pub fn actions(&self) -> Vec<String> {
        self.model
            .actions
            .iter()
            .map(|(id, _)| id.to_string())
            .collect()
    }
    /// Require the exact clock contract supported by the local host, irrespective of naming.
    /// Descriptive text and protocol header identity do not affect execution semantics.
    pub fn clock_compatible(&self) -> Result<(), String> {
        let clock = b10x_canon::model::parse(SYSTEM_QUERY_YAML).map_err(|e| e.to_string())?;
        let semantic = |model: &Protocol| {
            let mut model = model.clone();
            model.protocol = clock.protocol.clone();
            macro_rules! clear_descriptions {
                ($($field:ident),*) => { $(
                    model.$field = b10x_canon::model::Declarations::new(model.$field.iter().map(|(key, value)| {
                        let mut value = value.clone();
                        value.description = None;
                        (key.clone(), value)
                    }).collect());
                )* };
            }
            clear_descriptions!(
                artifacts,
                evidence_kinds,
                claims,
                obligations,
                actions,
                outcomes,
                invalidation
            );
            model
        };
        if semantic(&self.model) == semantic(&clock) {
            Ok(())
        } else {
            Err(format!(
                "NoLocalExecutor: {} requires bindings or artifact initialization beyond the supported system.time.read clock contract",
                self.name()
            ))
        }
    }
}
/// Host-owned snapshot shared by routing and case initialization.
#[derive(Debug, Clone, Default)]
pub struct ProtocolCatalog {
    entries: BTreeMap<String, ProtocolEntry>,
}
impl ProtocolCatalog {
    /// Engineering definitions plus Loom's built-in system query.
    pub fn bundled() -> Result<Self, String> {
        let mut catalog = Self::engineering()?;
        catalog.add_yaml(
            "system-query@1",
            SYSTEM_QUERY_YAML,
            ProtocolSource {
                kind: SourceKind::Loom,
                location: "loom".into(),
                revision: env!("CARGO_PKG_VERSION").into(),
                path: "protocols/system-query/1.yaml".into(),
            },
        )?;
        Ok(catalog)
    }
    /// The plugin host's catalog: `inbound-answer@1` only. Routing and `protocols list` read
    /// [`Self::bundled`], which does not hold it.
    pub fn plugins() -> Result<Self, String> {
        let mut catalog = Self::default();
        catalog.add_yaml(
            "inbound-answer@1",
            INBOUND_ANSWER_YAML,
            ProtocolSource {
                kind: SourceKind::Loom,
                location: "loom".into(),
                revision: env!("CARGO_PKG_VERSION").into(),
                path: "protocols/inbound-answer/1.yaml".into(),
            },
        )?;
        Ok(catalog)
    }
    /// Engineering-only compatibility catalog used by existing embedding entrypoints.
    pub fn engineering() -> Result<Self, String> {
        let mut catalog = Self::default();
        for (name, major) in canon_engineering::registry::list() {
            let builtin =
                canon_engineering::registry::get(name, major).map_err(|e| e.to_string())?;
            catalog.add_yaml(
                &format!("{name}@{major}"),
                builtin.yaml,
                ProtocolSource {
                    kind: SourceKind::Engineering,
                    location: "engineering-protocols".into(),
                    revision: "0.9.0".into(),
                    path: format!("protocols/{name}/{major}.yaml"),
                },
            )?;
        }
        Ok(catalog)
    }
    /// Validate and add one host-selected definition. Duplicate identities always refuse.
    ///
    /// `name` is the registration, `name@major`: lowercase letters, digits and hyphens before `@`,
    /// starting with a lowercase letter and at most 100 bytes, then a positive canonical major,
    /// the document's revision. A protocol id is not one; `self.improvement/1`
    /// registers as `self-improvement@1`, as the bundled `software-change@1` holds
    /// `software.change/1`, and a refused registration names the one its protocol id takes.
    pub fn add_yaml(
        &mut self,
        name: &str,
        yaml: &str,
        source: ProtocolSource,
    ) -> Result<(), String> {
        if self.entries.contains_key(name) {
            return Err(format!("duplicate protocol registration: {name}"));
        }
        let Some(major) = parse_registration(name) else {
            let declared = (yaml.len() <= MAX_PROTOCOL_BYTES)
                .then(|| b10x_canon::model::parse(yaml).ok())
                .flatten();
            return Err(registration_refusal(
                name,
                declared
                    .as_ref()
                    .map(|model| (model.protocol.id.as_str(), model.protocol.revision)),
            ));
        };
        if yaml.len() > MAX_PROTOCOL_BYTES {
            return Err("protocol document exceeds 1 MiB".into());
        }
        let model = b10x_canon::model::parse(yaml).map_err(|e| format!("{name}: {e}"))?;
        if model.protocol.revision != major {
            return Err(format!(
                "{name}: registration major does not match document revision {}",
                model.protocol.revision
            ));
        }
        b10x_canon::validate::validate(&model)
            .map_err(|e| format!("{name}: invalid protocol: {e:?}"))?;
        b10x_canon::ir::compile(&model)
            .map_err(|e| format!("{name}: cannot compile protocol: {e:?}"))?;
        self.entries.insert(
            name.into(),
            ProtocolEntry {
                definition: ProtocolDefinition {
                    name: name.into(),
                    yaml: yaml.into(),
                    sha256: digest(yaml.as_bytes()),
                    source,
                },
                model,
            },
        );
        Ok(())
    }
    /// Entries in deterministic registration order.
    pub fn iter(&self) -> impl Iterator<Item = &ProtocolEntry> {
        self.entries.values()
    }
    /// Find a registration by exact identity.
    pub fn get(&self, name: &str) -> Option<&ProtocolEntry> {
        self.entries.get(name)
    }
}
/// The major of `name`, refusing a registration that is not `name@major`. The refusal can read
/// no definition, so it derives the registration it names from `name` itself.
fn registration_major(name: &str) -> Result<u64, String> {
    parse_registration(name).ok_or_else(|| registration_refusal(name, None))
}
/// The rule [`parse_registration`] holds a registration to, as a refusal states it.
const REGISTRATION_RULE: &str = "lowercase letters, digits and hyphens before @, starting with a \
                                 lowercase letter and at most 100 bytes, then a positive canonical \
                                 major";
/// The major of a well-formed registration: lowercase letters, digits and hyphens before `@`,
/// starting with a lowercase letter and at most 100 bytes, then a positive canonical major.
fn parse_registration(name: &str) -> Option<u64> {
    let (stem, version) = name.split_once('@')?;
    if stem.is_empty()
        || stem.len() > 100
        || !stem.as_bytes()[0].is_ascii_lowercase()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return None;
    }
    let major: u64 = version.parse().ok()?;
    (major != 0 && major.to_string() == version).then_some(major)
}
/// The registration the catalog gives protocol `id` at `revision`, as the bundled catalog does
/// (`software.change/1` is `software-change@1`): dots and underscores become hyphens and
/// `/<major>` becomes `@<major>`. `None` when the result is still no registration.
fn registration_for(id: &str, revision: &str) -> Option<String> {
    if id.len() > 100 {
        return None;
    }
    let name = format!("{}@{revision}", id.replace(['.', '_'], "-"));
    parse_registration(&name).map(|_| name)
}
/// Why `name` is no registration, naming the registration its protocol id takes: the id and
/// revision the definition declares when it was read (`declared`), else the id `name` spells
/// before its last `@` (or `/`) with the major after it. When the mapping gives no registration
/// for that id, the refusal says so and states the rule alone.
fn registration_refusal(name: &str, declared: Option<(&str, u64)>) -> String {
    let rule = format!(
        "protocol registration must be name@major ({REGISTRATION_RULE}), not {}",
        echo(name)
    );
    let (id, revision) = match declared {
        Some((id, revision)) => (id.to_owned(), revision.to_string()),
        None => match name.rsplit_once('@').or_else(|| name.rsplit_once('/')) {
            Some((id, revision)) => (id.to_owned(), revision.to_owned()),
            None => return format!("{rule}; the name given spells no protocol id and major"),
        },
    };
    match registration_for(&id, &revision) {
        Some(expected) => format!(
            "{rule}: protocol id {id}/{revision} registers as {expected}, its dots and \
             underscores as hyphens and /<major> as @<major>, as the bundled software-change@1 \
             holds software.change/1"
        ),
        None => format!(
            "{rule}; protocol id {} has no registration under the bundled catalog's mapping",
            echo(&format!("{id}/{revision}"))
        ),
    }
}
/// How many bytes of a refused name its refusal echoes.
const ECHOED_BYTES: usize = 120;
/// `text` quoted with its control and format characters escaped, at most [`ECHOED_BYTES`] of it,
/// cut on a character boundary and marked when cut, so a refusal's length is bounded.
fn echo(text: &str) -> String {
    if text.len() <= ECHOED_BYTES {
        return format!("{text:?}");
    }
    let mut end = ECHOED_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{:?}… (cut to its first {end} of {} bytes)",
        &text[..end],
        text.len()
    )
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
