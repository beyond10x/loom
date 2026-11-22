//! `embedder`: an engine that depends on `b10x-loom-tree` alone, as Runbook's does. It opens a
//! one-case tree of `probe@1` (`fixture/`), runs it with an observer that encodes each run-log
//! line, approves the capability the run waits on as [`DEFAULT_PRINCIPAL`], runs it again, closes
//! the case and reads its status, printing one `key value` line per fact on standard output.
//!
//! ```text
//! embedder (--cgroup-root DIR | --own-scope | --local) --store DIR --workspace DIR [--cancel-after-ms N]
//! embedder --hold-clone --cgroup-root DIR
//! embedder --hold-twice --cgroup-root DIR
//! ```
//!
//! `--hold-clone` runs no tree: it builds the runner on the root, keeps a clone of its
//! [`TreeRunner::runner`], drops the [`TreeRunner`] and prints `clone-held`; after one line on its
//! standard input it drops the clone and prints `clone-dropped`, then waits until its standard
//! input closes. `--hold-twice` builds a second runner on the root while the first lives and
//! prints whether it was refused. A refusal prints `refused <refusal>` on standard error and ends
//! with exit status 2.

use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use b10x_loom_tree::embed::{self, DEFAULT_PRINCIPAL, ProtocolCatalog, TreeRunner, encode_line};
use b10x_loom_tree::model::{ApprovalDecision, DecisionKind, TreeRun, TreeStop};
use b10x_loom_tree::runlog::RunLogLine;
use b10x_loom_tree::{
    ApproveRequest, CloseRequest, KeptProtocol, OpenRequest, Refused, RunSettings, TreeTime,
    approve, close, open_tree, read_protocols_directory, run_tree, tree_status,
};

/// The protocol and bindings of the fixture's tree.
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixture");

/// The instant every command of the tree records.
const AT: &str = "2026-11-22T08:00:00Z";

/// The cancel handle of a bound program, which `run_tree` sets from the caller's flag: named here
/// so that the embedder's build breaks when its path does.
const _CANCEL: fn() -> embed::Cancel = embed::Cancel::new;

/// Where the runner launches.
enum Mode {
    Root(PathBuf),
    OwnScope,
    Local,
}

/// What the command line asks for.
struct Arguments {
    mode: Mode,
    store: PathBuf,
    workspace: PathBuf,
    cancel_after: Option<Duration>,
    hold_clone: bool,
    hold_twice: bool,
}

fn arguments() -> Result<Arguments, String> {
    let mut mode = None;
    let (mut store, mut workspace, mut cancel_after) = (None, None, None);
    let (mut hold_clone, mut hold_twice) = (false, false);
    let mut words = std::env::args().skip(1);
    while let Some(word) = words.next() {
        let mut value = || words.next().ok_or(format!("{word} takes a value"));
        match word.as_str() {
            "--cgroup-root" => mode = Some(Mode::Root(PathBuf::from(value()?))),
            "--own-scope" => mode = Some(Mode::OwnScope),
            "--local" => mode = Some(Mode::Local),
            "--store" => store = Some(PathBuf::from(value()?)),
            "--workspace" => workspace = Some(PathBuf::from(value()?)),
            "--cancel-after-ms" => {
                let milliseconds = value()?.parse().map_err(|e| format!("{word}: {e}"))?;
                cancel_after = Some(Duration::from_millis(milliseconds));
            }
            "--hold-clone" => hold_clone = true,
            "--hold-twice" => hold_twice = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Arguments {
        mode: mode.ok_or("--cgroup-root, --own-scope or --local is required")?,
        store: store.unwrap_or_default(),
        workspace: workspace.unwrap_or_default(),
        cancel_after,
        hold_clone,
        hold_twice,
    })
}

fn main() -> ExitCode {
    let arguments = match arguments() {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("usage: {error}");
            return ExitCode::from(64);
        }
    };
    match embed_tree(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(refused) => {
            eprintln!("refused {refused}");
            ExitCode::from(2)
        }
    }
}

/// Prints `line` and flushes it, so a test reading the pipe sees it at once.
fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Every task of this process and the cgroup it is in, as `task-cgroup <tid> <path>`.
fn say_task_cgroups() {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return;
    };
    for task in tasks.flatten() {
        let cgroup = std::fs::read_to_string(task.path().join("cgroup")).unwrap_or_default();
        let path = cgroup
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .unwrap_or("none");
        say(&format!(
            "task-cgroup {} {path}",
            task.file_name().to_string_lossy()
        ));
    }
}

