//! Adversary cases for `story:session-executor-stable-copy-path` (wave 2026-10-30-w3, unit C):
//! the copy name's byte bound, a leftover the user owns but cannot remove as it stands, links at
//! every level of a leftover, a session whose observer panics, two spellings of one scratch, and
//! a copy name that is a running unnamed session's root.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and its
//! hook requests print): no harness program runs, no model is called and nothing reaches the
//! network. Scratch lives under the system's temporary directory, as in `stable_copy_path.rs`.

use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, SuspensionReason,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::protocol::{Event, EventLine};
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{CredentialSource, Kind, ProcessRunner, SessionExecutor};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Gives every directory under `path` (links not followed) and `path` itself `rwx` for the
/// owner, so that a tree a case made unremovable can be removed.
fn writable(path: &Path) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            writable(&entry.path());
        }
    }
}

/// `tree/` the workspace with `src/lib.rs`, `scratch/` for the copies, `outside/` holding
/// `keep.txt`; removed when dropped, after every directory under it is made writable again.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-adversary-w3c-{}-{name}",
            std::process::id()
        ));
        writable(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        std::fs::create_dir_all(dir.join("outside/sub")).expect("a directory outside scratch");
        std::fs::write(dir.join("outside/keep.txt"), "keep\n").expect("a file outside scratch");
        std::fs::write(dir.join("outside/sub/keep.txt"), "keep\n").expect("a file outside");
        Self(dir)
    }

    fn scratch(&self) -> PathBuf {
        self.0.join("scratch")
    }

    fn outside(&self) -> PathBuf {
        self.0.join("outside")
    }

    fn executor(&self, copy_name: Option<&str>, sessions: &Sessions) -> SessionExecutor {
        self.executor_in(self.scratch(), copy_name, sessions)
    }

    fn executor_in(
        &self,
        scratch: PathBuf,
        copy_name: Option<&str>,
        sessions: &Sessions,
    ) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(Box::new(sessions.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = scratch;
        executor.copy_name = copy_name.map(str::to_owned);
        executor
    }

    fn scratch_entries(&self) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(self.scratch())
            .expect("read scratch")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        entries.sort();
        entries
    }

    /// The files under `outside/`, each with its bytes.
    fn outside_files(&self) -> Vec<(String, String)> {
        let mut files = Vec::new();
        listed(&self.outside(), &self.outside(), &mut files);
        files.sort();
        files
            .into_iter()
            .filter_map(|file| {
                std::fs::read_to_string(self.outside().join(&file))
                    .ok()
                    .map(|bytes| (file, bytes))
            })
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        writable(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

type OnStart = Rc<dyn Fn(&Path)>;

/// A scripted Codex session; the working directory of each start is recorded in `started`,
/// and `on_start`, when set, runs in the copy first, as the session's own commands would.
#[derive(Clone, Default)]
struct Sessions {
    started: Rc<RefCell<Vec<PathBuf>>>,
    on_start: Option<OnStart>,
}

impl ProcessRunner for Sessions {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        self.started.borrow_mut().push(plan.cwd.to_path_buf());
        if let Some(on_start) = &self.on_start {
            on_start(plan.cwd);
        }
        ScriptedRunner::new(rollout(plan.cwd), ScriptedLog::new()).start(plan)
    }
}

fn listed(root: &Path, dir: &Path, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("an entry").path();
        into.push(
            path.strip_prefix(root)
                .expect("under the root")
                .display()
                .to_string(),
        );
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_dir()) {
            listed(root, &path, into);
        }
    }
}

fn hook(key: &str, tool: &str, input: &serde_json::Value, cwd: &Path) -> [ScriptStep; 2] {
    let raw = serde_json::json!({
        "session_id": "codex-1", "turn_id": "t1", "cwd": cwd.display().to_string(),
        "hook_event_name": "PreToolUse", "model": "scripted", "permission_mode": "default",
        "tool_name": tool, "tool_input": input, "tool_use_id": key,
    });
    let line = serde_json::json!({
        "metaharness.codex/1": "hook_request", "key": key, "input": raw,
    });
    [
        ScriptStep::line(line.to_string()),
        ScriptStep::awaiting(key),
    ]
}

