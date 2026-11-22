//! Trusted, immutable-per-run protocol catalogs and offline installed definitions.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod git;
pub mod lock;
mod store;
/// The Canon crate Loom links, so that an embedder names one runtime dependency and links the
/// same Canon as Loom.
pub use b10x_canon as canon;
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
/// Loom's protocol a plugin turn answers an inbound item under and sends the proposed reply:
/// `inbound-answer@1` plus `reply.send`. Plugin catalog only.
pub const OPERATOR_REPLY_YAML: &str = include_str!("../../../protocols/operator-reply/1.yaml");
/// The vendored engineering definitions, `protocols/engineering/<name>/<major>.yaml`, as (path
/// below `protocols/engineering/`, bytes), sorted by path: every file of that directory other than
/// `lock.json`, whatever its name, so a file the lock does not name reaches its check.
pub const ENGINEERING_FILES: &[(&str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/engineering_files.rs"));
/// The bytes of `protocols/engineering/lock.json`, the `loom.protocol-lock/1` document
/// [`ENGINEERING_FILES`] are held to; no bytes when the file is absent, which the lock refuses.
pub const ENGINEERING_LOCK: &[u8] = include!(concat!(env!("OUT_DIR"), "/engineering_lock.rs"));

/// Why [`ProtocolCatalog::from_vendored`] built no catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendoredError {
    /// The lock refused: it is malformed, or a file is unlocked, changed or missing.
    Lock(lock::LockError),
    /// A file the lock holds is not UTF-8 or is no definition the catalog admits.
    Definition {
        /// The file's path below `protocols/engineering/`.
        path: String,
        /// What is wrong with it.
        problem: String,
    },
}
/// One line: `engineering protocol lock refused: <Variant> <path>…` or
/// `engineering protocol <path>: <problem>`.
impl std::fmt::Display for VendoredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let line = match self {
            Self::Lock(error) => format!("engineering protocol lock refused: {error}"),
            Self::Definition { path, problem } => format!("engineering protocol {path}: {problem}"),
        };
        f.write_str(&line.lines().collect::<Vec<_>>().join(" "))
    }
}
impl std::error::Error for VendoredError {}

