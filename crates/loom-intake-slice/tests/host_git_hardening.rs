//! Loom's host-side git runs no hooks and no fsmonitor (story `host-git-hardening`; design
//! `effect-isolation`, decision 3 and threat T4).
//!
//! `.git` sits inside the work tree a test command may write, so the code a model edited can plant
//! a hook in `.git/hooks` or a `core.fsmonitor` command in `.git/config`, and the next git call
//! Loom makes on the host would run it as the operator. Every git command Loom runs for a run
//! therefore carries `-c core.hooksPath=/dev/null -c core.fsmonitor=false` and the other
//! overrides in `crate::git`, through one helper, which also refuses a workspace whose own
//! configuration names a program or changed since the case opened.
//!
//! The two cases the story started with; the others below each name what they plant:
//!
//! - [`a_run_that_edits_and_commits_runs_no_workspace_hook_and_no_fsmonitor`]: a fixture workspace
//!   whose local configuration points `core.hooksPath` at `.git/hooks` (so the operator's global
//!   setting cannot hide the hooks) holds hooks for every step of a commit, each writing a marker
//!   outside the work tree, and sets `core.fsmonitor` to a command writing another. A case is
//!   opened on it and the slice's executor performs `repository.inspect`, `repository.edit` (which
//!   commits) and `tests.run`: every entry through which Loom runs git. No marker may exist
//!   afterwards. The same fixture then runs plain `git status` and `git commit` itself and the
//!   pre-commit and fsmonitor markers must appear, so the planted commands are shown to be live.
//! - [`every_host_git_command_goes_through_the_one_helper`]: a scan of every crate's `src/` finds
//!   `Command::new` with a git program only in `crates/loom-intake-slice/src/git.rs`, and no
//!   manifest or lock file names a git library, so a new call site cannot forget the flags.
//!
//! Fixture repositories live under `CARGO_TARGET_TMPDIR`; the fixture's own git calls run with no
//! system or global configuration. Source paths are read at run time from `CARGO_MANIFEST_DIR`.
//! The planted commands are shell scripts, so the file is Unix-only, as Loom's CI is.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    ExecutorOutcomeProposedAction, ProposedActionArguments,
};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::executor::{ExecuteError, LocalExecutor, Report, TestCommand};
use b10x_loom_intake_slice::git::HostGitRefusal;
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::json;

const PICK: &str = "software-change@1";
const INTENT: &str = "make the failing check pass";

/// The hooks git runs around `add` and `commit`; each one planted writes its own marker.
const HOOKS: [&str; 6] = [
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "post-index-change",
    "reference-transaction",
];

/// The marker the planted `core.fsmonitor` command writes.
const FSMONITOR: &str = "fsmonitor";

/// The marker of a planted `filter.x.clean` command.
const FILTER: &str = "filter-clean";

/// The marker of a planted `gpg.program`.
const GPG: &str = "gpg-program";

/// The one file allowed to start git on the host.
const HELPER: &str = "crates/loom-intake-slice/src/git.rs";

#[test]
fn a_run_that_edits_and_commits_runs_no_workspace_hook_and_no_fsmonitor() {
    let fixture = Fixture::new();
    let first = fixture.head();

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        fixture.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));

    let inspected = executor
        .execute(&proposal(
            "repository.inspect",
            &json!({"paths": ["check.txt"]}),
        ))
        .expect("inspect is performed");
    assert!(matches!(inspected, Report::Inspected(_)), "{inspected:?}");
    let edited = executor
        .execute(&proposal(
            "repository.edit",
            &json!({
                "files": [{"path": "check.txt", "contents": "fixed\n"}],
                "message": "fix the check"
            }),
        ))
        .expect("the edit is committed");
    let Report::Edited { revision } = &edited else {
        panic!("an edit reports its revision: {edited:?}");
    };
    assert_ne!(revision, &first, "the edit made a commit");
    assert_eq!(revision, &fixture.head(), "the edit's revision is HEAD");
    let run = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    assert!(matches!(run, Report::TestsRun(_)), "{run:?}");

    let fired = fixture.markers();
    assert!(
        fired.is_empty(),
        "Loom's host-side git ran what the workspace planted: {fired:?}"
    );

    // The control: the same workspace, with git as the operator runs it, executes both.
    fixture.git(&["status", "--porcelain"]);
    fixture.git(&["commit", "--quiet", "--allow-empty", "--message", "control"]);
    let control = fixture.markers();
    for marker in ["pre-commit", FSMONITOR] {
        assert!(
            control.iter().any(|fired| fired == marker),
            "the planted `{marker}` does not fire under plain git, so this case proves nothing: \
             {control:?}"
        );
    }
}