fn rollout(cwd: &Path) -> Vec<ScriptStep> {
    let meta = serde_json::json!({
        "timestamp": "2026-08-22T10:00:00.000Z", "type": "session_meta",
        "payload": {"id": "codex-1", "session_id": "codex-1", "cli_version": "0.153.4",
                    "cwd": cwd.display().to_string(), "originator": "codex_exec",
                    "model_provider": "openai"},
    });
    let started = serde_json::json!({
        "timestamp": "2026-08-22T10:00:01.000Z", "type": "event_msg",
        "payload": {"type": "task_started", "turn_id": "t1"},
    });
    let tokens = serde_json::json!({
        "timestamp": "2026-08-22T10:00:04.000Z", "type": "event_msg",
        "payload": {"type": "token_count", "info": {"total_token_usage": {
            "input_tokens": 100, "cached_input_tokens": 40, "output_tokens": 20,
            "reasoning_output_tokens": 6, "total_tokens": 120}}},
    });
    let action = serde_json::json!({"action": EDIT, "summary": "looked"}).to_string();
    let completed = serde_json::json!({
        "timestamp": "2026-08-22T10:00:05.000Z", "type": "event_msg",
        "payload": {"type": "task_complete", "turn_id": "t1", "duration_ms": 4200,
                    "error": null, "last_agent_message": action},
    });
    let mut steps = vec![
        ScriptStep::line(meta.to_string()),
        ScriptStep::line(started.to_string()),
    ];
    steps.extend(hook(
        "call-ls",
        "Bash",
        &serde_json::json!({"command": "ls src"}),
        cwd,
    ));
    steps.push(ScriptStep::line(tokens.to_string()));
    steps.push(ScriptStep::line(completed.to_string()));
    steps
}

fn step_of(executor: &SessionExecutor) -> ExecutorStep {
    let case = CaseId("case-adversary-w3c".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-adversary-w3c".to_owned()),
        authority_context: AuthorityContext(Value::Null),
    });
    let frontier = Frontier::new(FrontierData {
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

fn suspended_why(step: &ExecutorStep) -> Option<String> {
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        return None;
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        return None;
    };
    Some(why.clone())
}

fn proposes(step: &ExecutorStep, what: &str) {
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "{what}: the session runs and proposes: {:?}",
        step.outcome
    );
}

/// What a session does when it fills a Go module cache in its home or copy: a directory tree
/// whose directories are `r-x` (Go makes its module cache read-only), with a file in it.
fn read_only_tree(at: &Path) {
    let module = at.join("go/pkg/mod/example.com/m@v1.0.0");
    std::fs::create_dir_all(&module).expect("a module directory");
    std::fs::write(module.join("go.mod"), "module example.com/m\n").expect("a module file");
    std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o555))
        .expect("the module directory read-only");
}

/// A session whose own commands left a read-only directory in its copy (the shape of Go's
/// module cache): when the call returns, the root `<scratch>/<name>` is gone, as the story says
/// of every call ("the copy is removed when the call returns, as today").
#[test]
fn the_root_is_removed_when_the_session_left_a_read_only_directory() {
    let scratch = Scratch::new("ro-drop");
    let sessions = Sessions {
        on_start: Some(Rc::new(|cwd: &Path| read_only_tree(cwd))),
        ..Sessions::default()
    };

    let executor = scratch.executor(Some("case-17"), &sessions);
    proposes(&step_of(&executor), "the session");

    assert!(
        scratch.scratch_entries().is_empty(),
        "the root is removed when the call returns; left in scratch: {:?}",
        scratch.scratch_entries()
    );
}

/// The case's next step, under the same name, after a session left a read-only directory: it
/// runs. Under a copy name, a root the first call could not remove must not suspend every later
/// step of the case.
#[test]
fn a_read_only_directory_a_session_left_does_not_wedge_its_copy_name() {
    let scratch = Scratch::new("ro-wedge");
    let first = Sessions {
        on_start: Some(Rc::new(|cwd: &Path| read_only_tree(cwd))),
        ..Sessions::default()
    };
    let executor = scratch.executor(Some("case-17"), &first);
    proposes(&step_of(&executor), "the first step");

    let second = Sessions::default();
    let executor = scratch.executor(Some("case-17"), &second);
    let step = step_of(&executor);
    proposes(&step, "the next step of the case under the same name");
}

/// A leftover of the user's own whose root is mode `0000` and whose copy holds a read-only
/// directory: the story's "a directory left at that path by an earlier session of the same user
/// ... is removed first", so the session runs.
#[test]
fn a_leftover_holding_a_read_only_directory_is_emptied() {
    leftover_is_emptied("ro-inner", 0o700);
}

