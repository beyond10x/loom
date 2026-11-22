//! The files the tree keeps in its store beside the governor's case files: the manifest, and the
//! writes and bounded reads of the tree's own documents.

use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use loom::tree::{TreeCatalog, TreeManifest, TreeRefusalReason};
use serde_json::{Map, Value, json};

use crate::{Refused, unreadable};

/// The `format` of every tree manifest.
pub const FORMAT: &str = "loom.tree/1";

/// The tree's manifest file in the store, written once by `tree open`.
pub const MANIFEST: &str = "tree.json";

/// The most bytes of one document of the tree the store is read for.
pub(crate) const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

/// The bytes of the regular file `path`, at most [`MAX_DOCUMENT_BYTES`]; `None` when it is
/// absent. A link, a directory or any other entry is refused and never followed.
pub(crate) fn read_regular(path: &Path) -> Result<Option<Vec<u8>>, Refused> {
    match fs::symlink_metadata(path) {
        Ok(entry) if entry.file_type().is_file() => {}
        Ok(_) => {
            return Err(unreadable(
                path.display(),
                "is not a regular file; it is not followed",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unreadable(path.display(), error)),
    }
    let mut bytes = Vec::new();
    let read = OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|file| file.take(MAX_DOCUMENT_BYTES + 1).read_to_end(&mut bytes));
    read.map_err(|error| unreadable(path.display(), error))?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(unreadable(path.display(), "is larger than 1 MiB"));
    }
    Ok(Some(bytes))
}

/// The JSON object of the regular file `path`; `None` when it is absent.
pub(crate) fn read_object(path: &Path) -> Result<Option<Map<String, Value>>, Refused> {
    let Some(bytes) = read_regular(path)? else {
        return Ok(None);
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(object)) => Ok(Some(object)),
        Ok(_) => Err(unreadable(path.display(), "is not one JSON object")),
        Err(error) => Err(unreadable(path.display(), error)),
    }
}

/// Writes `bytes` as `name` in the store `dir`: to a fresh file in `dir`, synced and renamed over
/// the name, so a reader sees the previous document or this one, and a link at the name is
/// replaced, never followed. Called holding the store ([`crate::StoreLock`]).
pub(crate) fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Refused> {
    let target = dir.join(name);
    let temporary = dir.join(format!(".tree-{}-{name}", std::process::id()));
    let written = (|| {
        let _ = fs::remove_file(&temporary);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &target)?;
        if let Ok(directory) = fs::File::open(dir) {
            let _ = directory.sync_all();
        }
        Ok::<(), io::Error>(())
    })();
    written.map_err(|error| {
        let _ = fs::remove_file(&temporary);
        unreadable(target.display(), error)
    })
}

/// `value` as the bytes the tree writes: pretty JSON ended by a line feed.
pub(crate) fn document(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The text member `name` of `object`, read from `path`.
pub(crate) fn text<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    path: &Path,
) -> Result<&'a str, Refused> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| unreadable(path.display(), format!("holds no text member {name:?}")))
}

/// The integer member `name` of `object`, read from `path`.
pub(crate) fn integer(
    object: &Map<String, Value>,
    name: &str,
    path: &Path,
) -> Result<i64, Refused> {
    object
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| unreadable(path.display(), format!("holds no integer member {name:?}")))
}

/// The manifest as `tree.json` holds it.
pub(crate) fn manifest_json(manifest: &TreeManifest) -> Value {
    json!({
        "format": manifest.format,
        "root": manifest.root,
        "workspace": manifest.workspace,
        "bindings": manifest.bindings,
        "catalog": match manifest.catalog {
            TreeCatalog::Host => "Host",
            TreeCatalog::Directory => "Directory",
        },
    })
}

/// The manifest of the tree in the store `dir`.
///
/// # Errors
/// `NotATree` when the store holds no manifest; `StoreUnreadable` when it cannot be read, is not
/// one, or names another format.
pub fn read_manifest(dir: &Path) -> Result<TreeManifest, Refused> {
    let path = dir.join(MANIFEST);
    let Some(object) = read_object(&path)? else {
        return Err(Refused::new(
            TreeRefusalReason::NotATree,
            format!("{} holds no tree: it has no {MANIFEST}", dir.display()),
        ));
    };
    let format = text(&object, "format", &path)?;
    if format != FORMAT {
        return Err(unreadable(
            path.display(),
            format!("is of format {format:?}, not {FORMAT}"),
        ));
    }
    let catalog = match text(&object, "catalog", &path)? {
        "Host" => TreeCatalog::Host,
        "Directory" => TreeCatalog::Directory,
        other => {
            return Err(unreadable(
                path.display(),
                format!("names the catalog {other:?}, neither Host nor Directory"),
            ));
        }
    };
    Ok(TreeManifest {
        format: format.to_owned(),
        root: text(&object, "root", &path)?.to_owned(),
        workspace: text(&object, "workspace", &path)?.to_owned(),
        bindings: text(&object, "bindings", &path)?.to_owned(),
        catalog,
    })
}

/// The store directory `dir` as an absolute path, which must exist.
pub(crate) fn existing_store(dir: &Path) -> Result<PathBuf, Refused> {
    let entry = fs::symlink_metadata(dir).map_err(|error| {
        Refused::new(
            TreeRefusalReason::NotATree,
            format!("{} cannot be read as a store: {error}", dir.display()),
        )
    })?;
    if !entry.is_dir() {
        return Err(Refused::new(
            TreeRefusalReason::NotATree,
            format!("{} is not a directory", dir.display()),
        ));
    }
    fs::canonicalize(dir).map_err(|error| unreadable(dir.display(), error))
}
