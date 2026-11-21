//! `loom-xtask protocols-vendor --checkout <dir> --tag <tag>` (story:engineering-protocols-vendor-command).
//!
//! Each case builds its own Git repository under `std::env::temp_dir()` with plumbing commands
//! only: no user or system configuration, no hooks, no signing, a fixed author and date. Its
//! `origin` is `git@example.invalid:someone/fork.git`, so a lock that read the remote would say so.
//! The protocol files of `t1` and `t2` are the tree's own vendored files, which Canon accepts; `t3`
//! holds one file that is not UTF-8. The command runs with `--root` at a temporary root, so the
//! repository's own `protocols/engineering/` is only read.

use loom_protocols::{ProtocolCatalog, lock};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const ORIGIN: &str = "git@example.invalid:someone/fork.git";
/// `t3`'s one file: `0xff`, `0xfe`, `0x00` and `0x80` are not UTF-8 text.
const RAW: &[u8] = b"name: raw-bytes\nbytes: \xff\xfe\x00\x80\n";
/// The sha256 of [`RAW`], taken with `sha256sum` outside this test.
const RAW_SHA256: &str = "bca3f387d2f1251245aba8bef169cc5b3a770a45c886f16a2dd71eac53022a9d";
const VOCABULARY: &[u8] = b"terms: []\n";

/// A directory under `std::env::temp_dir()`, removed when dropped.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "loom-protocols-vendor-{}-{}-{name}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("remove a stale fixture directory");
        }
        fs::create_dir_all(&dir).expect("create a fixture directory");
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The tree's own vendored file at `path` below `protocols/engineering/`.
fn vendored(path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocols/engineering")
        .join(path);
    fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
}

/// `git -C <dir> <args>` with no configuration but the repository's, no hooks, no monitor, no
/// signing and a fixed identity and date; standard input is null.
fn git_output(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Loom Test")
        .env("GIT_AUTHOR_EMAIL", "loom-test@example.invalid")
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_NAME", "Loom Test")
        .env("GIT_COMMITTER_EMAIL", "loom-test@example.invalid")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .stdin(Stdio::null())
        .output()
        .expect("run git")
}

/// [`git_output`], which must succeed; its standard output with the trailing newline removed.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_output(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("git prints UTF-8")
        .trim_end()
        .to_owned()
}

/// The Git repository a case reads, with `t1`, `t2` and `t3` and any tag a case adds.
struct Checkout {
    dir: Dir,
    /// Scratch for blob and tree files, outside the repository.
    blobs: Dir,
}

impl Checkout {
    fn path(&self) -> &Path {
        &self.dir.0
    }