#[test]
fn a_filter_driver_a_test_plants_never_runs() {
    let fixture = Fixture::unplanted();
    let clean = fixture.planted(FILTER, "cat");
    let plant = format!(
        "git config --local filter.x.clean '{}' && echo 'check.txt filter=x' > .gitattributes",
        clean.display()
    );
    let refused = planted_by_the_test_command(&fixture, &plant);
    assert_eq!(refused, fixture.markers(), "nothing else fired");

    // The control: plain git runs the planted clean filter when it adds the file.
    std::fs::write(fixture.workspace().join("check.txt"), "fixed\n").expect("write check.txt");
    fixture.git(&["add", "check.txt"]);
    assert_eq!(
        fixture.markers(),
        [FILTER],
        "the planted filter does not run under plain git, so this case proves nothing"
    );
}

#[test]
fn a_gpg_program_a_test_plants_never_runs() {
    let fixture = Fixture::unplanted();
    let gpg = fixture.planted(GPG, "exit 1");
    let plant = format!(
        "git config --local commit.gpgsign true && git config --local gpg.program '{}'",
        gpg.display()
    );
    let refused = planted_by_the_test_command(&fixture, &plant);
    assert_eq!(refused, fixture.markers(), "nothing else fired");

    // The control: plain git runs the planted signing program when it commits.
    let control = fixture.git_raw(&["commit", "--quiet", "--allow-empty", "--message", "c"]);
    assert!(
        !control.status.success(),
        "the planted program refuses to sign"
    );
    assert_eq!(
        fixture.markers(),
        [GPG],
        "the planted signing program does not run under plain git, so this case proves nothing"
    );
}

/// Opens a case on `fixture`, lets `tests.run` run `plant` with `sh -c` (a test that writes into
/// `.git/config`, as a confined command could), then asks for the edit that fixes the check. The
/// edit and a second `tests.run` must be refused before git runs, with nothing written, nothing
/// committed and no marker; returns the markers (none).
fn planted_by_the_test_command(fixture: &Fixture, plant: &str) -> Vec<String> {
    let first = fixture.head();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case.clone(),
        fixture.workspace(),
        TestCommand::new("sh", ["-c", plant]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));
    let run = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    let Report::TestsRun(run) = &run else {
        panic!("tests.run reports a run: {run:?}");
    };
    assert_eq!(run.exit_code(), Some(0), "the plant ran: {run:?}");
    assert!(fixture.markers().is_empty(), "planting fired a marker");

    let edited = executor.execute(&proposal(
        "repository.edit",
        &json!({
            "files": [{"path": "check.txt", "contents": "fixed\n"}],
            "message": "fix the check"
        }),
    ));
    let fired = fixture.markers();
    assert!(
        fired.is_empty(),
        "Loom's host-side git ran what the test planted in `.git/config`: {fired:?} \
         (edit: {edited:?})"
    );
    assert!(
        matches!(
            edited,
            Err(ExecuteError::HostGit(HostGitRefusal::ConfigChanged { .. }))
        ),
        "the edit is refused as a changed configuration: {edited:?}"
    );
    assert_eq!(fixture.head(), first, "a refused edit committed");
    assert_eq!(
        fixture.read("check.txt"),
        "broken\n",
        "a refused edit wrote"
    );
    let rerun = executor.execute(&proposal("tests.run", &json!({})));
    assert!(
        matches!(
            rerun,
            Err(ExecuteError::HostGit(HostGitRefusal::ConfigChanged { .. }))
        ),
        "a later tests.run is refused as a changed configuration: {rerun:?}"
    );
    assert!(fixture.markers().is_empty(), "{:?}", fixture.markers());
    fixture.markers()
}

