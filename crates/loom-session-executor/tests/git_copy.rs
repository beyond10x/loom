//! A session's scratch copy carries the workspace's git state, and nothing of it reaches back.
//!
//! Every session here is scripted (metaharness's `ScriptedRunner`): no harness program runs, no
//! model is called and nothing reaches the network. Each test looks at the copy from inside the
//! session, through the runner it is started with, and runs git there and in the workspace.
//! These tests start git themselves, so they live here and not in the crate's sources, where
//! every git a host runs goes through a hardened helper.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, commission_state, frontier_state,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{
    CredentialSource, ProcessRunner, Proposal, ProposedFile, SessionExecutor, TOOLS,
};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// `git` with `args` in `dir`, as a fixed identity and never signing; its standard output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8")
}

/// A directory of the test's own under the system's temporary directory, with `scratch/` in it
/// for the copies; removed when dropped. Not under the repository: metaharness refuses a Claude
/// session whose copy has memory files above it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-git-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The files of `tree` a session's copy holds: tracked, and untracked but not ignored.
fn listed(tree: &Path) -> Vec<String> {
    git(
        tree,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )
    .split('\0')
    .filter(|path| !path.is_empty())
    .map(str::to_owned)
    .collect()
}

/// A repository at `dir` in the object format `objects` and the ref format `refs`: two commits
/// on `work`, a tag, refs packed between them; a config naming a remote, a hook path and an
/// include, and a hook; a change staged, one not, a tracked file removed, a file untracked and
/// one ignored.
fn repository(dir: &Path, objects: &str, refs: &str) {
    let write = |path: &str, text: &str| {
        let at = dir.join(path);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        std::fs::write(at, text).expect("a file");
    };
    std::fs::create_dir_all(dir).expect("the tree");
    git(
        dir,
        &[
            "init",
            "-q",
            "-b",
            "work",
            &format!("--object-format={objects}"),
            &format!("--ref-format={refs}"),
        ],
    );
    write(".gitignore", "target/\n");
    write("README.md", "a story\n");
    write("src/lib.rs", "pub fn a() {}\n");
    write("tests/t.rs", "#[test]\nfn t() {}\n");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "start"]);
    git(dir, &["tag", "v1"]);
    git(dir, &["pack-refs", "--all"]);
    write("src/lib.rs", "pub fn a() -> i32 {\n    1\n}\n");
    git(dir, &["commit", "-q", "-am", "second"]);
    for (key, value) in [
        ("remote.origin.url", "https://example.invalid/r.git"),
        ("core.hooksPath", "hooks-elsewhere"),
        ("include.path", "nowhere.inc"),
    ] {
        git(dir, &["config", key, value]);
    }
    write(".git/hooks/pre-commit", "#!/bin/sh\nexit 1\n");
    write("README.md", "a story, changed\n");
    write("staged.txt", "staged\n");
    git(dir, &["add", "staged.txt"]);
    std::fs::remove_file(dir.join("tests/t.rs")).expect("removed");
    write("notes.txt", "untracked\n");
    write("target/out", "ignored\n");
}

/// Every regular file under `dir`, by its path relative to `root`.
fn walk(root: &Path, dir: &Path, into: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("the directory reads") {
        let path = entry.expect("an entry").path();
        let meta = std::fs::symlink_metadata(&path).expect("its metadata");
        if meta.is_dir() {
            walk(root, &path, into);
        } else if meta.is_file() {
            let relative = path.strip_prefix(root).expect("under the root");
            into.push(relative.to_string_lossy().into_owned());
        }
    }
}

/// Every file under `dir` by its path relative to it, with its bytes and the time it was last
/// written.
fn stamped(dir: &Path) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found
        .into_iter()
        .map(|path| {
            let at = dir.join(&path);
            let written = std::fs::metadata(&at)
                .and_then(|meta| meta.modified())
                .expect("a time");
            (path, (std::fs::read(&at).expect("the file"), written))
        })
        .collect()
}

/// What a test does in the copy while the session runs, given the copy's directory.
type Look = Box<dyn FnMut(&Path)>;

/// A scripted Claude Code session: it runs its [`Look`] in the copy it is started in, then
/// replies naming the edit.
struct Probe(Look);

impl ProcessRunner for Probe {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        (self.0)(plan.cwd);
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": TOOLS, "mcp_servers": [],
            "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "vendor-git",
        });
        let said = serde_json::json!({"action": EDIT, "summary": "looked"}).to_string();
        let reply = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": said}],
                        "usage": {"input_tokens": 1, "output_tokens": 1}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "usage": {"input_tokens": 1, "output_tokens": 1},
        });
        ScriptedRunner::of_lines(
            [init.to_string(), reply.to_string(), result.to_string()],
            ScriptedLog::new(),
        )
        .start(plan)
    }
}

