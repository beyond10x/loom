//! Adversary cases, pass 2, for `story:engineering-protocols-vendor-command` (wave 2026-11-21-w4,
//! U4): the exact tag ref, the Git version gate and the lazy-fetch guard after correction 1.
//!
//! Each case builds its own Git repository under `std::env::temp_dir()` with no user or system
//! configuration, no hooks and a fixed identity and date. A hostile setting is planted in the
//! case's own temporary repository only; the one program a case plants touches a marker file in
//! its own directory. Nothing reaches the network.

use loom_protocols::lock;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
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
            "loom-adv2-w4u4-{}-{}-{name}",
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

/// `t1` (annotated: `software-change/1.yaml`, `incident-response/1.yaml`) and `t3` (`raw-bytes`).
fn checkout() -> Checkout {
    let checkout = Checkout {
        dir: Dir::new("checkout"),
        blobs: Dir::new("blobs"),
    };
    git(checkout.path(), &["init", "-q", "--template="]);
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

/// A run with `env` that must be refused, name `named` on standard error and leave the root as it
/// was, on an empty root and on a root a plain `t1` run wrote.
fn refused_changing_nothing(checkout: &Checkout, named: &str, tag: &str, env: &[(&str, &OsStr)]) {
    for after_t1 in [false, true] {
        let root = Dir::new("root");
        if after_t1 {
            let out = vendor(&root.0, checkout.path(), "t1");
            assert!(out.status.success(), "t1: {}", text(&out));
        }
        let before = snapshot(&root.0);
        let out = vendor_with(&root.0, checkout.path(), tag, env);
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

/// The `git` the test itself finds on `PATH`.
fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH is set");
    std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("git on PATH")
}

/// A partial clone whose promisor remote is the `file://` source, whose own configuration allows
/// `file` (outranking `-c protocol.allow=never`) and names `remote.origin.uploadpack`, a program
/// that touches a marker. `-c protocol.ext.allow=never` does not cover `file`; only
/// `GIT_NO_LAZY_FETCH` keeps `cat-file` on the missing blob from starting that program.
#[test]
fn adv2_a_file_promisor_remote_with_an_upload_pack_program_runs_nothing() {
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
    let program = partial.0.join("upload-pack.sh");
    fs::write(
        &program,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
    )
    .expect("write the program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    git(
        &clone,
        &[
            "config",
            "remote.origin.uploadpack",
            program.to_str().expect("UTF-8"),
        ],
    );
    git(&clone, &["config", "protocol.file.allow", "always"]);
    let root = Dir::new("root");
    let out = vendor(&root.0, &clone, "t1");
    assert!(
        !marker.exists(),
        "the checkout's upload-pack program ran: {}",
        text(&out)
    );
    assert!(!out.status.success(), "the blob is missing: {}", text(&out));
    assert!(snapshot(&root.0).is_empty(), "nothing is written");
}

/// A `git` on `PATH` that reports 2.43.0 (and otherwise runs the real Git, logging every call) is
/// refused naming the version, before any other Git call, and nothing is written.
#[cfg(unix)]
#[test]
fn adv2_a_git_older_than_2_44_on_path_is_refused_before_any_other_git_call() {
    use std::os::unix::fs::PermissionsExt;
    let checkout = checkout();
    let bin = Dir::new("old-git");
    let calls = bin.0.join("calls");
    let wrapper = bin.0.join("git");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{calls}'\nfor a in \"$@\"; do\n  if [ \"$a\" = --version ]; \
             then echo 'git version 2.43.0'; exit 0; fi\ndone\nexec '{git}' \"$@\"\n",
            calls = calls.display(),
            git = real_git().display()
        ),
    )
    .expect("write the wrapper");
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).expect("chmod");
    let mut path = OsString::from(bin.0.as_os_str());
    path.push(":");
    path.push(std::env::var_os("PATH").expect("PATH is set"));
    refused_changing_nothing(&checkout, "2.43.0", "t1", &[("PATH", path.as_os_str())]);
    let logged = fs::read_to_string(&calls).expect("the wrapper ran");
    assert!(
        logged.lines().all(|line| line.ends_with("--version")),
        "only `git --version` runs before the refusal: {logged}"
    );
}

/// No `git` on `PATH`: refused, nothing written.
#[test]
fn adv2_no_git_on_path_is_refused_and_nothing_is_written() {
    let checkout = checkout();
    let empty = Dir::new("empty-path");
    refused_changing_nothing(&checkout, "git", "t1", &[("PATH", empty.0.as_os_str())]);
}

/// A tag that points at a tree is refused naming the tag.
#[test]
fn adv2_a_tag_that_points_at_a_tree_is_refused() {
    let checkout = checkout();
    let tree = git(checkout.path(), &["rev-parse", "t1^{tree}"]);
    git(checkout.path(), &["tag", "tree-tag", &tree]);
    refused_changing_nothing(&checkout, "tree-tag", "tree-tag", &[]);
}

/// `refs/tags/t9` is a directory (`t9/x` is a tag): refused, `t9/x` is not taken for `t9`.
#[test]
fn adv2_a_tag_ref_that_is_a_directory_is_refused() {
    let checkout = checkout();
    git(checkout.path(), &["tag", "t9/x", "t3"]);
    refused_changing_nothing(&checkout, "t9", "t9", &[]);
}

/// Every other ref `rev-parse refs/tags/t9` would find (`refs/refs/tags/t9`,
/// `refs/tags/refs/tags/t9`, `refs/remotes/refs/tags/t9`, `refs/remotes/refs/tags/t9/HEAD`) is
/// not taken for tag `t9`.
#[test]
fn adv2_no_other_ref_spelled_refs_tags_x_is_taken_for_tag_x() {
    let checkout = checkout();
    let commit = git(checkout.path(), &["rev-parse", "t3^{commit}"]);
    for other in [
        "refs/refs/tags/t9",
        "refs/tags/refs/tags/t9",
        "refs/remotes/refs/tags/t9",
        "refs/remotes/refs/tags/t8/HEAD",
    ] {
        git(checkout.path(), &["update-ref", other, &commit]);
    }
    for tag in ["t9", "t8"] {
        refused_changing_nothing(&checkout, tag, tag, &[]);
    }
}

/// A replace ref for `t1`'s tag object, naming another tag object that points at `t3`: the lock
/// keeps `t1`'s own commit and files.
#[test]
fn adv2_a_replace_ref_for_the_tag_object_does_not_move_the_tag() {
    let checkout = checkout();
    let tag_object = git(checkout.path(), &["rev-parse", "refs/tags/t1"]);
    git(
        checkout.path(),
        &["tag", "-a", "-m", "decoy", "decoy", "t3"],
    );
    let decoy = git(checkout.path(), &["rev-parse", "refs/tags/decoy"]);
    git(checkout.path(), &["replace", "-f", &tag_object, &decoy]);
    let root = Dir::new("root");
    let out = vendor(&root.0, checkout.path(), "t1");
    assert!(out.status.success(), "{}", text(&out));
    let decoded = lock::decode(&read(&root.0, "lock.json")).expect("the lock decodes");
    assert_eq!(
        decoded.commit,
        git(
            checkout.path(),
            &["--no-replace-objects", "rev-parse", "t1^{commit}"]
        )
    );
    assert_eq!(
        read(&root.0, "software-change/1.yaml"),
        vendored("software-change/1.yaml")
    );
}