/// An executor on a workspace no case opened in this process has no recording to check against,
/// so every action that runs git is refused rather than run unchecked.
#[test]
fn a_workspace_no_case_opened_is_refused_host_git() {
    let opened = Fixture::unplanted();
    let unopened = Fixture::unplanted();
    let first = unopened.head();
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, opened.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        unopened.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));
    for (action, arguments) in [
        ("repository.inspect", json!({"paths": ["check.txt"]})),
        (
            "repository.edit",
            json!({"files": [{"path": "check.txt", "contents": "fixed\n"}]}),
        ),
        ("tests.run", json!({})),
    ] {
        let result = executor.execute(&proposal(action, &arguments));
        assert!(
            matches!(
                result,
                Err(ExecuteError::HostGit(HostGitRefusal::Unrecorded { .. }))
            ),
            "`{action}` on an unopened workspace is refused: {result:?}"
        );
    }
    assert_eq!(unopened.head(), first, "a refused edit committed");
    assert_eq!(
        unopened.read("check.txt"),
        "broken\n",
        "a refused edit wrote"
    );
}

/// A workspace whose own configuration already names a program when the case opens is refused at
/// open, naming the key; keys the slice overrides on its command line (`core.hooksPath`,
/// `core.fsmonitor`) are not, as the first case shows.
#[test]
fn a_program_key_in_the_workspace_config_refuses_the_open() {
    for (key, listed) in [
        ("core.sshCommand", "core.sshcommand"),
        ("filter.lfs.smudge", "filter.lfs.smudge"),
        ("credential.helper", "credential.helper"),
    ] {
        let fixture = Fixture::unplanted();
        let program = fixture.planted("planted", "exit 0");
        fixture.git(&["config", key, &program.to_string_lossy()]);
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let opened = case::open(&governor, PICK, INTENT, fixture.workspace());
        assert!(
            matches!(
                &opened,
                Err(case::CaseError::HostGit(HostGitRefusal::ProgramKey { key, scope, .. }))
                    if key == listed && scope == "local"
            ),
            "a local `{key}` refuses the open: {opened:?}"
        );
        assert!(fixture.markers().is_empty(), "{:?}", fixture.markers());
    }
}

/// A test that writes `.git/commondir` moves git's common directory (its configuration, objects and
/// refs) without changing a byte of `.git/config`. Here the new common directory is a plain copy,
/// with no program key for the configuration scan to find: the move alone refuses the next call.
#[test]
fn a_commondir_a_test_plants_is_refused() {
    let fixture = Fixture::unplanted();
    let first = fixture.head();
    let common = fixture.root.join("common");
    let plant = format!(
        "cp -R .git '{common}' && echo '{common}' > .git/commondir",
        common = common.display()
    );
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        fixture.workspace(),
        TestCommand::new("sh", ["-c", plant.as_str()]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));
    let run = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    let Report::TestsRun(run) = &run else {
        panic!("tests.run reports a run: {run:?}");
    };
    assert_eq!(run.exit_code(), Some(0), "the plant ran: {run:?}");
    let edited = executor.execute(&proposal(
        "repository.edit",
        &json!({"files": [{"path": "check.txt", "contents": "fixed\n"}]}),
    ));
    assert!(
        matches!(
            &edited,
            Err(ExecuteError::HostGit(
                HostGitRefusal::GitDirChanged { .. } | HostGitRefusal::ConfigChanged { .. }
            ))
        ),
        "an edit after a planted `commondir` is refused: {edited:?}"
    );
    std::fs::remove_file(fixture.workspace().join(".git").join("commondir"))
        .expect("remove the planted commondir");
    assert_eq!(fixture.head(), first, "a refused edit committed");
    assert_eq!(
        fixture.read("check.txt"),
        "broken\n",
        "a refused edit wrote"
    );
}

