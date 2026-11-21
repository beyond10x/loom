//! Adversary pass 1 for story:engineering-protocol-lock (wave 2026-11-21-w3, unit U3).
//!
//! Each case drives `ProtocolCatalog::from_vendored` or `lock::decode` from the story's § Outcome
//! and the brief's lines of attack, on inputs the unit's own suite does not feed it: the lock's
//! boundaries (path forms, digest forms, commit, tag, the document's shape and bytes), the order
//! of the refusals against parsing, a file set that is not a set, and whether the embedded file set
//! is the directory on disk now.

use std::path::{Path, PathBuf};

use loom_protocols::lock::{self, LockError, LockRefusal, ProtocolLock};
use loom_protocols::{ENGINEERING_FILES, ENGINEERING_LOCK, ProtocolCatalog, VendoredError};

const SOFTWARE: &str = "software-change/1.yaml";

fn committed() -> ProtocolLock {
    lock::decode(ENGINEERING_LOCK).expect("the committed lock decodes")
}

fn vendored() -> Vec<(String, Vec<u8>)> {
    ENGINEERING_FILES
        .iter()
        .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
        .collect()
}

fn borrowed(files: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8])> {
    files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect()
}

fn lock_refusal(files: &[(String, Vec<u8>)], lock: &[u8], case: &str) -> LockError {
    match ProtocolCatalog::from_vendored(&borrowed(files), lock) {
        Err(VendoredError::Lock(error)) => error,
        Err(other) => panic!("{case}: refused, but not by the lock: {other}"),
        Ok(catalog) => panic!("{case}: built a catalog of {} entries", catalog.iter().count()),
    }
}

/// The committed lock with `software-change/1.yaml`'s entry changed by `change`, encoded.
fn with_entry(change: impl FnOnce(&mut loom_protocols::lock::LockedFile)) -> Vec<u8> {
    let mut locked = committed();
    change(
        locked
            .files
            .iter_mut()
            .find(|file| file.path == SOFTWARE)
            .expect("software-change/1.yaml is locked"),
    );
    lock::encode(&locked)
}

/// A locked path that is not `<name>/<major>.yaml` refuses `Malformed` at that path, whatever the
/// form: traversal, absolute, backslash, NUL, case, percent-encoding, a trailing slash, `./`, a
/// depth other than two, a non-canonical major, another extension, the lock itself, non-ASCII.
#[test]
fn every_path_that_is_not_name_major_yaml_is_malformed_at_that_path() {
    let files = vendored();
    for bad in [
        "../software-change/1.yaml",
        "software-change/../1.yaml",
        "/software-change/1.yaml",
        "software-change\\1.yaml",
        "software-change/1.yaml\u{0}",
        "Software-change/1.yaml",
        "software%2Dchange/1.yaml",
        "software-change/1.yaml/",
        "./software-change/1.yaml",
        "a/software-change/1.yaml",
        "software-change/01.yaml",
        "software-change/0.yaml",
        "software-change/+1.yaml",
        "software-change/1.yml",
        "software-change/1.YAML",
        "lock.json",
        "softw\u{0430}re-change/1.yaml",
        "",
    ] {
        let bytes = with_entry(|file| file.path = bad.into());
        let refusal = lock_refusal(&files, &bytes, &format!("{bad:?}"));
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{bad:?}");
        assert_eq!(refusal.path, bad, "{bad:?}");
        assert!(
            refusal
                .problem
                .as_deref()
                .is_some_and(|said| said.contains("<name>/<major>.yaml")),
            "{bad:?}: {:?}",
            refusal.problem
        );
    }
}

/// A `sha256` that is not 64 lowercase hex digits refuses `Malformed` at the entry's path: a
/// `sha256:` prefix, 63 or 65 digits, a non-hex digit, mixed case.
#[test]
fn every_sha256_that_is_not_64_lowercase_hex_is_malformed_at_its_path() {
    let files = vendored();
    let good = "8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046";
    for bad in [
        format!("sha256:{good}"),
        good[..63].to_owned(),
        format!("{good}0"),
        format!("{}g", &good[..63]),
        format!("{}F", &good[..63]),
        String::new(),
    ] {
        let bytes = with_entry(|file| file.sha256 = bad.clone());
        let refusal = lock_refusal(&files, &bytes, &bad);
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{bad}");
        assert_eq!(refusal.path, SOFTWARE, "{bad}");
        assert_eq!(refusal.digests, None, "{bad}");
    }
}

/// The lock's `commit` is 40 lowercase hex digits (the implementor's decision, unit brief); any
/// other refuses `Malformed` at `lock.json`. No case of the unit's suite feeds a wrong commit.
#[test]
fn a_commit_that_is_not_40_lowercase_hex_is_malformed() {
    let files = vendored();
    let good = "70e6eff48963c9ea178d515ae33e645cf95c9773";
    for bad in [
        good.to_uppercase(),
        good[..39].to_owned(),
        format!("{good}0"),
        good[..7].to_owned(),
        "0.16.0".to_owned(),
        format!("{}g", &good[..39]),
    ] {
        let mut locked = committed();
        locked.commit = bad.clone();
        let refusal = lock_refusal(&files, &lock::encode(&locked), &bad);
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{bad}");
        assert_eq!(refusal.path, "lock.json", "{bad}");
        assert!(
            refusal
                .problem
                .as_deref()
                .is_some_and(|said| said.contains("commit")),
            "{bad}: {:?}",
            refusal.problem
        );
    }
}