/// One session over the files git lists in `tree`, its copy under `scratch`, `look` run in the
/// copy before the session replies.
fn session(scratch: &Scratch, tree: &Path, look: impl FnMut(&Path) + 'static) -> ExecutorStep {
    let mut executor = SessionExecutor::new(tree, listed(tree), "look at the copy")
        .with_runner(Box::new(Probe(Box::new(look))));
    executor.credentials = CredentialSource::None;
    executor.scratch = scratch.0.join("scratch");
    let case = CaseId("case-git".to_owned());
    let commission = Commission::<commission_state::Assigned>::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-g".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::<frontier_state::Issued>::new(FrontierData {
        frontier_id: FrontierId(uuid(0xf1)),
        case_id: case,
        case_revision: 1,
        claims: Vec::new(),
        obligations: Vec::new(),
        actions: vec![FrontierAction {
            action: EDIT.to_owned(),
            status: ActionStatus::Admissible,
            capability: None,
            reasons: Vec::new(),
        }],
    });
    executor.run_with_usage(&commission, &frontier)
}

/// The files the step's proposed action carries; a step that proposes nothing fails the test.
fn proposed(step: &ExecutorStep) -> Vec<ProposedFile> {
    match &step.outcome {
        ExecutorOutcome::ProposedAction(_) => {
            Proposal::from_outcome(&step.outcome)
                .expect("a proposed action")
                .expect("a proposal's arguments")
                .files
        }
        other => panic!("the session proposed nothing: {other:?}"),
    }
}

/// Acceptance 2: the copy is a git work tree, the copy its top, at the workspace's `HEAD`, on
/// its branch, with its status, staged and unstaged diff and log, in either object format and
/// either ref format.
#[test]
fn the_copy_is_a_work_tree_at_the_workspaces_head() {
    for (objects, refs) in [("sha1", "files"), ("sha256", "files"), ("sha1", "reftable")] {
        let scratch = Scratch::new(&format!("head-{objects}-{refs}"));
        let tree = scratch.0.join("tree");
        repository(&tree, objects, refs);
        let case = format!("{objects} {refs}");
        let status = git(&tree, &["status", "--porcelain"]);
        for line in [" M README.md", "A  staged.txt", " D tests/t.rs"] {
            assert!(status.contains(line), "{case}: the tree's status: {status}");
        }
        let looked = Rc::new(RefCell::new(false));
        let seen = Rc::clone(&looked);
        let workspace = tree.clone();
        let step = session(&scratch, &tree, move |copy| {
            let tree = &workspace;
            assert_eq!(
                git(copy, &["rev-parse", "--is-inside-work-tree"]),
                "true\n",
                "{case}"
            );
            assert_eq!(
                git(copy, &["rev-parse", "--show-toplevel"]).trim_end(),
                copy.to_str().expect("utf-8"),
                "{case}"
            );
            assert_eq!(
                git(copy, &["log", "-1", "--format=%H"]).trim_end(),
                git(tree, &["rev-parse", "HEAD"]).trim_end(),
                "{case}"
            );
            for args in [
                &["status", "--porcelain"][..],
                &["branch", "--show-current"],
                &["log", "--oneline", "-n", "5"],
                &["tag", "--list"],
                &["diff"],
                &["diff", "--cached"],
            ] {
                assert_eq!(
                    git(copy, args),
                    git(tree, args),
                    "{case}: git {args:?} in the copy and in the tree"
                );
            }
            *seen.borrow_mut() = true;
        });
        assert_eq!(proposed(&step), Vec::new(), "{objects} {refs}");
        assert!(*looked.borrow(), "{objects} {refs}: the session ran");
    }
}

/// A linked work tree's copy is at that work tree's own `HEAD` and status, its refs and objects
/// read from the repository it belongs to.
#[test]
fn a_linked_work_trees_copy_is_at_its_own_head() {
    let scratch = Scratch::new("linked");
    let main = scratch.0.join("tree");
    repository(&main, "sha1", "files");
    let linked = scratch.0.join("linked");
    let at = linked.to_str().expect("utf-8");
    git(&main, &["worktree", "add", "-q", "-b", "side", at]);
    std::fs::write(linked.join("side.txt"), "side\n").expect("a file");
    git(&linked, &["add", "side.txt"]);
    git(&linked, &["commit", "-q", "-m", "side"]);
    std::fs::write(linked.join("README.md"), "changed on the side\n").expect("a file");
    let looked = Rc::new(RefCell::new(false));
    let seen = Rc::clone(&looked);
    let (side, base) = (linked.clone(), main.clone());
    let step = session(&scratch, &linked, move |copy| {
        for args in [
            &["status", "--porcelain"][..],
            &["branch", "--show-current"],
            &["log", "-1", "--format=%H"],
        ] {
            assert_eq!(git(copy, args), git(&side, args), "git {args:?}");
        }
        assert_ne!(
            git(copy, &["log", "-1", "--format=%H"]),
            git(&base, &["rev-parse", "HEAD"])
        );
        *seen.borrow_mut() = true;
    });
    assert_eq!(proposed(&step), Vec::new());
    assert!(*looked.borrow(), "the session ran");
}

