//! Adversary cases for story `host-git-hardening` (design `effect-isolation`, decision 3, T4).
//!
//! The claim under attack: no git process Loom starts on the host runs code the workspace, or a
//! test writing into the workspace, planted. `crate::git` holds it with three overrides (hooks,
//! fsmonitor, signing) plus a digest of `config`, `config.worktree` and `info/attributes` taken
//! when the case opens. Each case below plants a program-running key by a route that digest does
//! not see, then asks Loom for an ordinary `repository.edit`, and asserts the planted program
//! wrote no marker.
//!
//! Fixture repositories live under `CARGO_TARGET_TMPDIR`; markers are written to a sibling of the
//! work tree; nothing touches the network. Unix-only, as the planted programs are shell scripts.

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

/// A second `b10x-loom run` on the same workspace opens a new case, and `case::open` records the
/// workspace's git configuration as it then is (`git.rs` `record`, "Opening a case again on the
/// same workspace records it again"). Run 1's test plants a clean filter in `.git/config` and a
/// `.gitattributes` selecting it; run 1's edit is refused, as the unit's own case shows. Run 2
/// then opens on the same directory, the planted configuration becomes the recording, and run 2's
/// edit hands `check.txt` to the planted filter through `git add`.
///
/// The story's acceptance frames the threat the same way: "a run whose workspace carries ... a
/// `core.fsmonitor` command in `.git/config`" at the start of the run. Only the three overridden
/// keys are neutralised for configuration that is already there when a case opens.
#[test]
fn adversary_a_second_run_on_a_workspace_a_test_planted_runs_none_of_the_plant() {
    let fixture = Fixture::new();
    let clean = fixture.planted("filter-clean", "cat");
    let plant = format!(
        "printf '[filter \"x\"]\\n\\tclean = %s\\n' '{}' >> .git/config \
         && echo 'check.txt filter=x' > .gitattributes",
        clean.display()
    );

    // Run 1: the test plants; the next host git call is refused, as the unit intends.
    {
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("run 1 opens");
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
            .expect("run 1's tests.run is performed");
        assert!(matches!(run, Report::TestsRun(_)), "{run:?}");
        let edited = executor.execute(&edit());
        assert!(
            matches!(
                edited,
                Err(ExecuteError::HostGit(HostGitRefusal::ConfigChanged { .. }))
            ),
            "run 1's edit is refused: {edited:?}"
        );
        assert!(fixture.markers().is_empty(), "{:?}", fixture.markers());
    }

    // Run 2, a fresh `b10x-loom run --workspace` on the same directory.
    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let opened = case::open(&governor, PICK, INTENT, fixture.workspace());
    let edited = opened.as_ref().ok().map(|case| {
        LocalExecutor::new(
            &governor,
            case.clone(),
            fixture.workspace(),
            TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
        )
        .with_runner(std::sync::Arc::new(
            b10x_loom_intake_slice::executor::UnconfinedRunner,
        ))
        .execute(&edit())
    });
    assert_eq!(
        fixture.markers(),
        Vec::<String>::new(),
        "run 2's host git ran the filter run 1's test planted in `.git/config` \
         (open: {opened:?}, edit: {edited:?})"
    );
}

/// `record` digests the files `git rev-parse --git-path` named when the case opened. A test that
/// writes `.git/commondir` moves git's common directory, and with it the `config` git reads,
/// without changing one byte of the recorded `.git/config`. Here the new common directory is a
/// copy of `.git` under `target/` (the one directory the confined test runner is planned to leave
/// writable) with a clean filter added to its `config`.
#[test]
fn adversary_a_commondir_a_test_plants_moves_the_config_past_the_digest() {
    let fixture = Fixture::new();
    let clean = fixture.planted("filter-clean", "cat");
    let common = fixture.workspace().join("target").join("common");
    let plant = format!(
        "mkdir -p target && cp -R .git '{common}' \
         && printf '[filter \"x\"]\\n\\tclean = %s\\n' '{clean}' >> '{common}/config' \
         && echo 'check.txt filter=x' > .gitattributes \
         && echo '{common}' > .git/commondir",
        common = common.display(),
        clean = clean.display()
    );
    let recorded_config = fixture.read(".git/config");

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
    assert_eq!(
        fixture.read(".git/config"),
        recorded_config,
        "the recorded `.git/config` is byte for byte what the case opened on"
    );
    assert!(fixture.markers().is_empty(), "planting fired a marker");

    let edited = executor.execute(&edit());
    assert_eq!(
        fixture.markers(),
        Vec::<String>::new(),
        "Loom's host git read the planted `.git/commondir` and ran its filter (edit: {edited:?})"
    );
    assert!(
        matches!(edited, Err(ExecuteError::HostGit(_))),
        "the edit is refused as host git: {edited:?}"
    );
}

/// A repository that shares its configuration in the work tree (`git config include.path
/// ../.gitconfig`, a known pattern) makes a work-tree file part of the configuration. `record`
/// digests `.git/config`, which holds only the `include`, not the file it includes. The model
/// itself, through `repository.edit` and with no test involved, writes `.gitconfig` with a clean
/// filter and `.gitattributes` selecting it; the same edit's `git add` then runs the filter.
#[test]
fn adversary_an_included_work_tree_config_the_model_edits_runs_nothing() {
    let fixture = Fixture::new();
    fixture.git(&["config", "include.path", "../.gitconfig"]);
    let clean = fixture.planted("filter-clean", "cat");

    let governor = CanonGovernor::new(MemoryCaseStore::default());
    let case = case::open(&governor, PICK, INTENT, fixture.workspace()).expect("the case opens");
    let executor = LocalExecutor::new(
        &governor,
        case,
        fixture.workspace(),
        TestCommand::new("grep", ["-qx", "fixed", "check.txt"]),
    )
    .with_runner(std::sync::Arc::new(
        b10x_loom_intake_slice::executor::UnconfinedRunner,
    ));
    let edited = executor.execute(&proposal(
        "repository.edit",
        &json!({
            "files": [
                {"path": ".gitconfig", "contents": format!("[filter \"x\"]\n\tclean = {}\n", clean.display())},
                {"path": ".gitattributes", "contents": "check.txt filter=x\n"},
                {"path": "check.txt", "contents": "fixed\n"}
            ],
            "message": "fix the check"
        }),
    ));
    assert_eq!(
        fixture.markers(),
        Vec::<String>::new(),
        "Loom's host git ran a filter the model wrote into an included work-tree config \
         (edit: {edited:?})"
    );
}

fn edit() -> ExecutorOutcomeProposedAction {
    proposal(
        "repository.edit",
        &json!({
            "files": [{"path": "check.txt", "contents": "fixed\n"}],
            "message": "fix the check"
        }),
    )
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

/// A scratch repository with a local identity and one failing check; markers go to a sibling
/// directory of the work tree.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    markers: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary-host-git-{}-{made}-{nanos}",
            std::process::id()
        ));
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

    /// An executable script outside the work tree that records `marker` and then runs `tail`.
    fn planted(&self, marker: &str, tail: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;

        let path = self.root.join(format!("{marker}-planted"));
        let script = format!(
            "#!/bin/sh\necho \"$@\" > '{}'\n{tail}\n",
            self.markers.join(marker).display()
        );
        std::fs::write(&path, script).expect("write a planted command");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make a planted command executable");
        path
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.workspace.join(file)).expect("read a workspace file")
    }

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
