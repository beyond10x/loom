//! Security review of `story:session-executor-stable-copy-path` (loom wave 2026-10-30-w3, unit
//! C): integrity checks of the named scratch root `<scratch>/<copy_name>` against a second party
//! on the same host. The default scratch is the system's temporary directory, which every local
//! user can write.
//!
//! - A directory of another user at the path is refused, naming the path, and left as it is: a
//!   session never runs in a directory it does not own.
//! - The root is checked (`symlink_metadata`, then the locked handle's device and inode) and
//!   then used by its path: emptied entry by entry, and the copy made in it. A swap of the root
//!   for a link between the check and that use must not delete the session user's files
//!   elsewhere, nor put the copy outside the root. A party other than the root's owner can make
//!   that swap in a scratch it can write that has no sticky bit; the test makes it itself, as
//!   the same user, timed on the emptying's first removal.
//!
//! Every session is scripted (metaharness's `ScriptedRunner`, the lines a Codex rollout and its
//! hook requests print): no harness program runs, no model is called and nothing reaches the
//! network. Fixtures live under the system's temporary directory, as in
//! `tests/stable_copy_path.rs`: inside a checkout the copy would have the repository's
//! `AGENTS.md` above it, which metaharness's hermetic floor (H11) refuses.

use std::cell::RefCell;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use b10x_loom_commission::model::json::Value;
use b10x_loom_commission::model::primitives::Uuid;
use b10x_loom_commission::model::responsibility::{
    ActionStatus, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, ExecutorOutcome, ExecutorStep, Frontier, FrontierAction, FrontierData,
    FrontierId, PrincipalId, SuspensionReason,
};
use b10x_loom_commission::ports::executor::AgentExecutor;
use b10x_loom_session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ScriptStep, ScriptedLog, ScriptedRunner,
};
use b10x_loom_session_executor::{CredentialSource, Kind, ProcessRunner, SessionExecutor};

const EDIT: &str = "implementation.edit";

fn uuid(n: u64) -> Uuid {
    Uuid(format!("00000000-0000-4000-8000-{n:012x}"))
}

/// Gives every directory under `path` (links not followed) and `path` itself `rwx` for the
/// owner, so that whatever a case left can be removed.
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

