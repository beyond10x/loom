//! The lock of the vendored engineering definitions, `protocols/engineering/lock.json`
//! ([`FORMAT`]): the source repository, tag and commit, and the path and sha256 of each file.
//!
//! This module is the lock's one encoder ([`encode`]) and one decoder ([`decode`]), and
//! [`check`] holds a set of files to a decoded lock. [`crate::ProtocolCatalog::from_vendored`]
//! decodes and checks before it admits any definition.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use intake::protocols::{LockRefusal, LockedFile, ProtocolLock};
use serde_json::Value;

/// The repository the vendored engineering definitions come from; a lock naming another refuses.
pub const REPOSITORY: &str = "https://github.com/beyond10x/engineering-protocols";
/// The lock's format.
pub const FORMAT: &str = "loom.protocol-lock/1";
/// The path a refusal of the lock as a whole names.
const LOCK_PATH: &str = "lock.json";
const MEMBERS: [&str; 5] = ["format", "repository", "tag", "commit", "files"];
const FILE_MEMBERS: [&str; 2] = ["path", "sha256"];

/// Why a lock or a set of files was refused: the refusal, the path it names, and its detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockError {
    /// What was refused.
    pub refusal: LockRefusal,
    /// The file's path below the vendored directory, or `lock.json` for the lock as a whole.
    pub path: String,
    /// `DigestMismatch` only: the locked digest and the digest of the file found.
    pub digests: Option<(String, String)>,
    /// `Malformed` only: what is wrong with the lock.
    pub problem: Option<String>,
}

impl LockError {
    fn malformed(path: &str, problem: impl Into<String>) -> Self {
        Self {
            refusal: LockRefusal::Malformed,
            path: path.into(),
            digests: None,
            problem: Some(problem.into()),
        }
    }
    fn file(refusal: LockRefusal, path: &str) -> Self {
        Self {
            refusal,
            path: path.into(),
            digests: None,
            problem: None,
        }
    }
}

/// `<Variant> <path>`, with ` (locked <digest>, found <digest>)` for a digest mismatch and
/// `: <what is wrong>` for a malformed lock.
impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Generated enums derive no `Display`; the variant is spelled as Rust spells it.
        let variant = match self.refusal {
            LockRefusal::Malformed => "Malformed",
            LockRefusal::DigestMismatch => "DigestMismatch",
            LockRefusal::Unlocked => "Unlocked",
            LockRefusal::Missing => "Missing",
        };
        write!(f, "{variant} {}", self.path)?;
        if let Some((locked, found)) = &self.digests {
            write!(f, " (locked {locked}, found {found})")?;
        }
        if let Some(problem) = &self.problem {
            write!(f, ": {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LockError {}

/// The lock entry of `bytes` at `path`: the path and the sha256 of the bytes as lowercase hex.
pub fn locked_file(path: &str, bytes: &[u8]) -> LockedFile {
    LockedFile {
        path: path.into(),
        sha256: crate::digest(bytes),
    }
}

/// The lock's bytes: one JSON object with the members `format`, `repository`, `tag`, `commit`,
/// `files` in that order, the files as `path`, `sha256` objects sorted by path as bytes,
/// indented by two spaces, one member per line, and one trailing newline.
pub fn encode(lock: &ProtocolLock) -> Vec<u8> {
    let string = |text: &str| Value::from(text).to_string();
    let mut files: Vec<&LockedFile> = lock.files.iter().collect();
    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let mut out = String::from("{\n");
    for (member, value) in [
        ("format", &lock.format),
        ("repository", &lock.repository),
        ("tag", &lock.tag),
        ("commit", &lock.commit),
    ] {
        out.push_str(&format!("  {}: {},\n", string(member), string(value)));
    }
    if files.is_empty() {
        out.push_str("  \"files\": []\n");
    } else {
        out.push_str("  \"files\": [\n");
        for (at, file) in files.iter().enumerate() {
            out.push_str(&format!(
                "    {{\n      \"path\": {},\n      \"sha256\": {}\n    }}{}\n",
                string(&file.path),
                string(&file.sha256),
                if at + 1 == files.len() { "" } else { "," }
            ));
        }
        out.push_str("  ]\n");
    }
    out.push_str("}\n");
    out.into_bytes()
}

/// Decode a lock, refusing it as `Malformed`: at `lock.json` when the bytes are not JSON, a
/// top-level member is wrong, undeclared or missing, an entry has no `path`, or the bytes are not
/// what [`encode`] writes for the lock they hold; at the entry's path when the path is not
/// `<name>/<major>.yaml`, is given twice, or a member of the entry is wrong, undeclared or
/// missing.
pub fn decode(bytes: &[u8]) -> Result<ProtocolLock, LockError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| LockError::malformed(LOCK_PATH, format!("not JSON: {error}")))?;
    let Value::Object(members) = value else {
        return Err(LockError::malformed(LOCK_PATH, "not a JSON object"));
    };
    let mut top =
        only_members(&members, &MEMBERS).map_err(|e| LockError::malformed(LOCK_PATH, e))?;
    let text = |top: &mut BTreeMap<&str, &Value>, member: &str| -> Result<String, LockError> {
        match top.remove(member) {
            Some(Value::String(text)) if !text.is_empty() => Ok(text.clone()),
            _ => Err(LockError::malformed(
                LOCK_PATH,
                format!("`{member}` is not a non-empty string"),
            )),
        }
    };
    let format = text(&mut top, "format")?;
    if format != FORMAT {
        return Err(LockError::malformed(
            LOCK_PATH,
            format!("`format` is {format:?}, not {FORMAT:?}"),
        ));
    }
    let repository = text(&mut top, "repository")?;
    if repository != REPOSITORY {
        return Err(LockError::malformed(
            LOCK_PATH,
            format!("`repository` is {repository:?}, not {REPOSITORY:?}"),
        ));
    }
    let tag = text(&mut top, "tag")?;
    let commit = text(&mut top, "commit")?;
    if !is_hex(&commit, 40) {
        return Err(LockError::malformed(
            LOCK_PATH,
            "`commit` is not 40 lowercase hex digits",
        ));
    }
    let Some(Value::Array(entries)) = top.remove("files") else {
        return Err(LockError::malformed(LOCK_PATH, "`files` is not an array"));
    };
    let mut files = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    for entry in entries {
        let file = decode_file(entry)?;
        if !seen.insert(file.path.clone()) {
            return Err(LockError::malformed(&file.path, "the path is given twice"));
        }
        files.push(file);
    }
    let lock = ProtocolLock {
        format,
        repository,
        tag,
        commit,
        files,
    };
    if encode(&lock) != bytes {
        return Err(LockError::malformed(
            LOCK_PATH,
            "the bytes are not the encoder's for the lock they hold",
        ));
    }
    Ok(lock)
}

