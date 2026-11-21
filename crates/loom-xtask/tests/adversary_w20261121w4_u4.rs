//! Adversary cases for `story:engineering-protocols-vendor-command` (wave 2026-11-21-w4, U4).
//!
//! Each case builds its own Git repository under `std::env::temp_dir()` with no user or system
//! configuration, no hooks and a fixed identity and date. A case that plants a hostile setting
//! plants it in its own temporary repository only. Nothing reaches the network: the one remote a
//! case names is an `ext::` command that only touches a marker file in the case's own directory.

use loom_protocols::lock;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const RAW: &[u8] = b"name: raw-bytes\nbytes: \xff\xfe\n";

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "loom-adv-w4u4-{}-{}-{name}",
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

fn vendored(path: &str) -> Vec<u8> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocols/engineering")
        .join(path);
    fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
}

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

/// A repository holding `t1` (annotated: `software-change/1.yaml`, `incident-response/1.yaml`,
/// `vocabulary.yaml`) and `t3` (`raw-bytes/1.yaml`, not UTF-8).
struct Checkout {
    dir: Dir,
    blobs: Dir,
}

impl Checkout {
    fn path(&self) -> &Path {
        &self.dir.0
    }

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

    /// Commits a tree of exactly `files` and returns the commit; tags it when `tag` is given.
    fn commit_files(&self, tag: Option<(&str, bool)>, files: &[(&str, &[u8])]) -> String {
        git(self.path(), &["read-tree", "--empty"]);
        for (path, bytes) in files {
            let id = self.blob(bytes);
            git(
                self.path(),
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("100644,{id},{path}"),
                ],
            );
        }
        let tree = git(self.path(), &["write-tree"]);
        let commit = git(self.path(), &["commit-tree", "-m", "c", &tree]);
        match tag {
            Some((name, true)) => {
                git(self.path(), &["tag", "-a", "-m", name, name, &commit]);
            }
            Some((name, false)) => {
                git(self.path(), &["tag", name, &commit]);
            }
            None => {}
        }
        commit
    }
}

fn checkout() -> Checkout {
    let checkout = Checkout {
        dir: Dir::new("checkout"),
        blobs: Dir::new("blobs"),
    };
    git(checkout.path(), &["init", "-q", "--template="]);
    git(
        checkout.path(),
        &[
            "remote",
            "add",
            "origin",
            "git@example.invalid:someone/fork.git",
        ],
    );
    checkout.commit_files(
        Some(("t1", true)),
        &[
            (
                "protocols/software-change/1.yaml",
                &vendored("software-change/1.yaml"),
            ),
            (
                "protocols/incident-response/1.yaml",
                &vendored("incident-response/1.yaml"),
            ),
            ("protocols/vocabulary.yaml", b"terms: []\n"),
        ],
    );
    checkout.commit_files(Some(("t3", false)), &[("protocols/raw-bytes/1.yaml", RAW)]);
    checkout
}

