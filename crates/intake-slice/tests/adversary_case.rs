//! Adversary cases for `intake_slice::case` (story `case-frontier`, wave 2026-10-04-w17).
//!
//! Fixture repositories live under `CARGO_TARGET_TMPDIR`. The fixtures' own git calls run with no
//! system or global configuration and with no inherited `GIT_DIR` or `GIT_WORK_TREE`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_commission::ports::governor::Governor as _;
use governor::{CanonGovernor, MemoryCaseStore};
use intake_slice::case::{self, CaseError};

const PICK: &str = "software-change@1";
const INTENT: &str = "make the failing test pass";

/// The SHA-256 of the UTF-8 bytes of [`INTENT`], computed outside Rust (`sha256sum`).
const INTENT_SHA256: &str = "f24909fc778c4e3c3482f6d6205955ec0015425fe1cc23822b034e54865abb93";

/// The module documents the intent revision as `intent-sha256-` and the lowercase hex SHA-256 of
/// the intent text's UTF-8 bytes. The acceptance test only checks that the revision is stable and
/// differs between texts, so a revision that drops a byte's leading zero or hashes something else
/// passes it. These literals come from `sha256sum`; `abc` hashes to a digest holding the byte 0x01.
#[test]
fn adversary_intent_revision_is_the_documented_sha256_literal() {
    assert_eq!(
        case::intent_revision(""),
        "intent-sha256-e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        case::intent_revision("abc"),
        "intent-sha256-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let workspace = Repo::new("intent-literal");
    workspace.commit("README.md", "first\n");
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, workspace.path()).expect("the case opens");
    assert_eq!(
        governor.revisions(&opened).expect("held")["intent"],
        format!("intent-sha256-{INTENT_SHA256}"),
        "the opened case's intent revision is the documented hash of the intent text"
    );
}

/// A directory that is not a git repository has no `HEAD` of its own. When it sits inside another
/// repository, git's discovery walks up and `case::open` takes the enclosing repository's `HEAD` as
/// the workspace's `implementation`.
#[test]
fn adversary_a_plain_directory_inside_another_repository_has_no_head() {
    let outer = Repo::new("enclosing");
    outer.commit("README.md", "outer\n");
    let inner = outer.path().join("not-a-repository");
    std::fs::create_dir_all(&inner).expect("create the inner directory");

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let result = case::open(&governor, PICK, INTENT, &inner);
    let leaked = result
        .as_ref()
        .ok()
        .map(|opened| governor.revisions(opened).expect("held")["implementation"].clone());
    assert!(
        matches!(result, Err(CaseError::Workspace { .. })),
        "a directory that is not a git repository opens no case; it opened {result:?} with \
         implementation {leaked:?}, the enclosing repository's HEAD is {}",
        outer.head()
    );
}

/// A bare repository has no working tree for the slice to edit, yet its `HEAD` reads.
#[test]
fn adversary_a_bare_repository_is_not_a_workspace() {
    let source = Repo::new("bare-source");
    source.commit("README.md", "first\n");
    let bare = scratch("bare");
    git(
        source.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            "--",
            ".",
            bare.to_str().expect("the scratch path is UTF-8"),
        ],
    );
    let _bare = Cleanup(bare.clone());

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let result = case::open(&governor, PICK, INTENT, &bare);
    assert!(
        matches!(result, Err(CaseError::Workspace { .. })),
        "a bare repository is not a workspace; it opened {result:?}"
    );
}

/// The variables the inherited-`GIT_DIR` child reads.
const CHILD_WORKSPACE: &str = "INTAKE_ADVERSARY_CHILD_WORKSPACE";
const CHILD_EXPECTED_HEAD: &str = "INTAKE_ADVERSARY_CHILD_EXPECTED_HEAD";
const CHILD_OTHER_HEAD: &str = "INTAKE_ADVERSARY_CHILD_OTHER_HEAD";

/// Git exports `GIT_DIR` and `GIT_WORK_TREE` to every hook (githooks(5)), so a slice started from
/// a hook, or under any tool that sets them, inherits them. `case::open` runs `git` in the
/// workspace with the environment it inherited, and `GIT_DIR` then names the repository git reads,
/// whatever the working directory. The child runs in its own process because setting the variable
/// in this one would reach every other test.
#[test]
fn adversary_an_inherited_git_dir_does_not_redirect_the_workspace() {
    let workspace = Repo::new("git-dir-workspace");
    workspace.commit("README.md", "the workspace\n");
    let elsewhere = Repo::new("git-dir-elsewhere");
    elsewhere.commit("OTHER.md", "another repository\n");
    assert_ne!(workspace.head(), elsewhere.head());

    let output = Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "adversary_git_dir_child_process",
            "--ignored",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("GIT_DIR", elsewhere.path().join(".git"))
        .env("GIT_WORK_TREE", elsewhere.path())
        .env(CHILD_WORKSPACE, workspace.path())
        .env(CHILD_EXPECTED_HEAD, workspace.head())
        .env(CHILD_OTHER_HEAD, elsewhere.head())
        .output()
        .expect("run the child test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("1 passed"),
        "the child ran exactly one test and it passed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(output.status.success(), "child exit: {:?}", output.status);
}

#[test]
#[ignore = "run in a child process by adversary_an_inherited_git_dir_does_not_redirect_the_workspace"]
fn adversary_git_dir_child_process() {
    let (Some(workspace), Some(expected)) = (
        std::env::var_os(CHILD_WORKSPACE),
        std::env::var(CHILD_EXPECTED_HEAD).ok(),
    ) else {
        return;
    };
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, Path::new(&workspace))
        .expect("the case opens on the workspace");
    let other = std::env::var(CHILD_OTHER_HEAD).unwrap_or_default();
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        expected,
        "implementation is the workspace's HEAD, not the inherited GIT_DIR's ({other})"
    );
}