/// The copy's `.git` is one file naming a git directory beside the copy, outside it, so a
/// session's Glob or Grep there lists that file and never git's own files; the directory has an
/// empty `hooks/`, the workspace's `info/exclude`, its objects as an alternate, and a config
/// naming no remote, include, hook path or monitor; it goes with the copy.
#[test]
fn the_copys_git_state_is_one_file_naming_a_directory_outside_it() {
    let scratch = Scratch::new("shape");
    let tree = scratch.0.join("tree");
    repository(&tree, "sha1", "files");
    assert!(git(&tree, &["config", "--list", "--local"]).contains("remote.origin.url="));
    let objects = std::fs::canonicalize(tree.join(".git/objects")).expect("the objects");
    let exclude = std::fs::read(tree.join(".git/info/exclude")).expect("the tree's");
    let named = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&named);
    let step = session(&scratch, &tree, move |copy| {
        let mut found = Vec::new();
        walk(copy, copy, &mut found);
        let under: Vec<&str> = found
            .iter()
            .map(String::as_str)
            .filter(|path| Path::new(path).starts_with(".git"))
            .collect();
        assert_eq!(under, [".git"]);
        let link = std::fs::read_to_string(copy.join(".git")).expect("a file");
        let gitdir = PathBuf::from(
            link.strip_prefix("gitdir: ")
                .and_then(|rest| rest.strip_suffix('\n'))
                .expect("a gitlink"),
        );
        assert_eq!(gitdir, copy.with_file_name("git"));
        assert!(!gitdir.starts_with(copy));
        assert_eq!(
            std::fs::read_dir(gitdir.join("hooks"))
                .expect("hooks")
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_to_string(gitdir.join("config")).expect("a config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n"
        );
        assert_eq!(
            std::fs::read_to_string(gitdir.join("objects/info/alternates")).expect("alternates"),
            format!("{}\n", objects.display())
        );
        assert_eq!(
            std::fs::read(gitdir.join("info/exclude")).expect("an exclude"),
            exclude
        );
        *seen.borrow_mut() = Some(gitdir);
    });
    assert_eq!(proposed(&step), Vec::new());
    let named = named.borrow().clone().expect("the session ran");
    assert!(!named.exists(), "removed with the copy");
}

/// git run in the copy (a status that may refresh the index, a log, diffs, the branch) writes
/// nothing of the workspace's git directory: every file of it, the index, refs and `HEAD` among
/// them, keeps its bytes and its time of last write.
#[test]
fn git_in_the_copy_never_writes_the_workspaces_git_directory() {
    for refs in ["files", "reftable"] {
        let scratch = Scratch::new(&format!("unwritten-{refs}"));
        let tree = scratch.0.join("tree");
        repository(&tree, "sha1", refs);
        let before = stamped(&tree.join(".git"));
        for path in ["HEAD", "index", "config"] {
            assert!(before.contains_key(path), "{refs}: {path}");
        }
        let step = session(&scratch, &tree, |copy| {
            std::fs::write(copy.join("src/lib.rs"), "changed in the copy\n").expect("a file");
            for args in [
                &["status"][..],
                &["status", "--porcelain"],
                &["--no-optional-locks", "status", "--short"],
                &["log", "--oneline", "-n", "5"],
                &["diff"],
                &["diff", "--cached"],
                &["branch", "--show-current"],
                &["config", "user.name"],
            ] {
                let _ = std::process::Command::new("git")
                    .args(args)
                    .current_dir(copy)
                    .output()
                    .expect("git runs");
            }
        });
        assert_eq!(
            proposed(&step),
            vec![ProposedFile {
                path: "src/lib.rs".to_owned(),
                content: Some("changed in the copy\n".to_owned()),
            }],
            "{refs}"
        );
        assert_eq!(stamped(&tree.join(".git")), before, "{refs}");
    }
}

/// What a session writes into the copy's git state is no proposal's: a `.git` file rewritten, or
/// replaced by a directory with a hook and a config, proposes nothing under `.git`, beside the
/// file the session did change.
#[test]
fn a_change_to_the_copys_git_state_is_left_out_of_the_proposal() {
    let scratch = Scratch::new("changes");
    let tree = scratch.0.join("tree");
    repository(&tree, "sha1", "files");
    let expected = vec![ProposedFile {
        path: "src/lib.rs".to_owned(),
        content: Some("changed\n".to_owned()),
    }];
    let planted = format!("gitdir: {}\n", tree.join(".git").display());
    let rewritten = session(&scratch, &tree, move |copy| {
        std::fs::write(copy.join("src/lib.rs"), "changed\n").expect("a file");
        std::fs::write(copy.join(".git"), &planted).expect("the gitlink rewritten");
    });
    assert_eq!(proposed(&rewritten), expected);
    let replaced = session(&scratch, &tree, |copy| {
        std::fs::write(copy.join("src/lib.rs"), "changed\n").expect("a file");
        std::fs::remove_file(copy.join(".git")).expect("the gitlink removed");
        std::fs::create_dir_all(copy.join(".git/hooks")).expect("a directory");
        std::fs::write(copy.join(".git/hooks/pre-commit"), "#!/bin/sh\n").expect("a hook");
        std::fs::write(copy.join(".git/config"), "[core]\n\tfsmonitor = x\n").expect("a file");
    });
    assert_eq!(proposed(&replaced), expected);
}