/// An empty `tag` would stamp an empty revision on every entry; it refuses `Malformed`.
#[test]
fn an_empty_tag_is_malformed() {
    let mut locked = committed();
    locked.tag = String::new();
    let refusal = lock_refusal(&vendored(), &lock::encode(&locked), "empty tag");
    assert_eq!(refusal.refusal, LockRefusal::Malformed);
    assert_eq!(refusal.path, "lock.json");
    assert!(
        refusal
            .problem
            .as_deref()
            .is_some_and(|said| said.contains("tag")),
        "{:?}",
        refusal.problem
    );
}

/// The document's shape: not an object, `files` not an array, an entry that is not an object,
/// each refused `Malformed` at `lock.json` saying which.
#[test]
fn a_lock_of_the_wrong_shape_is_malformed_saying_which() {
    let files = vendored();
    let empty = ProtocolLock {
        files: Vec::new(),
        ..committed()
    };
    let text = String::from_utf8(lock::encode(&empty)).expect("UTF-8");
    assert!(text.contains("\"files\": []"));
    for (case, bytes, said) in [
        ("an array", b"[]\n".to_vec(), "not a JSON object"),
        (
            "files an object",
            text.replace("\"files\": []", "\"files\": {}").into_bytes(),
            "`files` is not an array",
        ),
        (
            "an entry a number",
            text.replace("\"files\": []", "\"files\": [\n    1\n  ]")
                .into_bytes(),
            "not an object",
        ),
    ] {
        let refusal = lock_refusal(&files, &bytes, case);
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{case}");
        assert_eq!(refusal.path, "lock.json", "{case}");
        assert!(
            refusal
                .problem
                .as_deref()
                .is_some_and(|problem| problem.contains(said)),
            "{case}: {:?}",
            refusal.problem
        );
    }
}

/// The same lock in other bytes refuses: a byte-order mark, CR LF, no final newline, an extra
/// final newline, an escaped character, a member of an entry given twice, the files out of order.
#[test]
fn the_same_lock_in_other_bytes_is_malformed() {
    let files = vendored();
    let text = String::from_utf8(ENGINEERING_LOCK.to_vec()).expect("UTF-8");
    let mut reordered = text.clone();
    let first = "\"path\": \"eval-gated-change/1.yaml\"";
    let second = "\"path\": \"incident-response/1.yaml\"";
    let eval_sha = "631d759afb0a24401cca6abf7f970170052fab1bd2d29db6a09f65f935afb520";
    let incident_sha = "10a6f9120fe3570c54498e6be054622c2509a9f4d8ab1876aa72fa50b4df1943";
    for (from, to) in [
        (first, "@@1"),
        (second, first),
        ("@@1", second),
        (eval_sha, "@@2"),
        (incident_sha, eval_sha),
        ("@@2", incident_sha),
    ] {
        assert!(reordered.contains(from), "{from}");
        reordered = reordered.replacen(from, to, 1);
    }
    for (case, bytes) in [
        ("byte-order mark", [b"\xef\xbb\xbf".as_slice(), ENGINEERING_LOCK].concat()),
        ("CR LF", text.replace('\n', "\r\n").into_bytes()),
        (
            "no final newline",
            ENGINEERING_LOCK[..ENGINEERING_LOCK.len() - 1].to_vec(),
        ),
        ("two final newlines", [ENGINEERING_LOCK, b"\n"].concat()),
        (
            "an escaped character",
            text.replacen("\"0.16.0\"", "\"\\u0030.16.0\"", 1).into_bytes(),
        ),
        (
            "an entry member given twice",
            text.replacen(
                &format!("\"sha256\": \"{eval_sha}\""),
                &format!("\"sha256\": \"{incident_sha}\",\n      \"sha256\": \"{eval_sha}\""),
                1,
            )
            .into_bytes(),
        ),
        ("files out of order", reordered.into_bytes()),
    ] {
        assert_ne!(bytes, ENGINEERING_LOCK, "{case}: the case changes the bytes");
        let refusal = lock_refusal(&files, &bytes, case);
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{case}");
        assert_eq!(refusal.path, "lock.json", "{case}");
    }
}