/// `report_head` with an unmoved `HEAD` is the governor's no-op; a `HEAD` moved back to an earlier
/// commit is a new revision like any other.
#[test]
fn adversary_report_head_when_head_stays_and_when_it_moves_back() {
    let workspace = Repo::new("report-head");
    workspace.commit("README.md", "first\n");
    let first = workspace.head();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, workspace.path()).expect("opens");
    assert_eq!(governor.current_revision(&opened).expect("held"), 1);

    assert_eq!(
        case::report_head(&governor, &opened, workspace.path()).expect("reported"),
        1,
        "an unmoved HEAD leaves the case revision as it was"
    );

    workspace.commit("src.txt", "second\n");
    assert_eq!(
        case::report_head(&governor, &opened, workspace.path()).expect("reported"),
        2
    );
    git(workspace.path(), &["reset", "--quiet", "--hard", &first]);
    assert_eq!(
        case::report_head(&governor, &opened, workspace.path()).expect("reported"),
        3,
        "a HEAD moved back is a new revision"
    );
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        first
    );

    let unknown = b10x_commission::model::responsibility::CaseId("case-99".to_owned());
    assert!(matches!(
        case::report_head(&governor, &unknown, workspace.path()),
        Err(CaseError::Update(governor::UpdateError::UnknownCase))
    ));
}

/// Every ELS built-in opens: `intent` and `implementation` get the hash and `HEAD` where the
/// protocol declares them, every other declared artifact `r0`, and nothing undeclared.
#[test]
fn adversary_every_registry_protocol_opens_on_a_git_workspace() {
    let workspace = Repo::new("registry");
    workspace.commit("README.md", "first\n");
    let head = workspace.head();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let builtins = b10x_els::registry::list();
    assert!(builtins.len() >= 2, "the registry holds {builtins:?}");
    for (name, major) in builtins {
        let pick = format!("{name}@{major}");
        let declared = b10x_els::registry::get(name, major).expect("a valid built-in");
        let expected: BTreeMap<String, String> = declared
            .model
            .artifacts
            .ids()
            .map(|artifact| {
                let revision = match artifact.as_str() {
                    "intent" => format!("intent-sha256-{INTENT_SHA256}"),
                    "implementation" => head.clone(),
                    _ => "r0".to_owned(),
                };
                (artifact.as_str().to_owned(), revision)
            })
            .collect();
        let opened = case::open(&governor, &pick, INTENT, workspace.path())
            .unwrap_or_else(|error| panic!("{pick} opens: {error}"));
        assert_eq!(
            governor.revisions(&opened).expect("held"),
            expected,
            "{pick}"
        );
        assert!(governor.frontier(&opened).is_ok(), "{pick} has a frontier");
    }
}

/// A pick that is not `<name>@<major>` of a built-in is refused before git runs, so a directory
/// that is not a repository does not change the refusal.
#[test]
fn adversary_malformed_picks_are_refused_before_git_runs() {
    let not_a_repository = scratch("not-a-repository");
    std::fs::create_dir_all(&not_a_repository).expect("create");
    let _cleanup = Cleanup(not_a_repository.clone());
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    for pick in [
        "",
        "@",
        "@1",
        "software-change",
        "software-change@",
        "software-change@01",
        "software-change@+1",
        "software-change@-1",
        "software-change@1 ",
        " software-change@1",
        "software-change@1@1",
        "software-change@2",
        "software.change@1",
        "software-change/1",
        "no-such-protocol@1",
        "software-change@4294967296",
    ] {
        let result = case::open(&governor, pick, INTENT, &not_a_repository);
        assert!(
            matches!(&result, Err(CaseError::UnknownProtocol { pick: named, .. }) if named == pick),
            "`{pick}` is refused as an unknown protocol: {result:?}"
        );
    }
}

/// No commit, a missing directory: no `HEAD`, no case. A detached `HEAD` is still a commit.
#[test]
fn adversary_head_boundaries() {
    let governor = CanonGovernor::new(MemoryCaseStore::default());

    let unborn = Repo::new("unborn");
    assert!(matches!(
        case::open(&governor, PICK, INTENT, unborn.path()),
        Err(CaseError::Workspace { .. })
    ));

    let missing = scratch("missing");
    assert!(matches!(
        case::open(&governor, PICK, INTENT, &missing),
        Err(CaseError::Workspace { .. })
    ));

    let detached = Repo::new("detached space and ünïcode");
    detached.commit("a.txt", "a\n");
    let first = detached.head();
    detached.commit("b.txt", "b\n");
    git(
        detached.path(),
        &["checkout", "--quiet", "--detach", &first],
    );
    let opened = case::open(&governor, PICK, INTENT, detached.path()).expect("opens detached");
    assert_eq!(
        governor.revisions(&opened).expect("held")["implementation"],
        first
    );
    assert_eq!(
        governor.current_revision(&opened).expect("held"),
        1,
        "nothing raised the case revision before the first report"
    );
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adversary-case-{name}-{}-{nanos}",
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
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A git repository under the scratch directory, with a local identity.
struct Repo {
    root: Cleanup,
}

impl Repo {
    fn new(name: &str) -> Self {
        let root = scratch(name);
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
