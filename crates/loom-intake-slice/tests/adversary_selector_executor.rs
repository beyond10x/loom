//! Adversary cases for `b10x_loom_intake_slice::executor` and `b10x_loom_intake_slice::verifier` (story
//! `selector-executor`, wave 2026-10-04-w17).
//!
//! The executor's module documentation promises: "An edit is checked whole before any file is
//! written: one refused path refuses the edit, and nothing is written", and names only a disk error
//! as the way an edit can fail part-way. These cases are edits that pass every check the executor
//! makes and then fail after their files are written, each for a reason the model or the workspace
//! supplies: a pre-commit hook that rejects the commit, a NUL in the commit message, a second file
//! whose parent is the first, a path git refuses to add, and a path the workspace ignores.
//!
//! Fixture repositories live under `CARGO_TARGET_TMPDIR`. The fixtures' own git calls run with no
//! system or global configuration; each fixture sets a local identity, disables signing and points
//! `core.hooksPath` at its own hooks directory, so the executor's git (which reads the workspace's
//! configuration) sees the fixture's settings over the operator's.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use b10x_loom_commission::model::json as cjson;
use b10x_loom_commission::model::responsibility::{
    CaseId, EvidenceData, ExecutorOutcomeProposedAction, ProposedActionArguments,
};
use b10x_loom_intake_slice::case;
use b10x_loom_intake_slice::executor::{ExecuteError, LocalExecutor, Report, TestCommand};
use b10x_loom_intake_slice::verifier::TestResultVerifier;
use loom_governor::{CanonGovernor, MemoryCaseStore};
use serde_json::json;

const PICK: &str = "software-change@1";
const INTENT: &str = "make the failing check pass";
const PRODUCER: &str = "intake-slice-adversary-verifier";

/// A workspace whose pre-commit hook rejects every commit (a lint hook, as the pre-commit framework
/// installs) gets an edit of `check.txt`. The commit fails after the file is written and staged;
/// the edit reports an error, and the work tree is left with an uncommitted change, so every later
/// `tests.run` is about no revision until the model happens to rewrite that same file.
///
/// Since story `host-git-hardening` the executor's git runs no workspace hook
/// (`core.hooksPath=/dev/null`), so this hook never runs, the commit succeeds
/// and the case no longer reaches the rollback path. The NUL-message and git-refuses cases below
/// still cover rollback.
#[cfg(unix)]
#[test]
fn adversary_an_edit_its_pre_commit_hook_rejects_leaves_the_work_tree_untouched() {
    let fixture = Fixture::new("hook", &[("check.txt", "broken\n")]);
    fixture.hook(
        "pre-commit",
        "#!/bin/sh\necho 'lint: rejected' >&2\nexit 1\n",
    );
    let (governor, case) = fixture.open();
    let executor = fixture.executor(&governor, &case, grep_check());

    let result = executor.execute(&edit(
        json!([{"path": "check.txt", "contents": "fixed\n"}]),
        Some("fix the check"),
    ));
    fixture.assert_untouched("a commit its pre-commit hook rejects", &result);
}

/// A commit message is a model-written string; one holding a NUL passes the executor's check
/// (non-empty after trimming) and reaches `git commit --message`, which cannot be started with a NUL
/// in its arguments. By then the file is written and staged.
#[test]
fn adversary_an_edit_with_a_nul_in_its_message_leaves_the_work_tree_untouched() {
    let fixture = Fixture::new("nul-message", &[("check.txt", "broken\n")]);
    let (governor, case) = fixture.open();
    let executor = fixture.executor(&governor, &case, grep_check());

    let result = executor.execute(&edit(
        json!([{"path": "check.txt", "contents": "fixed\n"}]),
        Some("fix\u{0}the check"),
    ));
    fixture.assert_untouched("a message holding a NUL", &result);
}

/// Two files that each pass the path check, where the second's parent is the first: the first is
/// written, then creating the second's parent directory fails.
#[test]
fn adversary_an_edit_whose_later_file_cannot_be_written_leaves_the_work_tree_untouched() {
    let fixture = Fixture::new("file-then-child", &[("check.txt", "broken\n")]);
    let (governor, case) = fixture.open();
    let executor = fixture.executor(&governor, &case, grep_check());

    let result = executor.execute(&edit(
        json!([
            {"path": "check.txt", "contents": "fixed\n"},
            {"path": "check.txt/inner.txt", "contents": "x\n"}
        ]),
        Some("two files"),
    ));
    fixture.assert_untouched("a second file under the first", &result);
}

/// `.git.` is not `.git` to the executor's check, and git (core.protectNTFS, on by default) refuses
/// to add it. The file is written first, so it is left behind untracked.
#[test]
fn adversary_an_edit_git_refuses_to_add_leaves_the_work_tree_untouched() {
    let fixture = Fixture::new("git-refuses", &[("check.txt", "broken\n")]);
    let (governor, case) = fixture.open();
    let executor = fixture.executor(&governor, &case, grep_check());

    let result = executor.execute(&edit(
        json!([
            {"path": "check.txt", "contents": "fixed\n"},
            {"path": ".git.", "contents": "x\n"}
        ]),
        Some("a path git refuses"),
    ));
    fixture.assert_untouched("a path git refuses to add", &result);
}