/// A directory of the test's own under the system's temporary directory: `tree/`, the workspace
/// with `src/lib.rs`, and `scratch/` for the copies; removed when dropped.
struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "session-executor-security-w3c-{}-{name}",
            std::process::id()
        ));
        writable(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("scratch")).expect("a scratch directory");
        std::fs::create_dir_all(dir.join("tree/src")).expect("the tree");
        std::fs::write(dir.join("tree/src/lib.rs"), "pub fn a() {}\n").expect("a file");
        Self(dir)
    }

    fn scratch(&self) -> PathBuf {
        self.0.join("scratch")
    }

    /// A Codex executor over the workspace, its copy named `copy_name` under `scratch`, its
    /// sessions started by `sessions`.
    fn executor(&self, scratch: PathBuf, copy_name: &str, sessions: &Sessions) -> SessionExecutor {
        let mut executor =
            SessionExecutor::new(self.0.join("tree"), vec!["src/lib.rs".to_owned()], "fix a")
                .of_kind(Kind::Codex)
                .with_runner(Box::new(sessions.clone()));
        executor.credentials = CredentialSource::None;
        executor.scratch = scratch;
        executor.copy_name = Some(copy_name.to_owned());
        executor
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        writable(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A scripted Codex session; each start records the working directory metaharness handed the
/// child, resolved.
#[derive(Clone, Default)]
struct Sessions {
    started: Rc<RefCell<Vec<PathBuf>>>,
}

impl ProcessRunner for Sessions {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        self.started
            .borrow_mut()
            .push(std::fs::canonicalize(plan.cwd).unwrap_or_else(|_| plan.cwd.to_path_buf()));
        ScriptedRunner::new(rollout(plan.cwd), ScriptedLog::new()).start(plan)
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
    let case = CaseId("case-security-w3c".to_owned());
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(uuid(0xc1)),
        agent_revision_id: AgentRevisionId(uuid(0xc2)),
        case_id: case.clone(),
        principal: PrincipalId("principal-security-w3c".to_owned()),
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

/// The entries of `dir`, by name, sorted.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read the directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// A directory of another user at `<scratch>/<name>` suspends the step with
/// `ExternalAvailability`, naming the path, before any session starts, and is left with its
/// owner, its mode and its entries. Run as root, the directory is the test's own, given to the
/// overflow user (65534) with mode `0777`, so only the ownership check stands between the
/// session and emptying it; run as any other user, it is a directory of root's under `/`.
#[test]
fn a_directory_of_another_user_at_the_path_is_refused_and_left_as_it_is() {
    let fixture = Fixture::new("foreign-owner");
    let sessions = Sessions::default();
    let me = std::fs::metadata(&fixture.0)
        .expect("the fixture's owner")
        .uid();

    let (scratch, name) = if me == 0 {
        let foreign = fixture.scratch().join("foreign");
        std::fs::create_dir(&foreign).expect("a directory to give away");
        std::fs::write(foreign.join("keep.txt"), "keep\n").expect("a file of the other user's");
        std::fs::set_permissions(&foreign, std::fs::Permissions::from_mode(0o777))
            .expect("open to its new owner");
        std::os::unix::fs::chown(&foreign, Some(65534), Some(65534)).expect("given away");
        (fixture.scratch(), "foreign".to_owned())
    } else {
        let name = ["usr", "etc", "proc"]
            .into_iter()
            .find(|name| {
                std::fs::symlink_metadata(Path::new("/").join(name))
                    .is_ok_and(|meta| meta.is_dir() && meta.uid() != me)
            })
            .expect("a directory of another user's under /");
        (PathBuf::from("/"), name.to_owned())
    };
    let path = scratch.join(&name);
    let before = std::fs::symlink_metadata(&path).expect("the other user's directory");
    let listed_before = entries(&path);

    let executor = fixture.executor(scratch, &name, &sessions);
    let why = refusal(&step_of(&executor));

    assert!(
        why.contains(&path.display().to_string()),
        "the refusal names {}: {why}",
        path.display()
    );
    assert!(
        sessions.started.borrow().is_empty(),
        "no session runs in a directory of another user's: {:?}",
        sessions.started.borrow()
    );
    let after = std::fs::symlink_metadata(&path).expect("the directory is still there");
    assert_eq!(
        (after.uid(), after.mode(), after.ino()),
        (before.uid(), before.mode(), before.ino()),
        "the directory keeps its owner, mode and identity"
    );
    assert_eq!(
        entries(&path),
        listed_before,
        "the directory's entries are left"
    );
}

/// How many leftover entries the emptying works through: enough that the swap, made right after
/// the first of them is removed, lands while most are still to go.
const LEFTOVER: usize = 20_000;

/// How many fresh attempts are made while the swap lands only after the emptying is over (the
/// swapping thread was not scheduled in time), before such a run is taken as it is.
const ATTEMPTS: usize = 5;

fn leftover_name(n: usize) -> String {
    format!("f{n:05}")
}

/// `renameat2(AT_FDCWD, a, AT_FDCWD, b, RENAME_EXCHANGE)`: swaps the entries at `a` and `b` in
/// one step, so that `b` never names nothing; any user may make it on entries it may rename. The
/// crate has no binding to the call, so it is made directly.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn exchange(a: &Path, b: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;
    const AT_FDCWD: isize = -100;
    const RENAME_EXCHANGE: usize = 2;
    let a = std::ffi::CString::new(a.as_os_str().as_bytes())?;
    let b = std::ffi::CString::new(b.as_os_str().as_bytes())?;
    let ret: isize;
    // SAFETY: the Linux system call renameat2 with two NUL-terminated paths that outlive the
    // call, the current-directory sentinel for both directory descriptors and a flag word; it
    // reads the two paths and writes no memory of this process.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::asm!(
            "syscall",
            inlateout("rax") 316_isize => ret,
            in("rdi") AT_FDCWD,
            in("rsi") a.as_ptr(),
            in("rdx") AT_FDCWD,
            in("r10") b.as_ptr(),
            in("r8") RENAME_EXCHANGE,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    // SAFETY: as above.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!(
            "svc 0",
            in("x8") 276_isize,
            inlateout("x0") AT_FDCWD => ret,
            in("x1") a.as_ptr(),
            in("x2") AT_FDCWD,
            in("x3") b.as_ptr(),
            in("x4") RENAME_EXCHANGE,
            options(nostack),
        );
    }
    if ret < 0 {
        Err(std::io::Error::from_raw_os_error(
            i32::try_from(-ret).unwrap_or(i32::MAX),
        ))
    } else {
        Ok(())
    }
}

/// Elsewhere, the swap in two steps; the path names nothing for a moment between them.
#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn exchange(a: &Path, b: &Path) -> std::io::Result<()> {
    let target = std::fs::read_link(a)?;
    std::fs::remove_file(a)?;
    std::fs::rename(b, a)?;
    std::os::unix::fs::symlink(target, b)
}

/// What one attempt at swapping the root during its emptying saw.
struct Swap {
    /// Whether the root was swapped for the link at all; not, when the call never began to
    /// empty it.
    swapped: bool,
    /// Whether leftover entries were still in the root when it was swapped: the swap landed
    /// inside the emptying.
    inside: bool,
    outcome: String,
    /// How many of the user's files outside the root were removed.
    removed: usize,
    /// Entries made outside the root.
    made: Vec<String>,
    /// Sessions that ran outside the root, by their resolved working directory.
    ran_outside: Vec<PathBuf>,
}

impl Swap {
    fn reached_nothing(&self) -> bool {
        self.removed == 0 && self.made.is_empty() && self.ran_outside.is_empty()
    }
}