    /// Writes `bytes` as a blob; its object id.
    fn blob(&self, bytes: &[u8]) -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let file = self
            .blobs
            .0
            .join(format!("blob-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::write(&file, bytes).expect("write a blob file");
        git(
            self.path(),
            &["hash-object", "-w", file.to_str().expect("UTF-8")],
        )
    }

    /// Writes a tree whose entries are given as (mode, name, object id) exactly, whatever the
    /// names: `git mktree` would refuse a name such as `..`, a crafted tree does not.
    fn crafted_tree(&self, entries: &[(&str, &str, &str)]) -> String {
        let mut bytes = Vec::new();
        for (mode, name, id) in entries {
            bytes.extend_from_slice(format!("{mode} {name}\0").as_bytes());
            bytes.extend(
                (0..id.len()).step_by(2).map(|at| {
                    u8::from_str_radix(&id[at..at + 2], 16).expect("an object id is hex")
                }),
            );
        }
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let file = self
            .blobs
            .0
            .join(format!("tree-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::write(&file, bytes).expect("write a tree file");
        git(
            self.path(),
            &[
                "hash-object",
                "-t",
                "tree",
                "--literally",
                "-w",
                file.to_str().expect("UTF-8"),
            ],
        )
    }

    /// Commits `tree` and tags the commit `tag` (annotated when `annotated`); the commit's id.
    fn tag_tree(&self, tag: &str, tree: &str, annotated: bool) -> String {
        let commit = git(self.path(), &["commit-tree", "-m", tag, tree]);
        if annotated {
            git(self.path(), &["tag", "-a", "-m", tag, tag, &commit]);
        } else {
            git(self.path(), &["tag", tag, &commit]);
        }
        commit
    }

    /// Commits a tree of exactly `entries`, (mode, path, object id), and tags it `tag`.
    fn tag_entries(&self, tag: &str, entries: &[(&str, &str, String)], annotated: bool) -> String {
        git(self.path(), &["read-tree", "--empty"]);
        for (mode, path, id) in entries {
            git(
                self.path(),
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("{mode},{id},{path}"),
                ],
            );
        }
        let tree = git(self.path(), &["write-tree"]);
        self.tag_tree(tag, &tree, annotated)
    }

    /// Commits a tree of exactly `files`, (path, bytes), as regular files, and tags it `tag`.
    fn tag_files(&self, tag: &str, files: &[(&str, &[u8])], annotated: bool) -> String {
        let entries: Vec<(&str, &str, String)> = files
            .iter()
            .map(|(path, bytes)| ("100644", *path, self.blob(bytes)))
            .collect();
        self.tag_entries(tag, &entries, annotated)
    }
}

/// A checkout holding `t1` (annotated), `t2` and `t3`, whose `origin` is [`ORIGIN`].
fn checkout() -> Checkout {
    let checkout = Checkout {
        dir: Dir::new("checkout"),
        blobs: Dir::new("blobs"),
    };
    git(checkout.path(), &["init", "-q", "--template="]);
    git(checkout.path(), &["remote", "add", "origin", ORIGIN]);
    let software_change = vendored("software-change/1.yaml");
    checkout.tag_files(
        "t1",
        &[
            ("protocols/software-change/1.yaml", &software_change),
            (
                "protocols/incident-response/1.yaml",
                &vendored("incident-response/1.yaml"),
            ),
            ("protocols/vocabulary.yaml", VOCABULARY),
            ("README.md", b"engineering protocols\n"),
        ],
        true,
    );
    checkout.tag_files(
        "t2",
        &[
            ("protocols/software-change/1.yaml", &software_change),
            (
                "protocols/support-triage/1.yaml",
                &vendored("support-triage/1.yaml"),
            ),
            ("protocols/vocabulary.yaml", VOCABULARY),
        ],
        false,
    );
    checkout.tag_files("t3", &[("protocols/raw-bytes/1.yaml", RAW)], false);
    checkout
}

/// `loom-xtask --root <root> protocols-vendor --checkout <checkout> --tag <tag>`.
fn vendor(root: &Path, checkout: impl AsRef<OsStr>, tag: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loom-xtask"))
        .arg("--root")
        .arg(root)
        .arg("protocols-vendor")
        .arg("--checkout")
        .arg(checkout)
        .args(["--tag", tag])
        .stdin(Stdio::null())
        .output()
        .expect("run loom-xtask")
}

fn vendored_ok(root: &Path, checkout: &Checkout, tag: &str) {
    let out = vendor(root, checkout.path(), tag);
    assert!(
        out.status.success(),
        "protocols-vendor at {tag}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// What a path under a root is.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}

/// Every path under `root`, relative to it, with what it is; links are not followed.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Entry> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<PathBuf, Entry>) {
        for entry in fs::read_dir(dir).expect("read a directory") {
            let path = entry.expect("a directory entry").path();
            let relative = path.strip_prefix(root).expect("below the root").to_owned();
            let kind = fs::symlink_metadata(&path).expect("stat").file_type();
            if kind.is_symlink() {
                found.insert(relative, Entry::Link(fs::read_link(&path).expect("link")));
            } else if kind.is_dir() {
                found.insert(relative, Entry::Directory);
                walk(root, &path, found);
            } else {
                found.insert(relative, Entry::File(fs::read(&path).expect("read")));
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

/// The files under `<root>/protocols/engineering`, relative to it with `/` between parts.
fn engineering_files(root: &Path) -> BTreeSet<String> {
    let dir = root.join("protocols/engineering");
    snapshot(&dir)
        .into_iter()
        .filter(|(_, entry)| matches!(entry, Entry::File(_)))
        .map(|(path, _)| path.to_str().expect("UTF-8").replace('\\', "/"))
        .collect()
}

fn read(root: &Path, path: &str) -> Vec<u8> {
    let file = root.join("protocols/engineering").join(path);
    fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
}

/// `from_vendored` over every file the command wrote and its `lock.json`: each registration with
/// its revision.
fn catalog(root: &Path) -> BTreeMap<String, String> {
    let lock_bytes = read(root, "lock.json");
    let files: Vec<(String, Vec<u8>)> = engineering_files(root)
        .into_iter()
        .filter(|path| path != "lock.json")
        .map(|path| {
            let bytes = read(root, &path);
            (path, bytes)
        })
        .collect();
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice()))
        .collect();
    let catalog = ProtocolCatalog::from_vendored(&borrowed, &lock_bytes)
        .unwrap_or_else(|e| panic!("the written files load: {e}"));
    catalog
        .iter()
        .map(|entry| {
            (
                entry.name().to_owned(),
                entry.definition.source.revision.clone(),
            )
        })
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// Runs `refused` on an empty root and on a root a `t1` run wrote; each run must fail, name
/// `named` on standard error, and leave every path and byte under the root as it was.
fn assert_refused_changing_nothing(
    checkout: &Checkout,
    named: &str,
    refused: impl Fn(&Path) -> Output,
) {
    for after_t1 in [false, true] {
        let root = Dir::new("root");
        if after_t1 {
            vendored_ok(&root.0, checkout, "t1");
        }
        let before = snapshot(&root.0);
        let out = refused(&root.0);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "refused (after t1: {after_t1}), but exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(
            stderr.contains(named),
            "the refusal names {named:?} (after t1: {after_t1}): {stderr}"
        );
        assert_eq!(
            snapshot(&root.0),
            before,
            "a refusal changes nothing under the root (after t1: {after_t1}): {stderr}"
        );
    }
}

#[test]
fn at_t1_the_root_holds_exactly_the_protocol_files_byte_for_byte_and_the_lock() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    assert_eq!(
        engineering_files(&root.0),
        set(&[
            "incident-response/1.yaml",
            "lock.json",
            "software-change/1.yaml"
        ]),
        "vocabulary.yaml and files outside protocols/ are not copied"
    );
    for path in ["software-change/1.yaml", "incident-response/1.yaml"] {
        assert_eq!(read(&root.0, path), vendored(path), "{path} byte for byte");
    }
    let names = |dir: &Path| -> BTreeSet<String> {
        fs::read_dir(dir)
            .expect("read a directory")
            .map(|entry| {
                entry
                    .expect("a directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    };
    assert_eq!(
        names(&root.0),
        set(&["protocols"]),
        "nothing beside protocols/"
    );
    assert_eq!(
        names(&root.0.join("protocols")),
        set(&["engineering"]),
        "nothing beside protocols/engineering/, no staging left behind"
    );
}

#[test]
fn at_t1_from_vendored_builds_exactly_the_two_protocols_at_revision_t1() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    assert_eq!(
        catalog(&root.0),
        BTreeMap::from([
            ("incident-response@1".to_owned(), "t1".to_owned()),
            ("software-change@1".to_owned(), "t1".to_owned()),
        ])
    );
}

#[test]
fn at_t1_the_lock_names_the_constant_repository_and_the_tags_commit_not_the_remote() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    let decoded = lock::decode(&read(&root.0, "lock.json")).expect("the lock decodes");
    assert_eq!(
        git(checkout.path(), &["remote", "get-url", "origin"]),
        ORIGIN
    );
    assert_eq!(decoded.repository, lock::REPOSITORY);
    assert_eq!(decoded.tag, "t1");
    let commit = git(checkout.path(), &["rev-parse", "t1^{commit}"]);
    assert_ne!(
        commit,
        git(checkout.path(), &["rev-parse", "t1"]),
        "t1 is annotated, so its commit is not the tag object"
    );
    assert_eq!(decoded.commit, commit);
}

#[test]
fn at_t1_the_lock_is_the_encoders_bytes_for_what_it_decodes_to() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    let bytes = read(&root.0, "lock.json");
    let decoded = lock::decode(&bytes).expect("the lock decodes");
    assert_eq!(lock::encode(&decoded), bytes);
}

#[test]
fn a_second_run_at_t1_leaves_every_byte_under_the_root_unchanged() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    let first = snapshot(&root.0);
    vendored_ok(&root.0, &checkout, "t1");
    assert_eq!(snapshot(&root.0), first);
}

#[test]
fn a_t2_run_after_t1_removes_the_file_the_tag_no_longer_holds_and_its_directory() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    assert!(
        root.0
            .join("protocols/engineering/incident-response")
            .is_dir()
    );
    vendored_ok(&root.0, &checkout, "t2");
    let engineering = root.0.join("protocols/engineering");
    assert!(!engineering.join("incident-response/1.yaml").exists());
    assert!(!engineering.join("incident-response").exists());
}

#[test]
fn a_t2_run_after_t1_leaves_exactly_the_t2_files_which_load_at_revision_t2() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t1");
    vendored_ok(&root.0, &checkout, "t2");
    assert_eq!(
        engineering_files(&root.0),
        set(&[
            "lock.json",
            "software-change/1.yaml",
            "support-triage/1.yaml"
        ])
    );
    assert_eq!(
        read(&root.0, "support-triage/1.yaml"),
        vendored("support-triage/1.yaml")
    );
    assert_eq!(
        catalog(&root.0),
        BTreeMap::from([
            ("software-change@1".to_owned(), "t2".to_owned()),
            ("support-triage@1".to_owned(), "t2".to_owned()),
        ])
    );
}

#[test]
fn at_t3_bytes_that_are_not_utf8_are_written_unchanged_and_locked_by_their_sha256() {
    let checkout = checkout();
    let root = Dir::new("root");
    vendored_ok(&root.0, &checkout, "t3");
    assert_eq!(
        engineering_files(&root.0),
        set(&["lock.json", "raw-bytes/1.yaml"])
    );
    let written = read(&root.0, "raw-bytes/1.yaml");
    assert_eq!(written, RAW);
    assert!(written.contains(&0xff) && written.contains(&0xfe));
    let decoded = lock::decode(&read(&root.0, "lock.json")).expect("the lock decodes");
    assert_eq!(decoded.files.len(), 1);
    assert_eq!(decoded.files[0].path, "raw-bytes/1.yaml");
    assert_eq!(decoded.files[0].sha256, RAW_SHA256);
}

#[test]
fn a_tag_that_names_a_tree_is_refused_as_naming_no_commit_not_as_missing() {
    let checkout = checkout();
    let tree = git(checkout.path(), &["rev-parse", "t1^{tree}"]);
    git(checkout.path(), &["tag", "tree", &tree]);
    assert_refused_changing_nothing(&checkout, "tag tree: names no commit", |root| {
        vendor(root, checkout.path(), "tree")
    });
    let out = vendor(Dir::new("root").0.as_path(), checkout.path(), "tree");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("not in the checkout"), "{stderr}");
}

