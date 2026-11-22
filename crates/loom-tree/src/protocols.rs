//! The protocols of a tree opened with `tree open --protocols <dir>`: read from the files directly
//! in `<dir>`, kept in the store as `blobs/<sha256>` with the list `tree-protocols.json`, and the
//! catalog every later command builds from those blobs alone.
//!
//! Every file directly in `<dir>` whose name ends in `.yaml` and does not start with `.` is read; a
//! subdirectory is not, and a link is read only when its real path is a regular file in `<dir>`.
//! Each file is compiled and validated with Canon through [`ProtocolCatalog::add_yaml`] under the
//! registration its document declares ([`registration_for`]), with the source kind
//! [`SourceKind::Directory`]. The catalog holds those protocols and no other.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use loom::tree::{TreeProtocol, TreeRefusalReason};
use loom_protocols::{ProtocolCatalog, ProtocolSource, SourceKind, registration_for};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::store::{document, read_regular, write_file};
use crate::{Refused, unreadable};

/// The file of a tree opened with `--protocols` that names the protocol blobs it keeps.
pub const PROTOCOLS_FILE: &str = "tree-protocols.json";

/// The format of [`PROTOCOLS_FILE`].
const PROTOCOLS_FORMAT: &str = "loom.tree-protocols/1";

/// The directory of the store the protocol blobs are kept in.
const BLOBS: &str = "blobs";

/// The most bytes of a protocol file.
const MAX_PROTOCOL_BYTES: u64 = 1024 * 1024;

/// The most bytes of Canon's message a `ProtocolRefused` names.
const MAX_MESSAGE_BYTES: usize = 1024;

/// One protocol read from a protocols directory: what `tree-protocols.json` names and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptProtocol {
    /// The entry of `tree-protocols.json`.
    pub protocol: TreeProtocol,
    /// The document, as read.
    pub yaml: String,
}

/// The protocols of a directory and the catalog of exactly those.
#[derive(Debug, Clone)]
pub struct ProtocolDirectory {
    /// The catalog of the directory's protocols, with no built-in or installed one.
    pub catalog: ProtocolCatalog,
    /// Each protocol, in the order of its file name.
    pub kept: Vec<KeptProtocol>,
}

/// `message` cut to [`MAX_MESSAGE_BYTES`] on a character boundary, marked when cut.
fn cut_message(message: &str) -> String {
    if message.len() <= MAX_MESSAGE_BYTES {
        return message.to_owned();
    }
    let mut end = MAX_MESSAGE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} [cut]", &message[..end])
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads the protocols directory `dir` (see the module documentation).
///
/// # Errors
/// `ProtocolRefused` for a file that is not a regular UTF-8 file within 1 MiB, that Canon does not
/// parse, validate or compile, or whose id gives no registration, naming the file and at most 1024
/// bytes of the message; `ProtocolLinkLeavesDirectory` for a link whose real path is not in `dir`;
/// `DuplicateProtocol` for two files declaring one protocol id, naming both; `ProtocolNotInDirectory`
/// for a `case_open` naming a protocol id no file declares, naming it and the file that names it;
/// `NoProtocolFiles` when no file is read; `StoreUnreadable` when `dir` cannot be read.
pub fn read_protocols_directory(dir: &Path) -> Result<ProtocolDirectory, Refused> {
    let directory = fs::canonicalize(dir).map_err(|error| unreadable(dir.display(), error))?;
    let mut names: Vec<(String, PathBuf)> = fs::read_dir(&directory)
        .map_err(|error| unreadable(directory.display(), error))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (name.ends_with(".yaml") && !name.starts_with('.')).then(|| (name, entry.path()))
        })
        .collect();
    names.sort();
    let mut catalog = ProtocolCatalog::default();
    let mut kept = Vec::new();
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    let mut models = Vec::new();
    for (name, path) in names {
        let refused = |detail: String| {
            Refused::new(TreeRefusalReason::ProtocolRefused, cut_message(&detail))
                .file(path.display().to_string())
        };
        let entry = fs::symlink_metadata(&path).map_err(|error| refused(error.to_string()))?;
        let real = if entry.file_type().is_symlink() {
            let real = fs::canonicalize(&path).map_err(|error| refused(error.to_string()))?;
            if real.parent() != Some(directory.as_path()) {
                return Err(Refused::new(
                    TreeRefusalReason::ProtocolLinkLeavesDirectory,
                    format!(
                        "the link {} resolves to {}, outside {}",
                        path.display(),
                        real.display(),
                        directory.display()
                    ),
                )
                .file(path.display().to_string()));
            }
            real
        } else {
            path.clone()
        };
        let metadata = fs::metadata(&real).map_err(|error| refused(error.to_string()))?;
        if metadata.is_dir() {
            continue;
        }
        if !metadata.is_file() {
            return Err(refused("is not a regular file".into()));
        }
        if metadata.len() > MAX_PROTOCOL_BYTES {
            return Err(refused("is larger than 1 MiB".into()));
        }
        let bytes = fs::read(&real).map_err(|error| refused(error.to_string()))?;
        let yaml = String::from_utf8(bytes).map_err(|_| refused("is not UTF-8".into()))?;
        let model = loom_protocols::canon::model::parse(&yaml)
            .map_err(|error| refused(format!("Canon does not parse it: {error}")))?;
        let id = model.protocol.id.as_str().to_owned();
        let revision = model.protocol.revision;
        let Some(registration) = registration_for(&id, &revision.to_string()) else {
            return Err(refused(format!(
                "its protocol id {id} at revision {revision} gives no registration"
            )));
        };
        if let Some(first) = declared.get(&id) {
            return Err(Refused::new(
                TreeRefusalReason::DuplicateProtocol,
                format!("{first} and {name} both declare the protocol {id}"),
            )
            .file(path.display().to_string()));
        }
        catalog
            .add_yaml(
                &registration,
                &yaml,
                ProtocolSource {
                    kind: SourceKind::Directory,
                    location: directory.display().to_string(),
                    revision: String::new(),
                    path: name.clone(),
                },
            )
            .map_err(&refused)?;
        declared.insert(id.clone(), name.clone());
        kept.push(KeptProtocol {
            protocol: TreeProtocol {
                registration,
                protocol: id,
                revision: i64::try_from(revision).unwrap_or(i64::MAX),
                file: name.clone(),
                sha256: sha256(yaml.as_bytes()),
            },
            yaml,
        });
        models.push((name, model));
    }
    if kept.is_empty() {
        return Err(Refused::new(
            TreeRefusalReason::NoProtocolFiles,
            format!(
                "{} holds no file whose name ends in .yaml and does not start with .",
                directory.display()
            ),
        ));
    }
    for (name, model) in &models {
        for (action, declared_action) in model.actions.iter() {
            if let Some(opening) = &declared_action.case_open
                && !declared.contains_key(opening.protocol.as_str())
            {
                return Err(Refused::new(
                    TreeRefusalReason::ProtocolNotInDirectory,
                    format!(
                        "case_open of {action} in {name} names the protocol {}, which no file of {} declares",
                        opening.protocol.as_str(),
                        directory.display()
                    ),
                )
                .file(directory.join(name).display().to_string()));
            }
        }
    }
    Ok(ProtocolDirectory { catalog, kept })
}

