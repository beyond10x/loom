//! Adversary pass 2 cases for `intake_slice::case` (story `case-frontier`, wave 2026-10-04-w17).
//!
//! The module documents the workspace as "the root of a non-bare git work tree". These cases drive
//! `case::open` with work tree roots reached in ways the earlier cases do not use, and with git
//! directories that are not work trees. Fixture repositories live under `CARGO_TARGET_TMPDIR`; the
//! fixtures' own git calls run with no system or global configuration and with no inherited
//! `GIT_DIR` or `GIT_WORK_TREE`.

#![cfg(unix)]

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use governor::{CanonGovernor, MemoryCaseStore};
use intake_slice::case::{self, CaseError};

const PICK: &str = "software-change@1";
const INTENT: &str = "make the failing test pass";

/// A directory name may end in whitespace. Git prints such a root verbatim from
/// `rev-parse --show-toplevel`, and the slice trims git's output before comparing it with the
/// workspace, so the trimmed root names a different path and a valid work tree root is refused.
#[test]
fn adversary2_a_work_tree_root_whose_name_ends_in_a_space_opens() {
    let mut path = scratch("trailing-space").into_os_string();
    path.push(" ");
    let workspace = Repo::at(PathBuf::from(path));
    workspace.commit("README.md", "first\n");

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, workspace.path());
    let opened = match opened {
        Ok(opened) => opened,
        Err(error) => panic!(
            "`{}` is the root of a non-bare work tree and opens a case; it was refused: {error}",
            workspace.path().display()
        ),
    };
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        workspace.head()
    );
}

/// A Unix path need not be UTF-8. Git runs in such a work tree and prints its root, and the slice
/// refuses the root because git's output is not UTF-8, although only the object name is needed.
#[test]
fn adversary2_a_work_tree_root_whose_path_is_not_utf8_opens() {
    let mut name = OsString::from(format!("adversary2-case-latin1-{}-", std::process::id()));
    name.push(OsStr::from_bytes(b"caf\xe9"));
    let workspace = Repo::at(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name));
    workspace.commit("README.md", "first\n");

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, workspace.path());
    let opened = match opened {
        Ok(opened) => opened,
        Err(error) => panic!(
            "{:?} is the root of a non-bare work tree and opens a case; it was refused: {error}",
            workspace.path()
        ),
    };
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        workspace.head()
    );
}

/// A work tree root reached through a symbolic link or through `..`, a linked worktree (a `.git`
/// file), a submodule and a work tree with a separate git directory are all roots of non-bare work
/// trees, and each opens on its own `HEAD`. Every earlier case passes a canonical path, so none of
/// them notices a workspace path that is compared with git's root without being resolved first.
#[test]
fn adversary2_paths_that_resolve_to_a_work_tree_root_open_on_its_head() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let implementation = |workspace: &Path| match case::open(&governor, PICK, INTENT, workspace) {
        Ok(opened) => governor.revisions(&opened).expect("held")["implementation"].clone(),
        Err(error) => panic!("{workspace:?} opens a case: {error}"),
    };

    let main = Repo::new("main");
    main.commit("a.txt", "a\n");
    let first = main.head();
    main.commit("b.txt", "b\n");
    let second = main.head();
    assert_ne!(first, second);

    let link = scratch("link");
    std::os::unix::fs::symlink(main.path(), &link).expect("link to the work tree");
    let _link = Cleanup(link.clone());
    assert_eq!(implementation(&link), second, "through a symbolic link");

    std::fs::create_dir_all(main.path().join("nested")).expect("create a nested directory");
    assert_eq!(
        implementation(&main.path().join("nested").join("..")),
        second,
        "through `..`"
    );

    let linked = scratch("linked-worktree");
    let _linked = Cleanup(linked.clone());
    git(
        main.path(),
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().expect("the scratch path is UTF-8"),
            &first,
        ],
    );
    assert_eq!(
        implementation(&linked),
        first,
        "a linked worktree opens on its own HEAD, not the main work tree's"
    );

    let outer = Repo::new("superproject");
    outer.commit("README.md", "outer\n");
    git(
        outer.path(),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
            "--",
            main.path().to_str().expect("the scratch path is UTF-8"),
            "sub",
        ],
    );
    let submodule = outer.path().join("sub");
    assert_ne!(outer.head(), second);
    assert_eq!(
        implementation(&submodule),
        second,
        "a submodule opens on its own HEAD, not the superproject's"
    );

    let separate = scratch("separate-work-tree");
    let separate_git = scratch("separate-git-dir");
    std::fs::create_dir_all(&separate).expect("create the work tree");
    let (_separate, _separate_git) = (Cleanup(separate.clone()), Cleanup(separate_git.clone()));
    git(
        &separate,
        &[
            "init",
            "--quiet",
            "--separate-git-dir",
            separate_git.to_str().expect("the scratch path is UTF-8"),
        ],
    );
    std::fs::write(separate.join("c.txt"), "c\n").expect("write");
    git(&separate, &["add", "--", "c.txt"]);
    git(
        &separate,
        &[
            "-c",
            "user.name=Fixture Author",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--message",
            "c",
        ],
    );
    let separate_head = git(&separate, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(
        implementation(&separate),
        separate_head,
        "a separate git dir"
    );
}

/// A repository's `.git` directory reads a `HEAD` but is no work tree, and a bare repository whose
/// configuration also names a work tree is still bare: git warns that the two do not make sense
/// and treats it as bare. Neither is a workspace.
#[test]
fn adversary2_a_git_directory_is_not_a_workspace() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let workspace = Repo::new("dot-git");
    workspace.commit("README.md", "first\n");
    let dot_git = workspace.path().join(".git");
    let result = case::open(&governor, PICK, INTENT, &dot_git);
    assert!(
        matches!(result, Err(CaseError::Workspace { .. })),
        "a `.git` directory is not a workspace; it opened {result:?}"
    );

    let bare = scratch("bare-with-worktree");
    let _bare = Cleanup(bare.clone());
    git(
        workspace.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            "--",
            ".",
            bare.to_str().expect("the scratch path is UTF-8"),
        ],
    );
    let elsewhere = scratch("bare-worktree");
    std::fs::create_dir_all(&elsewhere).expect("create");
    let _elsewhere = Cleanup(elsewhere.clone());
    git(
        &bare,
        &[
            "config",
            "core.worktree",
            elsewhere.to_str().expect("the scratch path is UTF-8"),
        ],
    );
    let result = case::open(&governor, PICK, INTENT, &bare);
    assert!(
        matches!(result, Err(CaseError::Workspace { .. })),
        "a bare repository with core.worktree is not a workspace; it opened {result:?}"
    );
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary2-case-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}

/// Removes a scratch path when dropped.
struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A git repository under the scratch directory, with a local identity.
struct Repo {
    root: Cleanup,
}

impl Repo {
    fn new(name: &str) -> Self {
        Self::at(scratch(name))
    }

    fn at(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).expect("create the fixture repository");
        let repo = Self {
            root: Cleanup(root),
        };
        git(repo.path(), &["init", "--quiet"]);
        git(repo.path(), &["config", "user.name", "Fixture Author"]);
        git(
            repo.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        repo
    }

    fn path(&self) -> &Path {
        &self.root.0
    }

    fn commit(&self, file: &str, contents: &str) {
        std::fs::write(self.path().join(file), contents).expect("write a fixture file");
        git(self.path(), &["add", "--", file]);
        git(self.path(), &["commit", "--quiet", "--message", file]);
    }

    fn head(&self) -> String {
        git(self.path(), &["rev-parse", "HEAD"]).trim().to_owned()
    }
}