/// A linked worktree keeps its git directory under the main repository's, with a `commondir` file;
/// recorded when the case opens, it is the workspace's own and an edit commits.
#[test]
fn a_linked_worktree_opens_and_commits() {
    let fixture = Fixture::unplanted();
    let linked = fixture.root.join("linked");
    fixture.git(&[
        "worktree",
        "add",
        "--quiet",
        "-b",
        "linked",
        &linked.to_string_lossy(),
    ]);
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, &linked).expect("a linked worktree opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        &linked,
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));
    let edited = executor
        .execute(&proposal(
            "repository.edit",
            &json!({"files": [{"path": "check.txt", "contents": "fixed\n"}]}),
        ))
        .expect("the edit commits in the linked worktree");
    assert!(matches!(edited, Report::Edited { .. }), "{edited:?}");
}

#[test]
fn every_host_git_command_goes_through_the_one_helper() {
    let root = workspace_root();
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates/") {
        let src = entry.expect("a crates/ entry").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut sources);
        }
    }
    assert!(
        sources
            .iter()
            .any(|path| path.ends_with("loom-intake-slice/src/executor.rs")),
        "the scan reaches the slice's executor: {sources:?}"
    );

    let mut starts = Vec::new();
    for path in &sources {
        let text = std::fs::read_to_string(path).expect("read a source file");
        let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let relative = path
            .strip_prefix(&root)
            .expect("under the workspace")
            .to_string_lossy()
            .into_owned();
        for (at, _) in squeezed.match_indices("Command::new(") {
            let argument: String = squeezed[at + "Command::new(".len()..]
                .chars()
                .take_while(|c| *c != ')')
                .collect();
            if argument.to_ascii_lowercase().contains("git") {
                starts.push((relative.clone(), argument));
            }
        }
    }
    let bypassing: Vec<&(String, String)> =
        starts.iter().filter(|(file, _)| file != HELPER).collect();
    assert!(
        bypassing.is_empty(),
        "git is started outside `{HELPER}`, without its hook and fsmonitor flags: {bypassing:?}"
    );
    assert_eq!(
        starts.len(),
        1,
        "`{HELPER}` starts git in exactly one place: {starts:?}"
    );

    let helper = std::fs::read_to_string(root.join(HELPER)).expect("read the helper");
    for flag in ["core.hooksPath=", "core.fsmonitor=false"] {
        assert!(helper.contains(flag), "`{HELPER}` does not set `{flag}`");
    }

    // Substrate's pinned host driver implements its own Git source service. Loom never
    // enables that service. Every dependency path outside that exact foundation still
    // must be free of a Git library, including renamed and transitive dependencies.
    let metadata = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .current_dir(&root)
        .output()
        .expect("cargo metadata");
    assert!(
        metadata.status.success(),
        "{}",
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let nodes = metadata["resolve"]["nodes"].as_array().unwrap();
    let mut pending: Vec<String> = metadata["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let package = packages.iter().find(|package| package["id"] == id).unwrap();
        let name = package["name"].as_str().unwrap();
        if name == "b10x-substrate-host" {
            assert_eq!(package["version"], "0.7.10");
            assert_eq!(
                package["source"],
                "git+https://github.com/beyond10x/substrate?rev=65304edf6ebdf4a95f9c2c6138b0c20ea47d157e#65304edf6ebdf4a95f9c2c6138b0c20ea47d157e"
            );
            continue;
        }
        assert!(
            git_library(&format!("name = \"{name}\"")).is_none(),
            "{name} is reachable outside the pinned Substrate host"
        );
        let node = nodes.iter().find(|node| node["id"] == id).unwrap();
        pending.extend(
            node["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_owned()),
        );
    }
    let mut manifests = vec![root.join("Cargo.toml")];
    for entry in std::fs::read_dir(root.join("crates")).expect("read crates/") {
        let manifest = entry.expect("a crates/ entry").path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }
    for manifest in &manifests {
        let text = std::fs::read_to_string(manifest).expect("read a manifest");
        for line in text.lines() {
            if let Some(library) = git_library(line) {
                panic!(
                    "`{}` names the git library `{library}`, which bypasses the helper: `{}`",
                    manifest.display(),
                    line.trim()
                );
            }
        }
    }
    for (line, library) in [
        ("git2 = \"0.20\"", "git2"),
        ("gix.workspace = true", "gix"),
        ("[dependencies.gix-status]", "gix-status"),
        ("name = \"libgit2-sys\"", "libgit2-sys"),
    ] {
        assert_eq!(git_library(line), Some(library), "the scan misses `{line}`");
    }
    for line in ["b10x-loom-cli = { path = \"x\" }", "name = \"digest\""] {
        assert_eq!(git_library(line), None, "the scan flags `{line}`");
    }
}

/// The git library a manifest or lock-file line names as a package or dependency key, if any:
/// `git2`, `libgit2-sys`, `gix` or a `gix-*` crate.
fn git_library(line: &str) -> Option<&str> {
    let line = line.trim();
    let name = if let Some(locked) = line.strip_prefix("name = \"") {
        locked.split('"').next()?
    } else {
        let key = line
            .trim_start_matches('[')
            .trim_start_matches("dependencies.")
            .trim_start_matches("dev-dependencies.")
            .trim_start_matches("build-dependencies.")
            .trim_start_matches("workspace.dependencies.");
        key.split(|c: char| c == '=' || c == '.' || c == ']' || c.is_whitespace())
            .next()?
            .trim_matches('"')
    };
    let git = matches!(name, "git2" | "libgit2-sys" | "gix") || name.starts_with("gix-");
    git.then_some(name)
}

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    manifest
        .ancestors()
        .nth(2)
        .expect("the crate sits at crates/<name>")
        .to_path_buf()
}