/// The same with the leftover's root itself made `0000` by the session (`chmod 000 ..`).
#[test]
fn a_leftover_root_of_mode_0000_is_emptied() {
    leftover_is_emptied("ro-root", 0o000);
}

fn leftover_is_emptied(case: &str, root_mode: u32) {
    let scratch = Scratch::new(case);
    let leftover = scratch.scratch().join("case-17");
    std::fs::create_dir_all(leftover.join("copy")).expect("a leftover copy");
    read_only_tree(&leftover.join("home"));
    std::fs::set_permissions(&leftover, std::fs::Permissions::from_mode(root_mode))
        .expect("the leftover root's mode");

    let sessions = Sessions::default();
    let executor = scratch.executor(Some("case-17"), &sessions);
    let step = step_of(&executor);
    proposes(
        &step,
        &format!("{case}: a leftover root of mode {root_mode:o}"),
    );
    assert!(
        scratch.scratch_entries().is_empty(),
        "{case}: nothing is left in scratch: {:?}",
        scratch.scratch_entries()
    );
}

/// A leftover whose entries are links at every level (the root's `copy` itself a link to a
/// directory outside scratch, `home` holding a link to a directory and one to a file, a dangling
/// link, a deep directory holding a link): it is emptied, the session runs, and nothing outside
/// scratch is touched.
#[test]
fn a_leftover_holding_links_at_every_level_is_emptied_without_following_them() {
    let scratch = Scratch::new("links");
    let outside_before = scratch.outside_files();
    let leftover = scratch.scratch().join("case-17");
    std::fs::create_dir_all(leftover.join("home")).expect("a leftover home");
    std::os::unix::fs::symlink(scratch.outside(), leftover.join("copy")).expect("copy, a link");
    std::os::unix::fs::symlink(scratch.outside(), leftover.join("home/dir")).expect("a link");
    std::os::unix::fs::symlink(
        scratch.outside().join("keep.txt"),
        leftover.join("home/file"),
    )
    .expect("a link to a file");
    std::os::unix::fs::symlink(scratch.0.join("absent"), leftover.join("dangling"))
        .expect("a dangling link");
    let mut deep = leftover.join("git");
    for level in 0..200 {
        deep.push(format!("d{level}"));
    }
    std::fs::create_dir_all(&deep).expect("a deep tree");
    std::os::unix::fs::symlink(scratch.outside().join("sub"), deep.join("sub"))
        .expect("a deep link");

    let sessions = Sessions::default();
    let executor = scratch.executor(Some("case-17"), &sessions);
    proposes(&step_of(&executor), "the session over a leftover of links");

    assert_eq!(
        scratch.outside_files(),
        outside_before,
        "nothing outside scratch is touched"
    );
    assert!(scratch.scratch_entries().is_empty(), "nothing is left");
}

/// A session whose observer panics: the panic reaches the caller, the root is removed on the
/// way out and its lock released, so the case's next step under the same name runs.
#[test]
fn a_session_whose_observer_panics_leaves_its_copy_name_free() {
    let scratch = Scratch::new("panic");
    let sessions = Sessions::default();
    let executor =
        scratch
            .executor(Some("case-17"), &sessions)
            .with_observer(|line: &EventLine, _: &Path| {
                if matches!(line.event, Event::ToolRequested { .. }) {
                    panic!("the observer panics");
                }
            });
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step_of(&executor)));
    assert!(unwound.is_err(), "the observer's panic reaches the caller");
    drop(executor);
    assert!(
        scratch.scratch_entries().is_empty(),
        "the root is removed on the way out: {:?}",
        scratch.scratch_entries()
    );

    let executor = scratch.executor(Some("case-17"), &Sessions::default());
    proposes(&step_of(&executor), "the next step after a panic");
}

/// The 64-byte bound counts bytes: 64 bytes of two-byte characters are one name, 66 are not.
/// DEL and NUL are control characters.
#[test]
fn the_copy_names_bound_counts_bytes_and_refuses_del_and_nul() {
    let scratch = Scratch::new("bytes");
    let sessions = Sessions::default();
    let at_bound = "é".repeat(32);
    assert_eq!(at_bound.len(), 64);
    let over = "é".repeat(33);
    for name in [over.as_str(), "case\u{7f}17", "case\u{0}17", "case\u{85}17"] {
        let executor = scratch.executor(Some(name), &sessions);
        let why = suspended_why(&step_of(&executor))
            .unwrap_or_else(|| panic!("{name:?} suspends the step"));
        assert!(why.contains("copy name"), "{name:?}: {why}");
        assert!(
            sessions.started.borrow().is_empty(),
            "no session for {name:?}"
        );
        assert!(
            scratch.scratch_entries().is_empty(),
            "nothing made for {name:?}"
        );
    }
    let executor = scratch.executor(Some(&at_bound), &sessions);
    proposes(&step_of(&executor), "a name of 64 bytes in 32 characters");
}