fn vendor_with(
    root: &Path,
    checkout: impl AsRef<OsStr>,
    tag: &str,
    env: &[(&str, &OsStr)],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_loom-xtask"));
    command
        .arg("--root")
        .arg(root)
        .arg("protocols-vendor")
        .arg("--checkout")
        .arg(checkout)
        .args(["--tag", tag])
        .stdin(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("run loom-xtask")
}

fn vendor(root: &Path, checkout: impl AsRef<OsStr>, tag: &str) -> Output {
    vendor_with(root, checkout, tag, &[])
}

fn text(out: &Output) -> String {
    format!(
        "exit {}; stdout: {}; stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}

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

fn read(root: &Path, path: &str) -> Vec<u8> {
    let file = root.join("protocols/engineering").join(path);
    fs::read(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()))
}

/// A run that must be refused, name `named` and leave the root as it was, on an empty root and
/// on a root a `t1` run wrote.
fn refused_changing_nothing(checkout: &Checkout, named: &str, tag: &str) {
    for after_t1 in [false, true] {
        let root = Dir::new("root");
        if after_t1 {
            let out = vendor(&root.0, checkout.path(), "t1");
            assert!(out.status.success(), "t1: {}", text(&out));
        }
        let before = snapshot(&root.0);
        let out = vendor(&root.0, checkout.path(), tag);
        assert!(
            !out.status.success(),
            "--tag {tag} must be refused (after t1: {after_t1}), but: {}",
            text(&out)
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(named),
            "the refusal names {named:?}: {}",
            text(&out)
        );
        assert_eq!(
            snapshot(&root.0),
            before,
            "a refusal changes nothing (after t1: {after_t1})"
        );
    }
}

/// The checkout holds no tag `t9`, only a branch whose name is `refs/tags/t9`. `git rev-parse
/// refs/tags/t9^{commit}` resolves that through `refs/heads/refs/tags/t9`; the command must refuse
/// the tag it was asked for rather than lock the branch's commit as tag `t9`.
#[test]
fn adv_a_branch_named_refs_tags_x_is_not_vendored_as_tag_x() {
    let checkout = checkout();
    let branch = checkout.commit_files(None, &[("protocols/raw-bytes/1.yaml", RAW)]);
    git(
        checkout.path(),
        &["update-ref", "refs/heads/refs/tags/t9", &branch],
    );
    assert!(
        !git_output(
            checkout.path(),
            &["show-ref", "--verify", "--quiet", "refs/tags/t9"]
        )
        .status
        .success(),
        "the checkout holds no tag t9"
    );
    refused_changing_nothing(&checkout, "t9", "t9");
}

/// A partial clone whose promisor remote is an `ext::` command and whose own configuration allows
/// `ext::` (`protocol.ext.allow=always` outranks the helper's `-c protocol.allow=never`). Reading
/// the missing blob must not start the remote's command: only `GIT_NO_LAZY_FETCH` stops it.
#[test]
fn adv_a_promisor_remote_the_checkout_allows_runs_no_program() {
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
    let marker = partial.0.join("program-ran");
    let marker_text = marker.to_str().expect("UTF-8");
    assert!(
        !marker_text.contains(|c: char| c.is_whitespace() || c == '%'),
        "{marker_text}"
    );
    git(
        &clone,
        &[
            "config",
            "remote.origin.url",
            &format!("ext::sh -c touch% {marker_text}"),
        ],
    );
    git(&clone, &["config", "protocol.ext.allow", "always"]);
    let root = Dir::new("root");
    let out = vendor(&root.0, &clone, "t1");
    assert!(
        !marker.exists(),
        "the checkout's promisor remote ran a program: {}",
        text(&out)
    );
    assert!(!out.status.success(), "the blob is missing: {}", text(&out));
    assert!(snapshot(&root.0).is_empty(), "nothing is written");
}

/// `GIT_DIR` in the caller's environment names another repository whose `t1` holds other bytes.
/// The command reads the checkout it was given.
#[test]
fn adv_git_dir_in_the_environment_does_not_redirect_the_read() {
    let checkout = checkout();
    let other = Checkout {
        dir: Dir::new("other"),
        blobs: Dir::new("other-blobs"),
    };
    git(other.path(), &["init", "-q", "--template="]);
    other.commit_files(Some(("t1", false)), &[("protocols/raw-bytes/1.yaml", RAW)]);
    let git_dir = other.path().join(".git");
    let root = Dir::new("root");
    let out = vendor_with(
        &root.0,
        checkout.path(),
        "t1",
        &[("GIT_DIR", git_dir.as_os_str())],
    );
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        read(&root.0, "software-change/1.yaml"),
        vendored("software-change/1.yaml")
    );
    assert!(!root.0.join("protocols/engineering/raw-bytes").exists());
    let decoded = lock::decode(&read(&root.0, "lock.json")).expect("the lock decodes");
    assert_eq!(
        decoded.commit,
        git(checkout.path(), &["rev-parse", "t1^{commit}"])
    );
}

/// A replace ref in the checkout swaps `software-change/1.yaml`'s blob for other bytes. The tag's
/// bytes are written, not the replacement.
#[test]
fn adv_a_replace_ref_in_the_checkout_does_not_change_the_bytes() {
    let checkout = checkout();
    let original = git(
        checkout.path(),
        &["rev-parse", "t1:protocols/software-change/1.yaml"],
    );
    let replacement = checkout.blob(b"name: replaced\n");
    git(checkout.path(), &["replace", &original, &replacement]);
    let root = Dir::new("root");
    let out = vendor(&root.0, checkout.path(), "t1");
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        read(&root.0, "software-change/1.yaml"),
        vendored("software-change/1.yaml"),
        "the tag's blob, not the replace ref's"
    );
}

