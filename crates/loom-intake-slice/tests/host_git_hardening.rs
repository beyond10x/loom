//! Loom's host-side git runs no hooks and no fsmonitor (story `host-git-hardening`; design
//! `effect-isolation`, decision 3 and threat T4).
//!
//! `.git` sits inside the work tree a test command may write, so the code a model edited can plant
//! a hook in `.git/hooks` or a `core.fsmonitor` command in `.git/config`, and the next git call
//! Loom makes on the host would run it as the operator. Every git command Loom runs for a run
//! therefore carries `-c core.hooksPath=<an empty directory Loom owns> -c core.fsmonitor=false`,
//! through one helper.
//!
//! Two cases hold that:
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
use b10x_loom_intake_slice::executor::{LocalExecutor, Report, TestCommand};
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
    );

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

    let mut manifests = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
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
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt as _;

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

        let executable = |path: &Path, marker: &str| {
            let script = format!(
                "#!/bin/sh\necho \"$@\" > '{}'\nexit 0\n",
                fixture.markers.join(marker).display()
            );
            std::fs::write(path, script).expect("write a planted command");
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
                .expect("make a planted command executable");
        };
        let hooks = fixture.workspace.join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("create .git/hooks");
        for hook in HOOKS {
            executable(&hooks.join(hook), hook);
        }
        let fsmonitor = fixture.workspace.join(".git").join("fsmonitor-planted");
        executable(&fsmonitor, FSMONITOR);
        fixture.git(&["config", "core.hooksPath", &hooks.to_string_lossy()]);
        fixture.git(&["config", "core.fsmonitor", &fsmonitor.to_string_lossy()]);
        assert!(
            fixture.markers().is_empty(),
            "planting fired a marker: {:?}",
            fixture.markers()
        );
        fixture
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