fn rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("read a source directory") {
        let path = entry.expect("a source entry").path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

/// A scratch git repository with a local identity and one failing check, whose `.git` then holds
/// a hook for every step of a commit and a `core.fsmonitor` command, each writing a marker into a
/// sibling directory.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    markers: PathBuf,
}

impl Fixture {
    /// The fixture with every hook and the `core.fsmonitor` command planted before the case opens.
    fn new() -> Self {
        let fixture = Self::unplanted();
        let hooks = fixture.workspace.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("create .git/hooks");
        for hook in HOOKS {
            fixture.script(&hooks.join(hook), hook, "exit 0");
        }
        let fsmonitor = fixture.workspace.join(".git").join("fsmonitor-planted");
        fixture.script(&fsmonitor, FSMONITOR, "exit 0");
        fixture.git(&["config", "core.hooksPath", &hooks.to_string_lossy()]);
        fixture.git(&["config", "core.fsmonitor", &fsmonitor.to_string_lossy()]);
        assert!(
            fixture.markers().is_empty(),
            "planting fired a marker: {:?}",
            fixture.markers()
        );
        fixture
    }

    /// Writes an executable shell script at `path` that records `marker` (with its arguments) and
    /// then runs `tail`.
    fn script(&self, path: &Path, marker: &str, tail: &str) {
        use std::os::unix::fs::PermissionsExt as _;

        let script = format!(
            "#!/bin/sh\necho \"$@\" > '{}'\n{tail}\n",
            self.markers.join(marker).display()
        );
        std::fs::write(path, script).expect("write a planted command");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make a planted command executable");
    }

    /// A planted command outside the work tree, named after its marker.
    fn planted(&self, marker: &str, tail: &str) -> PathBuf {
        let path = self.root.join(format!("{marker}-planted"));
        self.script(&path, marker, tail);
        path
    }

    /// The workspace with one commit and nothing planted.
    fn unplanted() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("host-git-hardening-{}-{nanos}", std::process::id()));
        let workspace = root.join("workspace");
        let markers = root.join("markers");
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        std::fs::create_dir_all(&markers).expect("create the marker directory");
        let fixture = Self {
            root,
            workspace,
            markers,
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(fixture.workspace.join("check.txt"), "broken\n").expect("write check.txt");
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "a failing check"]);
        fixture
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.workspace.join(file)).expect("read a workspace file")
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// The markers written so far, by name, sorted.
    fn markers(&self) -> Vec<String> {
        let mut found: Vec<String> = std::fs::read_dir(&self.markers)
            .expect("read the marker directory")
            .map(|entry| {
                entry
                    .expect("a marker entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        found.sort();
        found
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn git_raw(&self, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("run git")
    }

    fn git(&self, args: &[&str]) -> String {
        let output = self.git_raw(args);
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