/// A locked entry with no file is refused `Missing` before any locked file is parsed: a locked
/// file Canon refuses does not answer for the missing one.
#[test]
fn a_missing_file_is_refused_before_any_definition_is_parsed() {
    let mut files = vendored();
    files.retain(|(path, _)| path != "support-triage/1.yaml");
    files.insert(0, ("aaa/1.yaml".to_owned(), b"garbage".to_vec()));
    let mut locked = committed();
    locked.files.push(lock::locked_file("aaa/1.yaml", b"garbage"));
    let refusal = lock_refusal(&files, &lock::encode(&locked), "missing and garbage");
    assert_eq!(refusal.refusal, LockRefusal::Missing);
    assert_eq!(refusal.path, "support-triage/1.yaml");
}

/// A file set that names one path twice builds no catalog: with a changed second copy the lock
/// refuses it, with an equal second copy the catalog does.
#[test]
fn a_path_given_twice_in_the_files_builds_no_catalog() {
    let mut changed = vendored();
    let mut copy = changed
        .iter()
        .find(|(path, _)| path == SOFTWARE)
        .expect("vendored")
        .clone();
    copy.1[0] ^= 0x20;
    changed.push(copy);
    let refusal = lock_refusal(&changed, ENGINEERING_LOCK, "changed second copy");
    assert_eq!(refusal.refusal, LockRefusal::DigestMismatch);
    assert_eq!(refusal.path, SOFTWARE);

    let mut equal = vendored();
    let copy = equal
        .iter()
        .find(|(path, _)| path == SOFTWARE)
        .expect("vendored")
        .clone();
    equal.push(copy);
    match ProtocolCatalog::from_vendored(&borrowed(&equal), ENGINEERING_LOCK) {
        Err(VendoredError::Definition { path, .. }) => assert_eq!(path, SOFTWARE),
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(catalog) => panic!("built a catalog of {} entries", catalog.iter().count()),
    }
}

/// A path the lock names in another case, or a file named `lock.json` handed as a file, is
/// `Unlocked`.
#[test]
fn a_near_miss_path_is_unlocked() {
    for path in ["Software-change/1.yaml", "./software-change/1.yaml", "lock.json"] {
        let mut files = vendored();
        let bytes = files
            .iter()
            .find(|(file, _)| file == SOFTWARE)
            .expect("vendored")
            .1
            .clone();
        files.push((path.to_owned(), bytes));
        let refusal = lock_refusal(&files, ENGINEERING_LOCK, path);
        assert_eq!(refusal.refusal, LockRefusal::Unlocked, "{path}");
        assert_eq!(refusal.path, path);
    }
}

/// A locked file of zero bytes is a `Definition` refusal at its path, printed on one line, and no
/// panic.
#[test]
fn a_locked_file_of_zero_bytes_is_a_definition_refusal() {
    let mut files = vendored();
    files.push(("zero/1.yaml".to_owned(), Vec::new()));
    files.sort();
    let mut locked = committed();
    locked.files.push(lock::locked_file("zero/1.yaml", b""));
    match ProtocolCatalog::from_vendored(&borrowed(&files), &lock::encode(&locked)) {
        Err(error @ VendoredError::Definition { .. }) => {
            let VendoredError::Definition { path, .. } = &error else {
                unreachable!()
            };
            assert_eq!(path, "zero/1.yaml");
            let line = error.to_string();
            assert!(!line.contains('\n'), "{line}");
            assert!(line.starts_with("engineering protocol zero/1.yaml: "), "{line}");
        }
        Err(other) => panic!("refused otherwise: {other}"),
        Ok(_) => panic!("built a catalog"),
    }
}

/// Every regular file below `directory`, relative to `root` with `/`, and its bytes.
fn on_disk(root: &Path, directory: &Path, found: &mut Vec<(String, Vec<u8>)>) {
    for entry in std::fs::read_dir(directory).expect("readable") {
        let entry = entry.expect("an entry");
        let path = entry.path();
        let kind = entry.file_type().expect("a type");
        assert!(!kind.is_symlink(), "{} is a link", path.display());
        if kind.is_dir() {
            on_disk(root, &path, found);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("below")
                .components()
                .map(|part| part.as_os_str().to_str().expect("UTF-8").to_owned())
                .collect::<Vec<_>>()
                .join("/");
            found.push((relative, std::fs::read(&path).expect("readable")));
        }
    }
}

/// The embedded set is the directory as it is now, every file at any depth other than the
/// top-level `lock.json`, sorted by path as bytes, and the embedded lock is `lock.json`: a build
/// that did not re-run its script after a file was added, removed or changed fails here.
#[test]
fn the_embedded_files_are_the_directory_on_disk() {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocols/engineering");
    let mut found = Vec::new();
    on_disk(&root, &root, &mut found);
    let lock_at = found
        .iter()
        .position(|(path, _)| path == "lock.json")
        .expect("lock.json on disk");
    let (_, lock_bytes) = found.remove(lock_at);
    found.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
    assert_eq!(lock_bytes, ENGINEERING_LOCK);
    let embedded: Vec<(String, Vec<u8>)> = vendored();
    assert_eq!(
        embedded.iter().map(|(path, _)| path).collect::<Vec<_>>(),
        found.iter().map(|(path, _)| path).collect::<Vec<_>>()
    );
    assert_eq!(embedded, found);
}
