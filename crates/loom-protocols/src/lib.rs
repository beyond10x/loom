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
                    revision: "0.3.0".into(),
                    path: format!("protocols/{name}/{major}.yaml"),
                },
            )?;
        }
        Ok(catalog)
    }
    /// Validate and add one host-selected definition. Duplicate identities always refuse.
    pub fn add_yaml(
        &mut self,
        name: &str,
        yaml: &str,
        source: ProtocolSource,
    ) -> Result<(), String> {
        if self.entries.contains_key(name) {
            return Err(format!("duplicate protocol registration: {name}"));
        }
        let major = registration_major(name)?;
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
fn registration_major(name: &str) -> Result<u64, String> {
    let invalid = || {
        "protocol registration must be name@major (lowercase letters, digits and hyphens; positive canonical major)".to_owned()
    };
    let (stem, version) = name.split_once('@').ok_or_else(invalid)?;
    if stem.is_empty()
        || stem.len() > 100
        || !stem.as_bytes()[0].is_ascii_lowercase()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(invalid());
    }
    let major: u64 = version.parse().map_err(|_| invalid())?;
    if major == 0 || major.to_string() != version {
        return Err(invalid());
    }
    Ok(major)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