/// The workspace ignores `*.local`, and its check reads `settings.local` (a local settings file, as
/// `.env` files are read by test suites). An edit writes `settings.local`; `git add` refuses the
/// ignored path, so the edit fails, but the file stays on disk. `git status --porcelain` does not
/// list ignored files, so the next `tests.run` calls the tree clean and runs the check on HEAD plus
/// the file the failed edit left. The verifier then submits `pass` about a revision that does not
/// hold the file the check read: evidence a clean checkout of that revision does not reproduce.
#[test]
fn adversary_a_failed_edit_of_an_ignored_file_yields_no_pass_about_a_revision_without_it() {
    let fixture = Fixture::new(
        "ignored",
        &[("check.txt", "broken\n"), (".gitignore", "*.local\n")],
    );
    let (governor, case) = fixture.open();
    let executor = fixture.executor(
        &governor,
        &case,
        TestCommand::new("grep", ["-qx", "fixed", "settings.local"]),
    );
    let verifier = TestResultVerifier::new(&governor, case.clone(), PRODUCER);

    let edited = executor.execute(&edit(
        json!([{"path": "settings.local", "contents": "fixed\n"}]),
        Some("local settings"),
    ));
    let run = executor
        .execute(&proposal("tests.run", &json!({})))
        .expect("tests.run is performed");
    let verified = verifier.verify(&run);

    let held = evidence(&governor, &case);
    for record in held
        .iter()
        .filter(|record| text(&record.facts, "result") == "pass")
    {
        let revision = text(&record.facts, "subject_revision");
        let shown = fixture.git_raw(&["show", &format!("{revision}:settings.local")]);
        assert!(
            shown.status.success() && shown.stdout == b"fixed\n",
            "the verifier submitted `pass` about {revision}, which does not hold the \
             `settings.local` the check read (edit: {edited:?}; verify: {verified:?}; \
             on disk: {:?}; status: {:?})",
            std::fs::read_to_string(fixture.workspace().join("settings.local")).ok(),
            fixture.status(),
        );
    }
}

fn grep_check() -> TestCommand {
    TestCommand::new("grep", ["-qx", "fixed", "check.txt"])
}

fn edit(files: serde_json::Value, message: Option<&str>) -> ExecutorOutcomeProposedAction {
    let mut arguments = json!({ "files": files });
    if let Some(message) = message {
        arguments["message"] = json!(message);
    }
    proposal("repository.edit", &arguments)
}

fn proposal(action: &str, arguments: &serde_json::Value) -> ExecutorOutcomeProposedAction {
    ExecutorOutcomeProposedAction {
        action: action.to_owned(),
        arguments: ProposedActionArguments(
            cjson::parse(&arguments.to_string()).expect("arguments are JSON"),
        ),
    }
}

fn evidence(governor: &CanonGovernor<MemoryCaseStore>, case: &CaseId) -> Vec<EvidenceData> {
    governor
        .evidence(case)
        .expect("the governor holds the case")
}

fn text<'a>(value: &'a cjson::Value, member: &str) -> &'a str {
    match value.member(member) {
        Some(cjson::Value::Text(text)) => text,
        other => panic!("`{member}` is not text: {other:?}"),
    }
}

/// A scratch git repository with a local identity, its own hooks directory and one commit.
struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    first: String,
}

impl Fixture {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adversary-selector-executor-{name}-{}-{nanos}",
            std::process::id()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        let mut fixture = Self {
            root,
            workspace,
            first: String::new(),
        };
        fixture.git(&["init", "--quiet", "--initial-branch=main"]);
        fixture.git(&["config", "user.name", "Fixture Author"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        let hooks = fixture.hooks();
        std::fs::create_dir_all(&hooks).expect("create the hooks directory");
        fixture.git(&["config", "core.hooksPath", &hooks.to_string_lossy()]);
        for (path, contents) in files {
            std::fs::write(fixture.workspace.join(path), contents).expect("write a fixture file");
        }
        fixture.git(&["add", "--all"]);
        fixture.git(&["commit", "--quiet", "--message", "first"]);
        fixture.first = fixture.head();
        fixture
    }

    fn workspace(&self) -> &Path {
        &self.workspace
    }

    fn hooks(&self) -> PathBuf {
        self.workspace.join(".git").join("hooks")
    }

    #[cfg(unix)]
    fn hook(&self, name: &str, script: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        let path = self.hooks().join(name);
        std::fs::write(&path, script).expect("write the hook");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make the hook executable");
    }

    fn open(&self) -> (CanonGovernor<MemoryCaseStore>, CaseId) {
        let governor = CanonGovernor::new(MemoryCaseStore::default());
        let case = case::open(&governor, PICK, INTENT, &self.workspace).expect("the case opens");
        (governor, case)
    }

    fn executor<'g>(
        &self,
        governor: &'g CanonGovernor<MemoryCaseStore>,
        case: &CaseId,
        test: TestCommand,
    ) -> LocalExecutor<'g, MemoryCaseStore> {
        LocalExecutor::new(governor, case.clone(), &self.workspace, test)
    }

    /// After an edit: whatever the executor answered, the work tree holds no uncommitted change;
    /// an edit it refused moved nothing and left `check.txt` as it was.
    fn assert_untouched(&self, what: &str, result: &Result<Report, ExecuteError>) {
        let status = self.status();
        let on_disk = std::fs::read_to_string(self.workspace.join("check.txt")).ok();
        assert_eq!(
            status, "",
            "{what}: the edit answered {result:?} and left the work tree with uncommitted \
             changes (check.txt on disk: {on_disk:?})"
        );
        if result.is_err() {
            assert_eq!(self.head(), self.first, "{what}: a refused edit committed");
            assert_eq!(
                on_disk.as_deref(),
                Some("broken\n"),
                "{what}: a refused edit wrote check.txt ({result:?})"
            );
        }
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn status(&self) -> String {
        self.git(&["status", "--porcelain", "--untracked-files=all"])
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