/// The caller's global Git configuration (here one Git cannot parse) is not read.
#[test]
fn adv_the_callers_global_git_configuration_is_not_read() {
    let checkout = checkout();
    let home = Dir::new("home");
    fs::write(home.0.join(".gitconfig"), b"[[[ not configuration\n").expect("write .gitconfig");
    fs::create_dir_all(home.0.join("xdg/git")).expect("create xdg");
    fs::write(home.0.join("xdg/git/config"), b"[[[ not configuration\n").expect("write config");
    let xdg = home.0.join("xdg");
    let root = Dir::new("root");
    let out = vendor_with(
        &root.0,
        checkout.path(),
        "t1",
        &[
            ("HOME", home.0.as_os_str()),
            ("XDG_CONFIG_HOME", xdg.as_os_str()),
        ],
    );
    assert!(out.status.success(), "{}", text(&out));
}

/// A tag whose `protocols/` holds no `<name>/<major>.yaml` file is refused, and a root a `t1` run
/// wrote keeps its catalog.
#[test]
fn adv_a_tag_with_no_protocol_file_is_refused() {
    let checkout = checkout();
    checkout.commit_files(
        Some(("empty", false)),
        &[
            ("protocols/vocabulary.yaml", b"terms: []\n"),
            ("README.md", b"nothing here\n"),
        ],
    );
    refused_changing_nothing(&checkout, "empty", "empty");
}

/// The root's `protocols/` is a link to a directory elsewhere: refused, and the link's target is
/// untouched.
#[cfg(unix)]
#[test]
fn adv_a_root_protocols_directory_that_is_a_link_is_refused() {
    let checkout = checkout();
    let root = Dir::new("root");
    let elsewhere = Dir::new("elsewhere");
    std::os::unix::fs::symlink(&elsewhere.0, root.0.join("protocols")).expect("link protocols");
    let before = snapshot(&root.0);
    let out = vendor(&root.0, checkout.path(), "t1");
    assert!(!out.status.success(), "{}", text(&out));
    assert_eq!(snapshot(&root.0), before);
    assert!(
        snapshot(&elsewhere.0).is_empty(),
        "nothing written through the link: {:?}",
        snapshot(&elsewhere.0).keys().collect::<Vec<_>>()
    );
}

/// The root's `protocols/engineering` is a regular file: refused, and the file is unchanged.
#[test]
fn adv_a_vendored_path_that_is_a_file_is_refused() {
    let checkout = checkout();
    let root = Dir::new("root");
    fs::create_dir_all(root.0.join("protocols")).expect("create protocols");
    fs::write(root.0.join("protocols/engineering"), b"a file\n").expect("write a file");
    let before = snapshot(&root.0);
    let out = vendor(&root.0, checkout.path(), "t1");
    assert!(!out.status.success(), "{}", text(&out));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("protocols/engineering"),
        "{}",
        text(&out)
    );
    assert_eq!(snapshot(&root.0), before);
}

/// A tag of an annotated tag peels to the commit; the lock names the tag as given.
#[test]
fn adv_a_tag_of_a_tag_locks_the_peeled_commit_and_the_given_name() {
    let checkout = checkout();
    git(checkout.path(), &["tag", "-a", "-m", "tt", "tt", "t1"]);
    let root = Dir::new("root");
    let out = vendor(&root.0, checkout.path(), "tt");
    assert!(out.status.success(), "{}", text(&out));
    let decoded = lock::decode(&read(&root.0, "lock.json")).expect("the lock decodes");
    assert_eq!(decoded.tag, "tt");
    assert_eq!(
        decoded.commit,
        git(checkout.path(), &["rev-parse", "t1^{commit}"])
    );
}
