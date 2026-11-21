//! story:engineering-protocol-lock: `ProtocolCatalog::engineering` is the vendored load of
//! `protocols/engineering/`, every file held to `protocols/engineering/lock.json` before any
//! definition is admitted.
//!
//! The vendored files are byte-equal to the registry of the pinned `b10x-canon-engineering`, so a
//! comparison of bytes or digests alone passes on a registry load too. What tells the vendored
//! load apart is that the refusals below are observed through `from_vendored`, the function
//! `engineering()` is, and that a lock with another tag moves every entry's revision.

use std::collections::BTreeSet;

use loom_protocols::lock::{self, LockError, LockRefusal, LockedFile, ProtocolLock};
use loom_protocols::{
    ENGINEERING_FILES, ENGINEERING_LOCK, ProtocolCatalog, SYSTEM_QUERY_YAML, SourceKind,
    VendoredError,
};
use sha2::{Digest, Sha256};

/// The ten files of engineering-protocols `0.16.0` and their sha256, as released (the same at
/// `0.15.2`): `git show 0.16.0:protocols/<path> | sha256sum` in engineering-protocols.
const RELEASED: [(&str, &str); 10] = [
    (
        "eval-gated-change/1.yaml",
        "631d759afb0a24401cca6abf7f970170052fab1bd2d29db6a09f65f935afb520",
    ),
    (
        "incident-response/1.yaml",
        "10a6f9120fe3570c54498e6be054622c2509a9f4d8ab1876aa72fa50b4df1943",
    ),
    (
        "loop-improvement/1.yaml",
        "04ba56c8fe24cc29f9deaa209697ec6a33f1c34c4e8fcaa8c4d0d3bbd198389f",
    ),
    (
        "ml-experiment/1.yaml",
        "7b0ac3b9f9d079fb729a84987a31902867c5869e137957e4af9d3be4c95f98da",
    ),
    (
        "protocol-adopt/1.yaml",
        "69d1304b7d6ac89f74f7d16cb9bc2d38197b9871182b5ba63977ac1dc4a28929",
    ),
    (
        "self-improvement/1.yaml",
        "7947a8ebb461b56b608c1d15821b126ed353d12a20eaab8a30ebc2f5f71d9033",
    ),
    (
        "software-change/1.yaml",
        "8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046",
    ),
    (
        "story-delivery/1.yaml",
        "f24e9c1f98c0e4c6ba4a88ad82696c68ac403655378ebc5af51c3c8a775f8e39",
    ),
    (
        "story-delivery/2.yaml",
        "37e129039e390b67c522ed35b95b19db534bcc4f76d579af3db1f8bce933ba13",
    ),
    (
        "support-triage/1.yaml",
        "d970d928e6299aa400566b0c4e2fb928479c42f7c96a82e5b78feab41277c67a",
    ),
];