#[test]
fn a_tag_the_checkout_does_not_hold_is_refused_naming_it_and_is_not_fetched() {
    let checkout = checkout();
    // A local remote that does hold the tag: a command that fetched would find it there.
    let upstream = Checkout {
        dir: Dir::new("upstream"),
        blobs: Dir::new("upstream-blobs"),
    };
    git(upstream.path(), &["init", "-q", "--template="]);
    upstream.tag_files(
        "t9",
        &[(
            "protocols/software-change/1.yaml",
            &vendored("software-change/1.yaml"),
        )],
        false,
    );
    git(
        checkout.path(),
        &[
            "remote",
            "add",
            "upstream",
            upstream.path().to_str().expect("UTF-8"),
        ],
    );
    assert_refused_changing_nothing(&checkout, "t9", |root| vendor(root, checkout.path(), "t9"));
    assert!(
        !git_output(
            checkout.path(),
            &["rev-parse", "-q", "--verify", "refs/tags/t9"]
        )
        .status
        .success(),
        "the command fetched t9 into the checkout"
    );
}

#[test]
fn a_revision_expression_given_as_the_tag_is_refused_naming_it() {
    let checkout = checkout();
    assert!(
        git_output(
            checkout.path(),
            &["rev-parse", "--verify", "refs/tags/t1^0"]
        )
        .status
        .success(),
        "`t1^0` resolves to t1's commit, so only the tag check refuses it"
    );
    for expression in ["t1^0", "t1~0", "t1@{0}"] {
        assert_refused_changing_nothing(&checkout, expression, |root| {
            vendor(root, checkout.path(), expression)
        });
    }
}

