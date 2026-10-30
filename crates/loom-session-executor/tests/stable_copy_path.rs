//! Acceptance for `story:session-executor-stable-copy-path`: a host names the session's scratch
//! copy (`SessionExecutor::copy_name`), so every session it runs under that name works in the
//! same path, `<scratch>/<copy_name>/copy`; without a name each session gets a path of its own.
//! The name is one plain path component, else the step suspends before any copy. A directory an
//! earlier session of the same user left at the path is replaced; a link or any other entry there
//! suspends the step and is left as it is. A session that still runs under the name keeps its
//! copy: a second one under the same name is refused.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and its
//! hook requests print): no harness program runs, no model is called and nothing reaches the
//! network. Scratch lives under the system's temporary directory, not under the build directory:
//! inside a checkout, the copy would have the repository's `AGENTS.md` above it, which
//! metaharness's hermetic floor (H11) refuses.

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

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, `scratch/` for the copies, and `outside/`, a directory outside `scratch`
/// holding `keep.txt`; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-stable-copy-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        std::fs::create_dir_all(dir.join("outside")).expect("a directory outside scratch");
        std::fs::write(dir.join("outside/keep.txt"), "keep\n").expect("a file outside scratch");
        Self(dir)
    }

    fn scratch(&self) -> PathBuf {
        self.0.join("scratch")
    }

    fn outside(&self) -> PathBuf {
        self.0.join("outside")
    }

    /// A Codex executor over the workspace, its copy named `copy_name`, its sessions started by
    /// `sessions`.
    fn executor(&self, copy_name: Option<&str>, sessions: &Sessions) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(Box::new(sessions.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = self.scratch();
        executor.copy_name = copy_name.map(str::to_owned);
        executor
    }

    /// The entries of `scratch`, by name, sorted.
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
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a scripted session's start saw: the working directory metaharness handed the child
/// (`RunSpec.cwd`, as the launch plan carries it), the mode of the directory above it, and the
/// files the copy held then, by their paths relative to it.
#[derive(Debug, Clone)]
struct Started {
    cwd: PathBuf,
    root_mode: u32,
    files: Vec<String>,
}

/// A scripted Codex session; each start is recorded in `started`.
#[derive(Clone, Default)]
struct Sessions {
    started: Rc<RefCell<Vec<Started>>>,
}

impl ProcessRunner for Sessions {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let root = plan.cwd.parent().expect("the copy has a root");
        let mut files = Vec::new();
        listed(plan.cwd, plan.cwd, &mut files);
        files.sort();
        self.started.borrow_mut().push(Started {
            cwd: plan.cwd.to_path_buf(),
            root_mode: std::fs::symlink_metadata(root)?.permissions().mode() & 0o7777,
            files,
        });
        ScriptedRunner::new(rollout(plan.cwd), ScriptedLog::new()).start(plan)
    }
}