/// The argument schema a protocol declares for one of its actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentSchema {
    /// The action's `arguments`: a JSON Schema in Canon's argument-schema dialect.
    Declared(serde_json::Value),
    /// The action declares no `arguments`: it takes only `{}`.
    Undeclared,
}

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
    /// The argument schema this definition declares for `action` (Canon's `Action.arguments`,
    /// held to Canon's argument-schema dialect by [`ProtocolCatalog::add_yaml`]):
    /// [`ArgumentSchema::Declared`] with the schema, [`ArgumentSchema::Undeclared`] when the
    /// action declares none and so takes only `{}`; `None` for an action the definition does not
    /// declare.
    pub fn argument_schema(&self, action: &str) -> Option<ArgumentSchema> {
        self.model
            .actions
            .iter()
            .find(|(id, _)| id.as_str() == action)
            .map(|(_, declared)| match &declared.arguments {
                Some(schema) => ArgumentSchema::Declared(schema.clone()),
                None => ArgumentSchema::Undeclared,
            })
    }
    /// Every action this definition declares, in declaration order, with its argument schema
    /// (`None`: it declares none): the input a host builds the executor's
    /// `b10x_loom_executor::arguments::ArgumentSchemas` from.
    pub fn argument_schemas(&self) -> Vec<(String, Option<serde_json::Value>)> {
        self.model
            .actions
            .iter()
            .map(|(id, declared)| (id.to_string(), declared.arguments.clone()))
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
    /// The plugin host's catalog: `inbound-answer@1` and `operator-reply@1`. Routing and
    /// `protocols list` read [`Self::bundled`], which holds neither.
    pub fn plugins() -> Result<Self, String> {
        let mut catalog = Self::default();
        for (name, yaml, path) in [
            (
                "inbound-answer@1",
                INBOUND_ANSWER_YAML,
                "protocols/inbound-answer/1.yaml",
            ),
            (
                "operator-reply@1",
                OPERATOR_REPLY_YAML,
                "protocols/operator-reply/1.yaml",
            ),
        ] {
            catalog.add_yaml(
                name,
                yaml,
                ProtocolSource {
                    kind: SourceKind::Loom,
                    location: "loom".into(),
                    revision: env!("CARGO_PKG_VERSION").into(),
                    path: path.into(),
                },
            )?;
        }
        Ok(catalog)
    }
    /// Engineering-only compatibility catalog used by existing embedding entrypoints: the
    /// vendored files held to their lock, [`Self::from_vendored`] over [`ENGINEERING_FILES`] and
    /// [`ENGINEERING_LOCK`], its refusal as the line [`VendoredError`] prints.
    pub fn engineering() -> Result<Self, String> {
        Self::from_vendored(ENGINEERING_FILES, ENGINEERING_LOCK).map_err(|error| error.to_string())
    }
    /// The engineering catalog of `files`, as (path below `protocols/engineering/`, bytes), held
    /// to `lock`, the bytes of a `loom.protocol-lock/1` document. The lock is decoded and every
    /// file checked against it before any definition is admitted: a malformed lock, a file the
    /// lock does not name, a file whose sha256 is not its entry's and an entry with no file each
    /// refuse with [`VendoredError::Lock`], building no catalog. Each file is then added as
    /// `<name>@<major>` from its path, with kind `Engineering`, location `engineering-protocols`,
    /// path `protocols/<path>` and the lock's `tag` as revision; a file that is not UTF-8 or that
    /// the catalog refuses is [`VendoredError::Definition`].
    pub fn from_vendored(files: &[(&str, &[u8])], lock: &[u8]) -> Result<Self, VendoredError> {
        let lock = lock::decode(lock).map_err(VendoredError::Lock)?;
        lock::check(&lock, files).map_err(VendoredError::Lock)?;
        let mut catalog = Self::default();
        for (path, bytes) in files {
            let definition = |problem: String| VendoredError::Definition {
                path: (*path).to_owned(),
                problem,
            };
            let registration = lock::registration(path)
                .ok_or_else(|| definition("the path is not <name>/<major>.yaml".into()))?;
            let yaml = std::str::from_utf8(bytes)
                .map_err(|error| definition(format!("not UTF-8: {error}")))?;
            catalog
                .add_yaml(
                    &registration,
                    yaml,
                    ProtocolSource {
                        kind: SourceKind::Engineering,
                        location: "engineering-protocols".into(),
                        revision: lock.tag.clone(),
                        path: format!("protocols/{path}"),
                    },
                )
                .map_err(definition)?;
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
            return Err(refuse_registration(name, Some(yaml)));
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
            .map_err(|e| format!("{name}: invalid protocol: {}", problems(&e)))?;
        b10x_canon::ir::compile(&model)
            .map_err(|e| format!("{name}: cannot compile protocol: {}", problems(&e)))?;
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
/// The major of `name`, refusing a registration that is not `name@major`. The refusal reads no
/// definition, so it derives the registration it names from `name` itself.
fn registration_major(name: &str) -> Result<u64, String> {
    parse_registration(name).ok_or_else(|| refuse_registration(name, None))
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
pub fn registration_for(id: &str, revision: &str) -> Option<String> {
    if id.len() > 100 {
        return None;
    }
    let name = format!("{}@{revision}", id.replace(['.', '_'], "-"));
    parse_registration(&name).map(|_| name)
}
/// Canon's problems with a protocol: the code of each (`Problem::code`, the one Canon's own
/// refusals name), then each problem as Canon's type holds it.
fn problems(problems: &[b10x_canon::validate::Problem]) -> String {
    let codes: Vec<&str> = problems.iter().map(|problem| problem.code()).collect();
    format!("{}: {problems:?}", codes.join(", "))
}
/// The refusal of `name`, which [`parse_registration`] refused, for the definition `yaml` the
/// host handed with it: the id and revision it declares when it is within
/// [`MAX_PROTOCOL_BYTES`] and parses, else `name` itself, name the registration. `add_yaml` and
/// the install store's `install_file` both refuse through it, so one input gets one answer.
fn refuse_registration(name: &str, yaml: Option<&str>) -> String {
    let declared = yaml
        .filter(|yaml| yaml.len() <= MAX_PROTOCOL_BYTES)
        .and_then(|yaml| b10x_canon::model::parse(yaml).ok());
    registration_refusal(
        name,
        declared
            .as_ref()
            .map(|model| (model.protocol.id.as_str(), model.protocol.revision)),
    )
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