#[test]
fn a_checkout_that_is_a_url_is_refused_naming_the_argument() {
    let checkout = checkout();
    for url in [
        "https://github.com/beyond10x/engineering-protocols",
        "git@example.invalid:someone/fork.git",
        "file:///nonexistent/engineering-protocols",
    ] {
        assert_refused_changing_nothing(&checkout, url, |root| vendor(root, url, "t1"));
    }
}

#[test]
fn a_path_that_is_not_name_major_yaml_is_refused_naming_it() {
    let checkout = checkout();
    checkout.tag_files(
        "one",
        &[
            (
                "protocols/incident-response/1.yaml",
                &vendored("incident-response/1.yaml"),
            ),
            (
                "protocols/software-change/1.yaml",
                &vendored("software-change/1.yaml"),
            ),
            (
                "protocols/software-change/one.yaml",
                &vendored("software-change/1.yaml"),
            ),
        ],
        false,
    );
    assert_refused_changing_nothing(&checkout, "protocols/software-change/one.yaml", |root| {
        vendor(root, checkout.path(), "one")
    });
}

#[test]
fn a_link_or_a_submodule_in_the_tree_is_refused_naming_it() {
    let checkout = checkout();
    let target = checkout.blob(b"../../../outside.yaml");
    let commit = git(checkout.path(), &["rev-parse", "t2^{commit}"]);
    let software_change = checkout.blob(&vendored("software-change/1.yaml"));
    checkout.tag_entries(
        "link",
        &[
            (
                "100644",
                "protocols/software-change/1.yaml",
                software_change.clone(),
            ),
            ("120000", "protocols/incident-response/1.yaml", target),
        ],
        false,
    );
    checkout.tag_entries(
        "submodule",
        &[
            (
                "100644",
                "protocols/software-change/1.yaml",
                software_change,
            ),
            ("160000", "protocols/incident-response", commit),
        ],
        false,
    );
    for tag in ["link", "submodule"] {
        assert_refused_changing_nothing(&checkout, "protocols/incident-response", |root| {
            vendor(root, checkout.path(), tag)
        });
    }
}

