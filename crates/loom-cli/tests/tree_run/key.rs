//! The names and keys of the trees the `tree_run` tests share
//! (`story:tree-run-binary-fits-its-start-bound`), in one file that both sides compile:
//! `crates/loom-cli/tests/tree_run.rs`, whose builders make the trees, and
//! `crates/loom-xtask/src/shared_trees.rs`, the setup that computes the key and builds what is
//! missing before any `tree_run` test runs. It reads no executable: a key over the bytes of a test
//! binary differs between two builds of one source in two scopes.

// Each side uses a part: the setup does not lock, a test computes no tree key.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

/// The variable naming the key of this run's shared trees, 16 lowercase hex digits: the setup
/// exports it to every `tree_run` test through nextest's environment file, and gives it to the
/// `cargo test` it runs. A test without it builds its trees under its own process id
/// ([`private_key`]).
pub const KEY_VARIABLE: &str = "LOOM_TREE_RUN_KEY";

/// Each shared tree: its name and the file its builder writes last, so an attempt directory that
/// holds it is whole (its sentinel).
pub const SHARED: [(&str, &str); 3] = [
    ("ended", "ended.json"),
    ("stepped", "stepped.json"),
    ("observed", "seen.json"),
];

/// The protocols each test's user has installed: registration and file under the fixture's
/// `protocols/`.
pub const INSTALLED: [(&str, &str); 6] = [
    ("tour@1", "tour.yaml"),
    ("tour-leg@1", "tour-leg.yaml"),
    ("fork@1", "../extra/fork.yaml"),
    ("sign@1", "../extra/sign.yaml"),
    ("answer@1", "../extra/answer.yaml"),
    ("review@1", "../extra/review.yaml"),
];

/// Whether `key` is a key the setup gives: 16 lowercase hex digits.
pub fn is_setup_key(key: &str) -> bool {
    key.len() == 16
        && key
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// The key of a process that was given none: its own, so no other process reads what it builds.
/// It passes it on to a child process that builds for it.
pub fn private_key(pid: u32) -> String {
    format!("pid{pid}")
}

/// Whether `key` is a key of [`private_key`]'s form.
pub fn is_private_key(key: &str) -> bool {
    key.strip_prefix("pid")
        .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The bytes of each of [`INSTALLED`] under `fixture`.
pub fn installed_bytes(fixture: &Path) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    INSTALLED
        .iter()
        .map(|(name, file)| {
            let path = fixture.join("protocols").join(file);
            fs::read(&path)
                .map(|bytes| (*name, bytes))
                .map_err(|e| format!("read {}: {e}", path.display()))
        })
        .collect()
}

/// The key of the installed home: SHA-256 over each protocol's registration and bytes, then the
/// tree key, as 16 hex digits. The install's code is in the sources the tree key covers.
pub fn home_key(installed: &[(&str, Vec<u8>)], tree_key: &str) -> String {
    let mut hasher = Sha256::new();
    for (name, bytes) in installed {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    hasher.update(tree_key.as_bytes());
    hasher.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The prefix of every attempt directory of the tree `name` under `key`.
pub fn attempt_prefix(name: &str, key: &str) -> String {
    format!("tree-run-{name}-{key}-")
}

/// The lock file the builders of the tree `name` under `key` take in turn.
pub fn lock_name(name: &str, key: &str) -> String {
    format!("tree-run-{name}-{key}.lock")
}

/// The attempt directory of the tree `name` under `key` in `base` that holds `sentinel`, the
/// first by name when there are more. A directory without it is never returned.
pub fn sealed_attempt(base: &Path, name: &str, key: &str, sentinel: &str) -> Option<PathBuf> {
    let prefix = attempt_prefix(name, key);
    let mut sealed: Vec<PathBuf> = fs::read_dir(base)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .filter(|path| path.join(sentinel).is_file())
        .collect();
    sealed.sort();
    sealed.into_iter().next()
}