/// Keeps `kept` in the store `store`: each document as `blobs/<sha256>` and the list as
/// [`PROTOCOLS_FILE`]. Called holding the store.
pub(crate) fn keep(store: &Path, kept: &[KeptProtocol]) -> Result<(), Refused> {
    let blobs = store.join(BLOBS);
    fs::create_dir_all(&blobs).map_err(|error| unreadable(blobs.display(), error))?;
    for protocol in kept {
        write_file(&blobs, &protocol.protocol.sha256, protocol.yaml.as_bytes())?;
    }
    let list = json!({
        "format": PROTOCOLS_FORMAT,
        "protocols": kept
            .iter()
            .map(|protocol| crate::encode::protocol_json(&protocol.protocol))
            .collect::<Vec<_>>(),
    });
    write_file(store, PROTOCOLS_FILE, &document(&list))
}

/// The protocols `tree-protocols.json` of the store `store` names; `None` when there is no such
/// file.
pub(crate) fn kept_protocols(store: &Path) -> Result<Option<Vec<TreeProtocol>>, Refused> {
    let path = store.join(PROTOCOLS_FILE);
    let Some(bytes) = read_regular(&path)? else {
        return Ok(None);
    };
    let bad = |detail: String| unreadable(path.display(), detail);
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| bad(error.to_string()))?;
    if value["format"] != PROTOCOLS_FORMAT {
        return Err(bad(format!("its format is not {PROTOCOLS_FORMAT}")));
    }
    let Some(entries) = value["protocols"].as_array() else {
        return Err(bad("it lists no protocols".into()));
    };
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            crate::read::protocol(entry, &format!("protocols[{index}]"))
                .map_err(|error| bad(error.0))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// The catalog a tree opened with `--protocols` keeps in its store, built from its blobs alone;
/// `None` for a tree opened without it, whose commands use the host's catalog.
///
/// # Errors
/// `StoredProtocolChanged` naming the registration when a blob no longer has the SHA-256
/// [`PROTOCOLS_FILE`] names; `ProtocolRefused` when Canon refuses a kept protocol;
/// `StoreUnreadable` when a file of the store cannot be read.
pub fn stored_catalog(store: &Path) -> Result<Option<ProtocolCatalog>, Refused> {
    let store = crate::store::existing_store(store)?;
    let Some(protocols) = kept_protocols(&store)? else {
        return Ok(None);
    };
    let blobs = store.join(BLOBS);
    let mut catalog = ProtocolCatalog::default();
    for protocol in &protocols {
        let changed = || {
            Refused::new(
                TreeRefusalReason::StoredProtocolChanged,
                format!(
                    "the kept protocol {} no longer has the SHA-256 {PROTOCOLS_FILE} names",
                    protocol.registration
                ),
            )
        };
        let valid_name = protocol.sha256.len() == 64
            && protocol
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !valid_name {
            return Err(changed());
        }
        let Some(bytes) = read_regular(&blobs.join(&protocol.sha256))? else {
            return Err(changed());
        };
        if sha256(&bytes) != protocol.sha256 {
            return Err(changed());
        }
        let yaml = String::from_utf8(bytes).map_err(|_| changed())?;
        catalog
            .add_yaml(
                &protocol.registration,
                &yaml,
                ProtocolSource {
                    kind: SourceKind::Directory,
                    location: blobs.display().to_string(),
                    revision: String::new(),
                    path: protocol.file.clone(),
                },
            )
            .map_err(|error| {
                Refused::new(TreeRefusalReason::ProtocolRefused, cut_message(&error))
                    .file(protocol.file.clone())
            })?;
    }
    Ok(Some(catalog))
}