/// Two executors that spell one scratch differently (`scratch` and `scratch/.`) hand the
/// harness the same working directory, and the second is refused while the first runs.
#[test]
fn two_spellings_of_one_scratch_give_one_path_and_one_lock() {
    let scratch = Scratch::new("spellings");
    let sessions = Sessions::default();
    let dotted = scratch.scratch().join(".");
    let second = scratch.executor_in(dotted.clone(), Some("case-17"), &sessions);
    let refused: Rc<RefCell<Option<Option<String>>>> = Rc::default();
    let first = {
        let refused = Rc::clone(&refused);
        scratch.executor(Some("case-17"), &sessions).with_observer(
            move |line: &EventLine, _: &Path| {
                if matches!(line.event, Event::ToolRequested { .. }) && refused.borrow().is_none() {
                    *refused.borrow_mut() = Some(suspended_why(&step_of(&second)));
                }
            },
        )
    };
    proposes(&step_of(&first), "the first");
    let refused = refused
        .borrow_mut()
        .take()
        .expect("the second call was made");
    assert!(
        refused.is_some(),
        "the second spelling is refused while the first runs"
    );

    let third = scratch.executor_in(dotted, Some("case-17"), &sessions);
    proposes(&step_of(&third), "the dotted spelling alone");
    let started = sessions.started.borrow();
    assert_eq!(started.len(), 2, "two sessions started: {started:?}");
    assert_eq!(
        started[0], started[1],
        "one working directory for both spellings"
    );
}

/// What the running unnamed session's observer saw of a named call made under its root's name.
struct Collision {
    outcome: ExecutorStep,
    before: Vec<String>,
    after: Vec<String>,
}

/// A host's copy name that happens to equal a running unnamed session's root
/// (`session-<pid>-<nanos>-<n>`, a directory of this user's under the same scratch, which takes
/// no lock): the named call is refused and the running session's copy is left as it is.
#[test]
fn a_copy_name_equal_to_a_running_unnamed_sessions_root_is_refused() {
    let scratch = Scratch::new("collision");
    let sessions = Sessions::default();
    let seen: Rc<RefCell<Option<Collision>>> = Rc::default();
    let unnamed = {
        let seen = Rc::clone(&seen);
        let scratch_dir = scratch.scratch();
        let tree = scratch.0.join("tree");
        let sessions = sessions.clone();
        scratch
            .executor(None, &sessions)
            .with_observer(move |line: &EventLine, copy: &Path| {
                if matches!(line.event, Event::ToolRequested { .. }) && seen.borrow().is_none() {
                    let name = copy
                        .parent()
                        .and_then(Path::file_name)
                        .expect("the unnamed root")
                        .to_string_lossy()
                        .into_owned();
                    let mut named =
                        SessionExecutor::new(tree.clone(), vec!["src/lib.rs".to_owned()], "fix a")
                            .of_kind(Kind::Codex)
                            .with_runner(Box::new(sessions.clone()));
                    named.credentials = CredentialSource::None;
                    named.scratch = scratch_dir.clone();
                    named.copy_name = Some(name);
                    let mut before = Vec::new();
                    listed(copy, copy, &mut before);
                    before.sort();
                    let outcome = step_of(&named);
                    let mut after = Vec::new();
                    listed(copy, copy, &mut after);
                    after.sort();
                    *seen.borrow_mut() = Some(Collision {
                        outcome,
                        before,
                        after,
                    });
                }
            })
    };
    let first = step_of(&unnamed);

    let Collision {
        outcome,
        before,
        after,
    } = seen.borrow_mut().take().expect("the named call was made");
    assert_eq!(
        before, after,
        "the running unnamed session's copy is left as it is"
    );
    assert!(
        suspended_why(&outcome).is_some(),
        "the named call is refused: {:?}",
        outcome.outcome
    );
    proposes(&first, "the running unnamed session");
}