/// Every entry under `dir`, by its path relative to `root`, links not followed.
fn listed(root: &Path, dir: &Path, into: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read the copy") {
        let path = entry.expect("an entry").path();
        into.push(
            path.strip_prefix(root)
                .expect("under the copy")
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

/// A pinned Codex rollout: its opening record, one call the executor decides (a listing of the
/// copy), the turn's token count, and its completion, whose last message names the action.
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
    let case = CaseId("case-stable-copy".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-stable-copy".to_owned()),
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

/// The step's suspension text, when it suspended for an external availability.
fn refusal(step: &ExecutorStep) -> String {
    let ExecutorOutcome::Suspended(suspended) = &step.outcome else {
        panic!("a refusal: {:?}", step.outcome);
    };
    let SuspensionReason::ExternalAvailability(Value::Text(why)) = &suspended.reason else {
        panic!(
            "suspended for an external availability: {:?}",
            suspended.reason
        );
    };
    why.clone()
}

fn proposes(step: &ExecutorStep) {
    assert!(
        matches!(step.outcome, ExecutorOutcome::ProposedAction(_)),
        "the session runs and proposes: {:?}",
        step.outcome
    );
}

/// Two executors under one name and one scratch run their sessions in the same path,
/// `<scratch>/<name>/copy`, report it as `SessionReport::cwd` and hand it to metaharness as the
/// child's working directory; the copy is gone after each call.
#[test]
fn two_sessions_under_one_copy_name_work_in_the_same_path() {
    let scratch = Scratch::new("same");
    let sessions = Sessions::default();
    let stable = std::fs::canonicalize(scratch.scratch())
        .expect("scratch resolves")
        .join("case-17")
        .join("copy");

    let mut cwds = Vec::new();
    for _ in 0..2 {
        let executor = scratch.executor(Some("case-17"), &sessions);
        proposes(&step_of(&executor));
        let cwd = executor
            .last_report()
            .expect("a report")
            .cwd
            .expect("the session's copy");
        assert!(!cwd.exists(), "the copy is gone when the call returns");
        assert!(
            scratch.scratch_entries().is_empty(),
            "nothing is left in scratch: {:?}",
            scratch.scratch_entries()
        );
        cwds.push(cwd);
    }

    assert_eq!(cwds, [stable.clone(), stable.clone()], "the reported paths");
    let started: Vec<PathBuf> = sessions
        .started
        .borrow()
        .iter()
        .map(|started| started.cwd.clone())
        .collect();
    assert_eq!(
        started,
        [stable.clone(), stable],
        "the working directory metaharness handed each child"
    );
    for started in sessions.started.borrow().iter() {
        assert_eq!(
            started.root_mode, 0o700,
            "the copy's root is the user's own"
        );
    }
}

/// Without a name each session works in a path of its own, as before.
#[test]
fn without_a_copy_name_each_session_gets_its_own_path() {
    let scratch = Scratch::new("unnamed");
    let sessions = Sessions::default();

    let cwds: Vec<PathBuf> = (0..2)
        .map(|_| {
            let executor = scratch.executor(None, &sessions);
            proposes(&step_of(&executor));
            executor
                .last_report()
                .expect("a report")
                .cwd
                .expect("the session's copy")
        })
        .collect();

    assert_ne!(cwds[0], cwds[1], "two sessions, two paths");
    for cwd in &cwds {
        let root = cwd.parent().and_then(Path::file_name).expect("a root");
        assert!(
            root.to_string_lossy().starts_with("session-"),
            "today's unique name: {}",
            cwd.display()
        );
        assert!(!cwd.exists(), "the copy is gone when the call returns");
    }
}

/// A name that is not one plain path component (`a/b`, `..`, `.`, empty, a control character,
/// 65 bytes), or one starting `session-`, the prefix of an unnamed session's root, suspends the
/// step with `ExternalAvailability` before any copy: no session starts and nothing is made in
/// scratch. A name of 64 bytes is one.
#[test]
fn a_copy_name_that_is_not_one_plain_component_is_refused() {
    let scratch = Scratch::new("names");
    let sessions = Sessions::default();
    let long = "a".repeat(65);

    for name in [
        "session-17",
        "a/b",
        "..",
        ".",
        "",
        "case\u{7}17",
        "case\n17",
        long.as_str(),
    ] {
        let executor = scratch.executor(Some(name), &sessions);
        let why = refusal(&step_of(&executor));
        assert!(
            why.contains("copy name"),
            "the refusal names the copy name {name:?}: {why}"
        );
        assert!(
            sessions.started.borrow().is_empty(),
            "no session starts for {name:?}"
        );
        assert!(
            scratch.scratch_entries().is_empty(),
            "nothing is made in scratch for {name:?}: {:?}",
            scratch.scratch_entries()
        );
    }

    let executor = scratch.executor(Some(&"a".repeat(64)), &sessions);
    proposes(&step_of(&executor));
}

/// A directory an earlier session of the same user left at `<scratch>/<name>` is removed and
/// the session runs in a fresh, empty `0700` root: nothing of the leftover reaches the copy, and
/// a link the leftover held is removed, not followed.
#[test]
fn a_leftover_copy_of_the_same_user_is_replaced() {
    let scratch = Scratch::new("leftover");
    let sessions = Sessions::default();
    let leftover = scratch.scratch().join("case-17");
    std::fs::create_dir_all(leftover.join("copy/src")).expect("a leftover copy");
    std::fs::write(leftover.join("copy/stale.txt"), "stale\n").expect("a leftover file");
    std::fs::write(leftover.join("copy/src/lib.rs"), "stale\n").expect("a leftover file");
    std::fs::create_dir_all(leftover.join("home")).expect("a leftover home");
    std::os::unix::fs::symlink(scratch.outside(), leftover.join("copy/out"))
        .expect("a leftover link");
    std::fs::set_permissions(&leftover, std::fs::Permissions::from_mode(0o755))
        .expect("the leftover's mode");

    let executor = scratch.executor(Some("case-17"), &sessions);
    proposes(&step_of(&executor));

    let started = sessions.started.borrow();
    let [started] = started.as_slice() else {
        panic!("one session started: {started:?}");
    };
    assert_eq!(
        started.files,
        ["src", "src/lib.rs"],
        "the copy holds the named file and nothing the leftover held"
    );
    assert_eq!(started.root_mode, 0o700, "the root is made 0700 again");
    assert!(!leftover.exists(), "the root is gone when the call returns");
    assert_eq!(
        std::fs::read_to_string(scratch.outside().join("keep.txt")).expect("kept"),
        "keep\n",
        "the leftover's link was not followed"
    );
}

/// A link at `<scratch>/<name>` (to a directory outside scratch) or a regular file there
/// suspends the step with `ExternalAvailability`, naming the path, before any copy; the entry
/// and the link's target are left as they were.
#[test]
fn a_link_or_foreign_entry_at_the_path_is_refused() {
    let scratch = Scratch::new("foreign");
    let sessions = Sessions::default();
    let linked = scratch.scratch().join("linked");
    std::os::unix::fs::symlink(scratch.outside(), &linked).expect("a link");
    let file = scratch.scratch().join("file");
    std::fs::write(&file, "mine\n").expect("a file");

    for (name, path) in [("linked", &linked), ("file", &file)] {
        let executor = scratch.executor(Some(name), &sessions);
        let why = refusal(&step_of(&executor));
        assert!(
            why.contains(&path.display().to_string()),
            "the refusal names {}: {why}",
            path.display()
        );
        assert!(
            sessions.started.borrow().is_empty(),
            "no session starts under {name}"
        );
    }

    assert!(
        std::fs::symlink_metadata(&linked).is_ok_and(|meta| meta.file_type().is_symlink()),
        "the link is left"
    );
    assert_eq!(
        std::fs::read_link(&linked).expect("the link"),
        scratch.outside(),
        "the link still names its target"
    );
    let mut outside: Vec<String> = std::fs::read_dir(scratch.outside())
        .expect("read outside")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    outside.sort();
    assert_eq!(outside, ["keep.txt"], "the link's target is untouched");
    assert_eq!(
        std::fs::read_to_string(scratch.outside().join("keep.txt")).expect("kept"),
        "keep\n"
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("the file"),
        "mine\n",
        "the file is left"
    );
    assert_eq!(scratch.scratch_entries(), ["file", "linked"]);
}

/// A named copy is made only in a scratch that no other user can write, or that has the sticky
/// bit: in a scratch others can write without it, the step suspends with `ExternalAvailability`
/// naming the scratch, before anything is made there. The same scratch with the sticky bit runs.
#[test]
fn a_scratch_others_can_write_without_the_sticky_bit_is_refused() {
    let scratch = Scratch::new("open-scratch");
    let sessions = Sessions::default();
    std::fs::set_permissions(scratch.scratch(), std::fs::Permissions::from_mode(0o777))
        .expect("a scratch others can write");

    let executor = scratch.executor(Some("case-17"), &sessions);
    let why = refusal(&step_of(&executor));
    assert!(
        why.contains(&scratch.scratch().display().to_string()),
        "the refusal names the scratch: {why}"
    );
    assert!(sessions.started.borrow().is_empty(), "no session starts");
    assert!(
        scratch.scratch_entries().is_empty(),
        "nothing is made in scratch: {:?}",
        scratch.scratch_entries()
    );

    std::fs::set_permissions(scratch.scratch(), std::fs::Permissions::from_mode(0o1777))
        .expect("the sticky bit");
    proposes(&step_of(&scratch.executor(Some("case-17"), &sessions)));
}

/// A call made while the executor already runs a session, here from its observer, is refused
/// before any copy and the running session goes on: the executor's own guard, unchanged under
/// a copy name.
#[test]
fn a_second_session_while_the_first_runs_is_refused() {
    let scratch = Scratch::new("reentry");
    let sessions = Sessions::default();
    let inner: Rc<RefCell<Option<ExecutorStep>>> = Rc::default();
    let executor = Rc::new_cyclic(|this: &std::rc::Weak<SessionExecutor>| {
        let this = this.clone();
        let inner = Rc::clone(&inner);
        scratch.executor(Some("case-17"), &sessions).with_observer(
            move |line: &EventLine, _: &Path| {
                if matches!(line.event, Event::ToolRequested { .. }) && inner.borrow().is_none() {
                    let executor = this.upgrade().expect("the executor");
                    *inner.borrow_mut() = Some(step_of(&executor));
                }
            },
        )
    });

    proposes(&step_of(&executor));

    let inner = inner.borrow_mut().take().expect("the second call was made");
    refusal(&inner);
    assert_eq!(sessions.started.borrow().len(), 1, "one session started");
}

/// What the first session's observer saw of the second executor's call: its refusal, and the
/// first session's copy before and after it.
struct Refused {
    why: String,
    before: Vec<String>,
    after: Vec<String>,
}

/// A second executor under the same name and scratch, called while the first runs a session,
/// is refused before any copy: the running session keeps its copy, unchanged, and goes on to
/// propose.
#[test]
fn a_second_executor_under_the_same_copy_name_while_the_first_runs_is_refused() {
    let scratch = Scratch::new("two-executors");
    let sessions = Sessions::default();
    let second = scratch.executor(Some("case-17"), &sessions);
    let seen: Rc<RefCell<Option<Refused>>> = Rc::default();
    let first = {
        let seen = Rc::clone(&seen);
        scratch.executor(Some("case-17"), &sessions).with_observer(
            move |line: &EventLine, copy: &Path| {
                if matches!(line.event, Event::ToolRequested { .. }) && seen.borrow().is_none() {
                    let mut before = Vec::new();
                    listed(copy, copy, &mut before);
                    before.sort();
                    let why = refusal(&step_of(&second));
                    let mut after = Vec::new();
                    listed(copy, copy, &mut after);
                    after.sort();
                    *seen.borrow_mut() = Some(Refused { why, before, after });
                }
            },
        )
    };

    proposes(&step_of(&first));

    let Refused { why, before, after } =
        seen.borrow_mut().take().expect("the second call was made");
    let root = scratch.scratch().join("case-17");
    assert!(
        why.contains(&root.display().to_string()),
        "the refusal names {}: {why}",
        root.display()
    );
    assert!(
        before.contains(&"src/lib.rs".to_owned()),
        "the first session's copy: {before:?}"
    );
    assert_eq!(before, after, "the first session's copy is unchanged");
    assert_eq!(sessions.started.borrow().len(), 1, "one session started");
    assert!(
        scratch.scratch_entries().is_empty(),
        "nothing is left in scratch: {:?}",
        scratch.scratch_entries()
    );
}