/// `<name>/<major>.yaml` as the registration `<name>@<major>`.
fn registration(path: &str) -> String {
    let (name, file) = path.split_once('/').expect("<name>/<major>.yaml");
    format!("{name}@{}", file.strip_suffix(".yaml").expect(".yaml"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// An owned copy of `ENGINEERING_FILES`, to change.
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

fn committed_lock() -> ProtocolLock {
    lock::decode(ENGINEERING_LOCK).expect("the committed lock decodes")
}

fn committed_text() -> String {
    String::from_utf8(ENGINEERING_LOCK.to_vec()).expect("the committed lock is UTF-8")
}

/// The lock refusal `from_vendored` answers `files` and `lock` with; a catalog or another error
/// fails the case.
fn lock_refusal(files: &[(String, Vec<u8>)], lock: &[u8]) -> LockError {
    match ProtocolCatalog::from_vendored(&borrowed(files), lock) {
        Err(VendoredError::Lock(error)) => error,
        Err(other) => panic!("refused, but not by the lock: {other}"),
        Ok(catalog) => panic!("built a catalog of {} entries", catalog.iter().count()),
    }
}

#[test]
fn engineering_is_the_vendored_load() {
    let engineering = ProtocolCatalog::engineering().expect("the engineering catalog");
    let vendored = ProtocolCatalog::from_vendored(ENGINEERING_FILES, ENGINEERING_LOCK)
        .expect("the vendored catalog");
    let definitions = |catalog: &ProtocolCatalog| {
        catalog
            .iter()
            .map(|entry| entry.definition.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(definitions(&engineering), definitions(&vendored));
    assert_eq!(engineering.iter().count(), ENGINEERING_FILES.len());
    for entry in engineering.iter() {
        let path = entry
            .definition
            .source
            .path
            .strip_prefix("protocols/")
            .expect("protocols/<name>/<major>.yaml");
        let (_, bytes) = ENGINEERING_FILES
            .iter()
            .find(|(file, _)| *file == path)
            .unwrap_or_else(|| panic!("{} has no vendored file {path}", entry.name()));
        assert_eq!(entry.definition.yaml.as_bytes(), *bytes, "{path}");
    }
}

#[test]
fn the_engineering_digests_are_the_released_ones() {
    let catalog = ProtocolCatalog::engineering().expect("the engineering catalog");
    let registrations: BTreeSet<String> = catalog
        .iter()
        .map(|entry| entry.name().to_owned())
        .collect();
    let released: BTreeSet<String> = RELEASED
        .iter()
        .map(|(path, _)| registration(path))
        .collect();
    assert_eq!(registrations, released);
    for (path, digest) in RELEASED {
        let entry = catalog.get(&registration(path)).expect("registered");
        assert_eq!(entry.definition.sha256, digest, "{path}");
    }
}

#[test]
fn every_engineering_entry_names_its_vendored_source() {
    let catalog = ProtocolCatalog::engineering().expect("the engineering catalog");
    assert_eq!(catalog.iter().count(), 10);
    for entry in catalog.iter() {
        let source = &entry.definition.source;
        let (name, major) = entry.name().split_once('@').expect("name@major");
        assert_eq!(source.kind, SourceKind::Engineering, "{}", entry.name());
        assert_eq!(source.location, "engineering-protocols", "{}", entry.name());
        assert_eq!(source.revision, "0.16.0", "{}", entry.name());
        assert_eq!(source.path, format!("protocols/{name}/{major}.yaml"));
    }
}

#[test]
fn the_revision_is_the_locks_tag() {
    let mut retagged = committed_lock();
    retagged.tag = "9.9.9".into();
    let catalog = ProtocolCatalog::from_vendored(ENGINEERING_FILES, &lock::encode(&retagged))
        .expect("a lock with another tag still holds the files");
    assert_eq!(catalog.iter().count(), 10);
    for entry in catalog.iter() {
        assert_eq!(
            entry.definition.source.revision,
            "9.9.9",
            "{}",
            entry.name()
        );
    }
}

/// `Deliver` becomes `deliver` in `software-change/1.yaml`'s description: one byte, and a
/// document Canon still admits, so only the lock can refuse it.
fn one_byte_changed() -> (Vec<(String, Vec<u8>)>, String) {
    let mut files = vendored();
    let (_, bytes) = files
        .iter_mut()
        .find(|(path, _)| path == "software-change/1.yaml")
        .expect("software-change/1.yaml is vendored");
    let at = bytes
        .windows(b"Deliver a change".len())
        .position(|window| window == b"Deliver a change")
        .expect("the description");
    bytes[at] = b'd';
    let changed = String::from_utf8(bytes.clone()).expect("UTF-8");
    ProtocolCatalog::default()
        .add_yaml(
            "software-change@1",
            &changed,
            loom_protocols::ProtocolSource {
                kind: SourceKind::Memory,
                location: "test".into(),
                revision: String::new(),
                path: String::new(),
            },
        )
        .expect("Canon admits the changed document");
    let found = sha256(changed.as_bytes());
    (files, found)
}

#[test]
fn a_changed_byte_is_refused_at_load() {
    let (files, found) = one_byte_changed();
    let refusal = lock_refusal(&files, ENGINEERING_LOCK);
    assert_eq!(refusal.refusal, LockRefusal::DigestMismatch);
    assert_eq!(refusal.path, "software-change/1.yaml");
    assert_eq!(
        refusal.digests,
        Some((
            "8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046".to_owned(),
            found
        ))
    );
}

#[test]
fn an_unlocked_file_is_refused_at_load() {
    for (path, bytes) in [
        ("extra/1.yaml", SYSTEM_QUERY_YAML.as_bytes()),
        ("README.md", b"not a protocol".as_slice()),
        ("aaa/1.yaml", SYSTEM_QUERY_YAML.as_bytes()),
    ] {
        let mut files = vendored();
        files.push((path.to_owned(), bytes.to_vec()));
        files.sort();
        let refusal = lock_refusal(&files, ENGINEERING_LOCK);
        assert_eq!(refusal.refusal, LockRefusal::Unlocked, "{path}");
        assert_eq!(refusal.path, path);
    }
}

#[test]
fn a_missing_locked_file_is_refused_at_load() {
    let mut files = vendored();
    files.retain(|(path, _)| path != "support-triage/1.yaml");
    assert_eq!(files.len(), 9);
    let refusal = lock_refusal(&files, ENGINEERING_LOCK);
    assert_eq!(refusal.refusal, LockRefusal::Missing);
    assert_eq!(refusal.path, "support-triage/1.yaml");
}

/// Each malformed lock, the path its refusal names, and a part of what it says is wrong.
fn malformed_locks() -> Vec<(&'static str, Vec<u8>, &'static str, &'static str)> {
    let text = committed_text();
    let replaced = |from: &str, to: &str| {
        assert!(text.contains(from), "the committed lock holds {from}");
        text.replacen(from, to, 1).into_bytes()
    };
    let commit_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("\"commit\""))
        .expect("a commit member")
        .to_owned();
    let mut twice = committed_lock();
    let software = twice
        .files
        .iter()
        .find(|file| file.path == "software-change/1.yaml")
        .expect("software-change/1.yaml is locked")
        .clone();
    twice.files.push(software);
    vec![
        (
            "bytes that are not JSON",
            b"not json".to_vec(),
            "lock.json",
            "not JSON",
        ),
        ("no bytes", Vec::new(), "lock.json", "not JSON"),
        (
            "another format",
            replaced("\"loom.protocol-lock/1\"", "\"loom.protocol-lock/2\""),
            "lock.json",
            "format",
        ),
        (
            "another repository",
            replaced(
                "\"https://github.com/beyond10x/engineering-protocols\"",
                "\"https://github.com/elsewhere/engineering-protocols\"",
            ),
            "lock.json",
            "repository",
        ),
        (
            "an undeclared top-level member",
            replaced("  \"tag\": ", "  \"extra\": \"x\",\n  \"tag\": "),
            "lock.json",
            "undeclared member `extra`",
        ),
        (
            "a missing top-level member",
            replaced(&format!("{commit_line}\n"), ""),
            "lock.json",
            "missing member `commit`",
        ),
        (
            "a path that is not <name>/<major>.yaml",
            replaced(
                "\"path\": \"software-change/1.yaml\"",
                "\"path\": \"software-change.yaml\"",
            ),
            "software-change.yaml",
            "<name>/<major>.yaml",
        ),
        (
            "a path given twice",
            lock::encode(&twice),
            "software-change/1.yaml",
            "twice",
        ),
        (
            "an undeclared member of an entry",
            replaced(
                "\"path\": \"software-change/1.yaml\",",
                "\"path\": \"software-change/1.yaml\",\n      \"size\": 1,",
            ),
            "software-change/1.yaml",
            "undeclared member `size`",
        ),
        (
            "a missing member of an entry",
            replaced(
                "\"path\": \"software-change/1.yaml\",\n      \"sha256\": \"8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046\"",
                "\"path\": \"software-change/1.yaml\"",
            ),
            "software-change/1.yaml",
            "missing member `sha256`",
        ),
        (
            "a sha256 that is not 64 lowercase hex digits",
            replaced(
                "8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046",
                "8367FDAFBA358845CE842C87298ECFB80970F4B72BA3466EC8402BC6272D2046",
            ),
            "software-change/1.yaml",
            "`sha256` is not 64 lowercase hex digits",
        ),
        (
            "an entry without a path",
            replaced("\"path\": \"software-change/1.yaml\",\n      ", ""),
            "lock.json",
            "path",
        ),
        (
            "a member given twice",
            replaced("  \"tag\": ", "  \"tag\": \"9.9.9\",\n  \"tag\": "),
            "lock.json",
            "encoder",
        ),
        (
            "bytes that are not the encoder's",
            replaced("  \"tag\": ", "  \"tag\":  "),
            "lock.json",
            "encoder",
        ),
    ]
}

#[test]
fn a_malformed_lock_is_refused_at_load() {
    let files = vendored();
    for (case, bytes, path, problem) in malformed_locks() {
        let refusal = lock_refusal(&files, &bytes);
        assert_eq!(refusal.refusal, LockRefusal::Malformed, "{case}");
        assert_eq!(refusal.path, path, "{case}");
        let said = refusal.problem.as_deref().unwrap_or_default();
        assert!(
            !said.is_empty(),
            "{case}: a malformed lock says what is wrong"
        );
        assert!(said.contains(problem), "{case}: {said}");
        assert_eq!(refusal.digests, None, "{case}");
    }
}

/// Every lock entry is checked before any file is parsed: a locked file Canon refuses, sorted
/// first, does not answer for a later file whose digest is wrong.
#[test]
fn every_digest_is_checked_before_any_definition_is_parsed() {
    let (mut files, _) = one_byte_changed();
    files.insert(0, ("aaa/1.yaml".to_owned(), b"garbage".to_vec()));
    let mut locked = committed_lock();
    locked
        .files
        .push(lock::locked_file("aaa/1.yaml", b"garbage"));
    let refusal = lock_refusal(&files, &lock::encode(&locked));
    assert_eq!(refusal.refusal, LockRefusal::DigestMismatch);
    assert_eq!(refusal.path, "software-change/1.yaml");
}

#[test]
fn a_locked_file_that_is_no_definition_is_a_definition_error() {
    for (path, bytes, problem) in [
        ("aaa/1.yaml", b"garbage".as_slice(), "aaa@1"),
        ("bbb/1.yaml", b"\xff\xfe".as_slice(), "UTF-8"),
    ] {
        let mut files = vendored();
        files.push((path.to_owned(), bytes.to_vec()));
        files.sort();
        let mut locked = committed_lock();
        locked.files.push(lock::locked_file(path, bytes));
        match ProtocolCatalog::from_vendored(&borrowed(&files), &lock::encode(&locked)) {
            Err(error @ VendoredError::Definition { .. }) => {
                let VendoredError::Definition {
                    path: refused,
                    problem: said,
                } = &error
                else {
                    unreachable!()
                };
                assert_eq!(refused, path);
                assert!(said.contains(problem), "{path}: {said}");
                assert!(
                    error
                        .to_string()
                        .starts_with(&format!("engineering protocol {path}: ")),
                    "{error}"
                );
            }
            Err(other) => panic!("{path}: {other}"),
            Ok(_) => panic!("{path}: built a catalog"),
        }
    }
}

#[test]
fn the_refusal_text_names_the_variant_and_the_path() {
    let print = |files: &[(String, Vec<u8>)], lock: &[u8]| {
        ProtocolCatalog::from_vendored(&borrowed(files), lock)
            .expect_err("refused")
            .to_string()
    };
    let (changed, found) = one_byte_changed();
    assert_eq!(
        print(&changed, ENGINEERING_LOCK),
        format!(
            "engineering protocol lock refused: DigestMismatch software-change/1.yaml (locked \
             8367fdafba358845ce842c87298ecfb80970f4b72ba3466ec8402bc6272d2046, found {found})"
        )
    );
    let mut unlocked = vendored();
    unlocked.push((
        "extra/1.yaml".to_owned(),
        SYSTEM_QUERY_YAML.as_bytes().to_vec(),
    ));
    assert_eq!(
        print(&unlocked, ENGINEERING_LOCK),
        "engineering protocol lock refused: Unlocked extra/1.yaml"
    );
    let mut missing = vendored();
    missing.retain(|(path, _)| path != "support-triage/1.yaml");
    assert_eq!(
        print(&missing, ENGINEERING_LOCK),
        "engineering protocol lock refused: Missing support-triage/1.yaml"
    );
    let malformed = print(&vendored(), b"not json");
    let (head, what) = malformed
        .split_once(": ")
        .and_then(|(_, rest)| rest.split_once(": "))
        .map(|(head, what)| (head.to_owned(), what.to_owned()))
        .expect("<prefix>: <variant> <path>: <what>");
    assert!(
        malformed.starts_with("engineering protocol lock refused: Malformed lock.json: "),
        "{malformed}"
    );
    assert_eq!(head, "Malformed lock.json");
    assert!(!what.is_empty(), "{malformed}");
    let engineering: fn() -> Result<ProtocolCatalog, String> = ProtocolCatalog::engineering;
    assert!(engineering().is_ok());
}

#[test]
fn the_committed_lock_names_the_release() {
    let locked = committed_lock();
    assert_eq!(
        lock::REPOSITORY,
        "https://github.com/beyond10x/engineering-protocols"
    );
    assert_eq!(locked.format, lock::FORMAT);
    assert_eq!(lock::FORMAT, "loom.protocol-lock/1");
    assert_eq!(locked.repository, lock::REPOSITORY);
    assert_eq!(locked.tag, "0.16.0");
    assert_eq!(locked.commit, "70e6eff48963c9ea178d515ae33e645cf95c9773");
    let files: Vec<LockedFile> = RELEASED
        .iter()
        .map(|(path, sha256)| LockedFile {
            path: (*path).to_owned(),
            sha256: (*sha256).to_owned(),
        })
        .collect();
    assert_eq!(locked.files, files);
}

#[test]
fn the_committed_lock_is_the_encoders_bytes() {
    let encoded = lock::encode(&committed_lock());
    assert_eq!(
        String::from_utf8_lossy(&encoded),
        String::from_utf8_lossy(ENGINEERING_LOCK)
    );
    assert_eq!(encoded, ENGINEERING_LOCK);
    assert!(ENGINEERING_LOCK.ends_with(b"}\n") && !ENGINEERING_LOCK.ends_with(b"\n\n"));
}

/// `encode` writes the members in their declared order and the files sorted by path as bytes,
/// whatever order it is given them in.
#[test]
fn encode_orders_members_and_files() {
    let mut reversed = committed_lock();
    reversed.files.reverse();
    assert_eq!(lock::encode(&reversed), ENGINEERING_LOCK);
    let empty = ProtocolLock {
        format: lock::FORMAT.into(),
        repository: lock::REPOSITORY.into(),
        tag: "1.0.0".into(),
        commit: "0".repeat(40),
        files: Vec::new(),
    };
    let text = String::from_utf8(lock::encode(&empty)).expect("UTF-8");
    assert_eq!(
        text,
        format!(
            "{{\n  \"format\": \"loom.protocol-lock/1\",\n  \"repository\": \
             \"https://github.com/beyond10x/engineering-protocols\",\n  \"tag\": \"1.0.0\",\n  \
             \"commit\": \"{}\",\n  \"files\": []\n}}\n",
            "0".repeat(40)
        )
    );
    assert_eq!(lock::decode(text.as_bytes()).expect("decodes"), empty);
}

#[test]
fn locked_file_is_the_sha256_of_the_bytes() {
    for (path, bytes) in ENGINEERING_FILES {
        assert_eq!(
            lock::locked_file(path, bytes),
            LockedFile {
                path: (*path).to_owned(),
                sha256: sha256(bytes),
            }
        );
    }
}

#[test]
fn every_engineering_entry_matches_its_lock_entry() {
    let catalog = ProtocolCatalog::engineering().expect("the engineering catalog");
    let locked = committed_lock();
    assert_eq!(catalog.iter().count(), locked.files.len());
    for file in &locked.files {
        let entry = catalog
            .get(&registration(&file.path))
            .unwrap_or_else(|| panic!("{} is locked and not registered", file.path));
        assert_eq!(entry.definition.sha256, file.sha256, "{}", file.path);
    }
}