/// One `files` entry. Without a string `path` the refusal names `lock.json`, else that path.
fn decode_file(entry: &Value) -> Result<LockedFile, LockError> {
    let Value::Object(members) = entry else {
        return Err(LockError::malformed(
            LOCK_PATH,
            "a `files` entry is not an object",
        ));
    };
    let Some(Value::String(path)) = members.get("path") else {
        return Err(LockError::malformed(
            LOCK_PATH,
            "a `files` entry has no string `path`",
        ));
    };
    let fields = only_members(members, &FILE_MEMBERS).map_err(|e| LockError::malformed(path, e))?;
    if registration(path).is_none() {
        return Err(LockError::malformed(
            path,
            "the path is not <name>/<major>.yaml",
        ));
    }
    let sha256 = match fields.get("sha256") {
        Some(Value::String(sha256)) if is_hex(sha256, 64) => sha256.clone(),
        _ => {
            return Err(LockError::malformed(
                path,
                "`sha256` is not 64 lowercase hex digits",
            ));
        }
    };
    Ok(LockedFile {
        path: path.clone(),
        sha256,
    })
}

/// The members of `object`, refusing one `declared` does not name and one it names that is absent.
fn only_members<'a>(
    object: &'a serde_json::Map<String, Value>,
    declared: &[&'static str],
) -> Result<BTreeMap<&'static str, &'a Value>, String> {
    if let Some(undeclared) = object.keys().find(|key| !declared.contains(&key.as_str())) {
        return Err(format!("undeclared member `{undeclared}`"));
    }
    let mut found = BTreeMap::new();
    for member in declared {
        let value = object
            .get(*member)
            .ok_or_else(|| format!("missing member `{member}`"))?;
        found.insert(*member, value);
    }
    Ok(found)
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The registration a locked path `<name>/<major>.yaml` holds, `<name>@<major>`, by the
/// catalog's registration rule; `None` for any other path.
pub(crate) fn registration(path: &str) -> Option<String> {
    let (name, file) = path.split_once('/')?;
    let major = file.strip_suffix(".yaml")?;
    let registration = format!("{name}@{major}");
    crate::parse_registration(&registration).map(|_| registration)
}

/// Hold `files`, as (path below the vendored directory, bytes), to `lock`: a file the lock does
/// not name is `Unlocked`, a file whose sha256 is not its entry's is `DigestMismatch`, and an
/// entry no file has is `Missing`. The first refusal, in the order of `files` and then of the
/// lock's entries, is returned; later files are not looked at.
pub fn check(lock: &ProtocolLock, files: &[(&str, &[u8])]) -> Result<(), LockError> {
    let locked: BTreeMap<&str, &str> = lock
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.sha256.as_str()))
        .collect();
    for (path, bytes) in files {
        let Some(expected) = locked.get(path) else {
            return Err(LockError::file(LockRefusal::Unlocked, path));
        };
        let found = crate::digest(bytes);
        if found != *expected {
            return Err(LockError {
                digests: Some(((*expected).to_owned(), found)),
                ..LockError::file(LockRefusal::DigestMismatch, path)
            });
        }
    }
    let present: BTreeSet<&str> = files.iter().map(|(path, _)| *path).collect();
    if let Some(file) = lock
        .files
        .iter()
        .find(|file| !present.contains(file.path.as_str()))
    {
        return Err(LockError::file(LockRefusal::Missing, &file.path));
    }
    Ok(())
}