/// A tag whose `protocols` tree holds `software-change/1.yaml` and, under the crafted entry
/// `name`, a directory `x` with `1.yaml`.
fn crafted_tag(checkout: &Checkout, tag: &str, name: &str) {
    let blob = checkout.blob(&vendored("software-change/1.yaml"));
    let leaf = checkout.crafted_tree(&[("100644", "1.yaml", blob.as_str())]);
    let x = checkout.crafted_tree(&[("40000", "x", leaf.as_str())]);
    let protocols = checkout.crafted_tree(&[
        ("40000", name, x.as_str()),
        ("40000", "software-change", leaf.as_str()),
    ]);
    let top = checkout.crafted_tree(&[("40000", "protocols", protocols.as_str())]);
    checkout.tag_tree(tag, &top, false);
}

#[test]
fn a_path_through_dot_dot_is_refused_and_nothing_leaves_the_vendored_directory() {
    let checkout = checkout();
    crafted_tag(&checkout, "dotdot", "..");
    let listed = git(checkout.path(), &["ls-tree", "-r", "--name-only", "dotdot"]);
    assert!(listed.contains("protocols/../x/1.yaml"), "{listed}");
    assert_refused_changing_nothing(&checkout, "protocols/../x/1.yaml", |root| {
        vendor(root, checkout.path(), "dotdot")
    });
}

#[test]
fn an_absolute_path_is_refused_and_nothing_is_written_there() {
    let checkout = checkout();
    let outside = Dir::new("outside");
    let name = outside.0.to_str().expect("UTF-8").to_owned();
    assert!(name.starts_with('/'), "{name}");
    crafted_tag(&checkout, "absolute", &name);
    let path = format!("protocols/{name}/x/1.yaml");
    assert_refused_changing_nothing(&checkout, &path, |root| {
        vendor(root, checkout.path(), "absolute")
    });
    assert!(
        snapshot(&outside.0).is_empty(),
        "nothing is written at the absolute path"
    );
}

#[test]
fn a_blob_the_checkout_lacks_is_refused_and_never_fetched() {
    let checkout = checkout();
    git(
        checkout.path(),
        &["config", "uploadpack.allowFilter", "true"],
    );
    let partial = Dir::new("partial");
    let source = format!("file://{}", checkout.path().to_str().expect("UTF-8"));
    git(
        &partial.0,
        &[
            "clone",
            "-q",
            "--bare",
            "--filter=blob:none",
            "--template=",
            &source,
            "clone.git",
        ],
    );
    let clone = partial.0.join("clone.git");
    let blob = git(
        &clone,
        &["rev-parse", "t1:protocols/software-change/1.yaml"],
    );
    let present = |clone: &Path| {
        git_output(
            clone,
            &["-c", "protocol.allow=never", "cat-file", "-e", &blob],
        )
        .status
        .success()
    };
    assert!(!present(&clone), "the partial clone lacks the blob");
    assert_refused_changing_nothing(&checkout, "protocols/incident-response/1.yaml", |root| {
        vendor(root, &clone, "t1")
    });
    assert!(!present(&clone), "the command fetched the missing blob");
}

#[cfg(unix)]
#[test]
fn a_vendored_directory_that_is_a_link_is_refused_and_its_target_is_untouched() {
    let checkout = checkout();
    let root = Dir::new("root");
    let elsewhere = Dir::new("elsewhere");
    fs::create_dir_all(root.0.join("protocols")).expect("create protocols/");
    std::os::unix::fs::symlink(&elsewhere.0, root.0.join("protocols/engineering"))
        .expect("link protocols/engineering");
    let before = snapshot(&root.0);
    let out = vendor(&root.0, checkout.path(), "t1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a linked vendored directory is refused"
    );
    assert!(stderr.contains("protocols/engineering"), "{stderr}");
    assert_eq!(snapshot(&root.0), before);
    assert!(
        snapshot(&elsewhere.0).is_empty(),
        "the link's target is untouched"
    );
}