fn embed_tree(arguments: &Arguments) -> Result<(), Refused> {
    if arguments.hold_clone || arguments.hold_twice {
        let Mode::Root(root) = &arguments.mode else {
            return Err(Refused::new(
                b10x_loom_tree::model::TreeRefusalReason::ConfinementRefused,
                "--hold-clone and --hold-twice take --cgroup-root",
            ));
        };
        return if arguments.hold_clone {
            hold_clone(root)
        } else {
            hold_twice(root)
        };
    }
    let (runner, confinement) = match &arguments.mode {
        Mode::Root(root) => (TreeRunner::on_root(root)?, "substrate"),
        Mode::OwnScope => {
            // A second thread lives while the process moves: every thread moves with it.
            let (_parked, parked) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || parked.recv());
            let runner = TreeRunner::in_own_scope()?;
            say_task_cgroups();
            (runner, "substrate")
        }
        Mode::Local => (TreeRunner::local()?, "local"),
    };
    say(&format!(
        "local-reason {}",
        runner.runner().local_reason().unwrap_or("none")
    ));

    // The catalog: the fixture's protocols directory, which the store keeps.
    let (catalog, kept): (ProtocolCatalog, Vec<KeptProtocol>) = {
        let directory = read_protocols_directory(&Path::new(FIXTURE).join("protocols"))?;
        (directory.catalog, directory.kept)
    };
    let time = TreeTime::At(AT.into());
    let store = arguments.store.as_path();
    open_tree(&OpenRequest {
        store: store.to_owned(),
        workspace: arguments.workspace.clone(),
        protocol: "probe@1".into(),
        bindings: Path::new(FIXTURE).join("probe.bindings.yaml"),
        revisions: Vec::new(),
        case: "root".into(),
        time: time.clone(),
        catalog: &catalog,
        protocols: kept,
    })?;
    let settings = RunSettings {
        max_steps: None,
        max_wall_seconds: None,
        time: time.clone(),
        runner: runner.runner(),
        confinement: Some(confinement.into()),
    };

    // The first run waits on the capability the program action needs.
    let first = run(store, &catalog, &settings, None)?;
    let waits = first
        .decisions
        .iter()
        .find(|decision| decision.kind == DecisionKind::Approve)
        .ok_or_else(|| {
            Refused::new(
                b10x_loom_tree::model::TreeRefusalReason::StoreUnreadable,
                format!("the first run waits on no approval: {first:?}"),
            )
        })?;
    say(&format!(
        "decision-principal {}",
        waits.principal.as_deref().unwrap_or("none")
    ));
    say(&format!("default-principal {DEFAULT_PRINCIPAL}"));
    approve(&ApproveRequest {
        store,
        case: &waits.case,
        capability: waits.capability.as_deref(),
        principal: DEFAULT_PRINCIPAL,
        decision: ApprovalDecision::Allow,
        action: None,
        result: None,
        reasons: None,
        time: time.clone(),
        catalog: &catalog,
    })?;
    say(&format!(
        "approved {}",
        waits.capability.as_deref().unwrap_or("none")
    ));

    // The second run starts the program; a flag set from another thread cancels it.
    let second = run(store, &catalog, &settings, arguments.cancel_after)?;
    if second.stop == TreeStop::AwaitingPerson {
        let closed = close(&CloseRequest {
            store,
            case: "root",
            outcome: "kept",
            time,
            catalog: &catalog,
        })?;
        say(&format!("closed {}", closed.outcome));
    }
    let status = tree_status(store, &catalog)?;
    for case in &status.cases {
        say(&format!(
            "case {} outcome {}",
            case.case,
            case.outcome.as_deref().unwrap_or("none")
        ));
    }
    Ok(())
}

/// One `run_tree` of `store`, every line encoded as the observer gets it; with `cancel_after`, a
/// second thread sets the run's flag that long after the run started.
fn run(
    store: &Path,
    catalog: &ProtocolCatalog,
    settings: &RunSettings,
    cancel_after: Option<Duration>,
) -> Result<TreeRun, Refused> {
    let mut observer =
        |case: &str, line: &RunLogLine| say(&format!("line {case} {}", encode_line(line)));
    let flag = &AtomicBool::new(false);
    let (ran, set) = std::thread::scope(|scope| {
        let setter = cancel_after.map(|after| {
            scope.spawn(move || {
                std::thread::sleep(after);
                flag.store(true, Ordering::SeqCst);
                Instant::now()
            })
        });
        let ran = run_tree(store, catalog, settings, Some(&mut observer), Some(flag));
        (ran, setter.and_then(|setter| setter.join().ok()))
    });
    let ran = ran?;
    say(&format!("stop {:?}", ran.stop));
    if let Some(set) = set {
        say(&format!(
            "returned-after-cancel-ms {}",
            Instant::now().duration_since(set).as_millis()
        ));
    }
    Ok(ran)
}

/// `--hold-clone`: the root stays held by a clone of the runner after the [`TreeRunner`] is gone.
fn hold_clone(root: &Path) -> Result<(), Refused> {
    let runner = TreeRunner::on_root(root)?;
    let clone = runner.runner();
    drop(runner);
    say("clone-held");
    let mut input = std::io::stdin().lock();
    let mut line = String::new();
    let _ = input.read_line(&mut line);
    drop(clone);
    say("clone-dropped");
    while input.read_line(&mut line).is_ok_and(|read| read > 0) {}
    Ok(())
}

/// `--hold-twice`: a second runner on the root this process holds.
fn hold_twice(root: &Path) -> Result<(), Refused> {
    let first = TreeRunner::on_root(root)?;
    say("first-held");
    match TreeRunner::on_root(root) {
        Ok(_) => say("second-held"),
        Err(refused) => say(&format!("second-refused {refused}")),
    }
    drop(first);
    Ok(())
}