/// One attempt: a leftover root of the session user's at `<scratch>/case-17` holding
/// [`LEFTOVER`] entries, and `kept/`, another directory of the same user's holding entries under
/// the same names. A second thread, running before the call starts, swaps the root for a link
/// to `kept/` ([`exchange`], in one step) as soon as the root's modification time shows its
/// first removal; the root itself is then at `<scratch>/moved`.
fn swap_while_emptying(attempt: usize) -> Swap {
    let fixture = Fixture::new(&format!("swapped-root-{attempt}"));
    let sessions = Sessions::default();
    let scratch = fixture.scratch();
    std::fs::set_permissions(&scratch, std::fs::Permissions::from_mode(0o777))
        .expect("a scratch others can write, no sticky bit");
    let root = scratch.join("case-17");
    let moved = scratch.join("moved");
    let kept = fixture.0.join("kept");
    std::fs::create_dir(&root).expect("a leftover of the user's");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("the leftover's mode");
    std::fs::create_dir(&kept).expect("the user's directory elsewhere");
    for n in 0..LEFTOVER {
        std::fs::write(root.join(leftover_name(n)), "").expect("a leftover entry");
        std::fs::write(kept.join(leftover_name(n)), "kept\n").expect("a file of the user's");
    }
    std::fs::File::open(&root)
        .and_then(|dir| dir.set_modified(std::time::UNIX_EPOCH))
        .expect("the leftover's modification time, so its first removal shows");
    let kept_before = entries(&kept);
    std::os::unix::fs::symlink(&kept, &moved).expect("the link, ready beside the root");

    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, ready) = (Arc::clone(&stop), Arc::clone(&ready));
        let (root, moved) = (root.clone(), moved.clone());
        std::thread::spawn(move || {
            ready.store(true, Ordering::SeqCst);
            while !stop.load(Ordering::Relaxed) {
                let touched = std::fs::symlink_metadata(&root)
                    .is_ok_and(|meta| meta.mtime() != 0 || meta.mtime_nsec() != 0);
                if touched {
                    exchange(&moved, &root).expect("the root swapped for the link");
                    let inside = std::fs::read_dir(&moved).is_ok_and(|mut listing| {
                        listing.any(|entry| {
                            entry.is_ok_and(|entry| {
                                entry.file_name().to_string_lossy().starts_with('f')
                            })
                        })
                    });
                    return (true, inside);
                }
                std::hint::spin_loop();
            }
            (false, false)
        })
    };
    while !ready.load(Ordering::SeqCst) {
        std::thread::yield_now();
    }
    let executor = fixture.executor(scratch.clone(), "case-17", &sessions);
    let step = step_of(&executor);
    stop.store(true, Ordering::Relaxed);
    let (swapped, inside) = swapper.join().expect("the swapper");

    let kept_after = entries(&kept);
    let removed = kept_before
        .iter()
        .filter(|name| !kept_after.contains(name))
        .count();
    let made = kept_after
        .iter()
        .filter(|name| !kept_before.contains(name))
        .cloned()
        .collect();
    let kept_resolved = std::fs::canonicalize(&kept).expect("kept resolves");
    let ran_outside = sessions
        .started
        .borrow()
        .iter()
        .filter(|cwd| cwd.starts_with(&kept_resolved))
        .cloned()
        .collect();
    Swap {
        swapped,
        inside,
        outcome: format!("{:?}", step.outcome),
        removed,
        made,
        ran_outside,
    }
}

/// A leftover root of the session user's at `<scratch>/<name>`, swapped for a link to another
/// directory of the same user's once the emptying has removed its first entry: the swap a second
/// party can make in a scratch it can write that has no sticky bit (here `0777`), between
/// `ScratchCopy::claim`'s check of the locked handle's device and inode and its removals, which
/// name each entry by the root's path. Nothing in the other directory may be removed, the copy
/// and its home must not be made there, and no session may run there. A call that never begins
/// to empty the root (it refuses the scratch) is never swapped and passes.
#[test]
fn a_root_swapped_for_a_link_while_its_leftover_is_emptied_reaches_nothing_outside_it() {
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        let swap = swap_while_emptying(attempt);
        eprintln!(
            "attempt {attempt}: swapped {}, inside the emptying {}, removed {}, made {:?}, \
             outcome {}",
            swap.swapped, swap.inside, swap.removed, swap.made, swap.outcome
        );
        let settled = !swap.swapped || swap.inside;
        last = Some(swap);
        if settled {
            break;
        }
    }
    let swap = last.expect("one attempt");
    assert!(
        swap.reached_nothing(),
        "swapped inside the emptying: {}; outcome {}; the user's files outside the root \
         removed: {} of {LEFTOVER}; entries made outside the root: {:?}; sessions run outside \
         the root: {:?}",
        swap.inside,
        swap.outcome,
        swap.removed,
        swap.made,
        swap.ran_outside
    );
}
