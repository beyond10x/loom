//! `b10x-loom tree`: the flags of the tree commands mapped onto `b10x-loom-tree`, what each
//! prints and the exit status it ends with.
//!
//! A command's document (the opened tree, the status, the verdict) is one JSON object on standard
//! output; a refusal is one line on standard error, every value in it through
//! [`printable`](b10x_loom_intake_slice::run::printable), so no text a file or a program wrote can
//! move the cursor or start a line.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::ValueEnum as _;

use b10x_loom_intake_slice::confinement::TestRunner;
use b10x_loom_intake_slice::run::printable;
use b10x_loom_tree::embed::{ProtocolCatalog, TreeRunner, host_catalog};
use b10x_loom_tree::model::{ApprovalDecision, CycleResult, TreeRefusalReason};
use b10x_loom_tree::suite::{
    SuiteRefused, SuiteSettings, cycle_line, report_json, run_suite, summary_lines, workers_served,
};
use b10x_loom_tree::{
    ApproveRequest, CloseRequest, HoldRequest, OpenRequest, Reasons, Refused, ReopenRequest,
    RunSettings, TreeTime, approve as record, open_tree, person_program, read_protocols_directory,
    refusal_json, replay_difference, replay_json, replay_tree, run_json, run_tree, status_json,
    stored_catalog, tree_status, verify_json, verify_tree,
};
use serde_json::json;

use crate::{
    REEXEC_MARKER, REEXEC_READY, TREE_CONFINEMENT_PRECONDITION, TREE_LOG_FAILED, TREE_REFUSED,
    TREE_REPLAY_DIFFERS, TREE_TEST_CONFINEMENT_PRECONDITION, TREE_TEST_FAILED, TREE_TEST_REFUSED,
    TreeApproveArgs, TreeCloseArgs, TreeCommand, TreeConfinement, TreeHoldArgs, TreeOpenArgs,
    TreeReopenArgs, TreeReplayArgs, TreeRunArgs, TreeStoreArgs, TreeTestArgs, start_delegated,
    tree_run_exit_status,
};

/// Runs `command` and answers its exit status.
pub fn command(command: &TreeCommand) -> u8 {
    let ended = match command {
        TreeCommand::Open(arguments) => open(arguments),
        TreeCommand::Run(arguments) => run(arguments),
        TreeCommand::Approve(arguments) => approve(arguments),
        TreeCommand::Close(arguments) => close(arguments),
        TreeCommand::Status(arguments) => status(arguments),
        TreeCommand::Verify(arguments) => verify(arguments),
        TreeCommand::Replay(arguments) => replay(arguments),
        TreeCommand::Hold(arguments) => hold(arguments),
        TreeCommand::Reopen(arguments) => reopen(arguments),
        TreeCommand::Test(arguments) => return test(arguments),
    };
    match ended {
        Ok(status) => status,
        Err(refused) => {
            refuse(&refused);
            TREE_REFUSED
        }
    }
}

/// The line a refusal prints on standard error, and its document.
fn refuse(refused: &Refused) {
    eprintln!(
        "b10x-loom: tree refused: {}",
        printable(&refused.to_string())
    );
    eprintln!("{}", printable(&refusal_json(&refused.0).to_string()));
}

/// `path` resolved against the current directory: the library takes absolute paths only.
fn resolved(path: &Path) -> Result<PathBuf, Refused> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| {
            Refused::new(
                b10x_loom_tree::model::TreeRefusalReason::StoreUnreadable,
                format!("the current directory cannot be read: {error}"),
            )
        })
}

/// The catalog of the tree in `store`: the one it keeps, else the host's ([`host_catalog`]).
fn tree_catalog(store: &Path) -> Result<ProtocolCatalog, Refused> {
    match stored_catalog(store)? {
        Some(catalog) => Ok(catalog),
        None => host_catalog(),
    }
}

fn open(arguments: &TreeOpenArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let (catalog, protocols) = match &arguments.protocols {
        Some(directory) => {
            let read = read_protocols_directory(&resolved(directory)?)?;
            (read.catalog, read.kept)
        }
        None => (host_catalog()?, Vec::new()),
    };
    let opened = open_tree(&OpenRequest {
        store: resolved(&arguments.store)?,
        workspace: resolved(&arguments.workspace)?,
        protocol: arguments.protocol.clone(),
        bindings: resolved(&arguments.bindings)?,
        revisions: arguments.revisions.clone(),
        case: arguments.case.clone(),
        time,
        catalog: &catalog,
        protocols,
    })?;
    println!(
        "{}",
        printable(
            &json!({
                "store": opened.store.display().to_string(),
                "root": opened.root,
                "protocol": opened.protocol,
                "workspace": opened.workspace.display().to_string(),
            })
            .to_string()
        )
    );
    Ok(0)
}

/// The runner of `confinement`, built by `b10x_loom_tree::embed` as an embedder builds it: `local`
/// opened for the reason the `--unconfined-bindings` flag gives, the one reason the bound-program
/// port admits; `substrate` the runner on the root this run holds, `held` ([`substrate_runner`]).
fn tree_runner(
    confinement: TreeConfinement,
    held: Option<&TreeRunner>,
) -> Result<Arc<dyn TestRunner>, Refused> {
    match confinement {
        TreeConfinement::Substrate => substrate_runner(held),
        TreeConfinement::Local => TreeRunner::local().map(|local| local.runner()),
    }
}

/// Substrate's linux adapter on the root `held` holds, refused without one: `tree run` never lets
/// a driver discover an ancestor, which another run may use too.
fn substrate_runner(held: Option<&TreeRunner>) -> Result<Arc<dyn TestRunner>, Refused> {
    held.map(TreeRunner::runner)
        .ok_or_else(|| confinement_refused("CgroupUndelegated: tree run holds no cgroup root"))
}

/// A `ConfinementRefused` refusal saying `detail`, then the precondition and remedy of `tree run`.
fn confinement_refused(detail: impl std::fmt::Display) -> Refused {
    TREE_RUN_PLACER.refused(detail)
}

/// The command that places its programs ([`place`]): its name, which a refusal of its start again
/// says, the precondition and remedy its confinement refusals end with, and the exit status it
/// ends with when the run started again ended without one.
#[derive(Clone, Copy)]
struct Placer {
    name: &'static str,
    precondition: &'static str,
    no_status: u8,
}

/// `tree run`, and `tree approve` of a person action that runs a program.
const TREE_RUN_PLACER: Placer = Placer {
    name: "tree run",
    precondition: TREE_CONFINEMENT_PRECONDITION,
    no_status: TREE_REFUSED,
};

/// `tree test`, whose refusals name its own remedy ([`TREE_TEST_CONFINEMENT_PRECONDITION`]),
/// never `tree run`'s.
const TREE_TEST_PLACER: Placer = Placer {
    name: "tree test",
    precondition: TREE_TEST_CONFINEMENT_PRECONDITION,
    no_status: TREE_TEST_REFUSED,
};

impl Placer {
    /// A `ConfinementRefused` refusal saying `detail`, then this command's precondition and
    /// remedy.
    fn refused(self, detail: impl std::fmt::Display) -> Refused {
        Refused::new(
            TreeRefusalReason::ConfinementRefused,
            format!("{detail}; {}", self.precondition),
        )
    }
}

/// Where a command under `substrate` runs its confined programs.
enum Placement {
    /// Under the root this process holds, through the runner that holds it.
    Held(TreeRunner),
    /// Nowhere here: the command was started again in a delegated scope of its own and ended with
    /// this exit status.
    StartedAgain(u8),
}

/// The placement of `placer`'s programs under `confinement`, `None` under `local`:
/// `cgroup_root`, held; in a command started again, the scope it moved into a leaf of, held;
/// otherwise the command started again in a delegated scope of its own. No root is ever
/// discovered. `tree run`, `tree test` and the `tree approve` of a person action that runs a
/// program place their programs here; each refusal ends with `placer`'s precondition and remedy.
fn place(
    placer: Placer,
    confinement: TreeConfinement,
    cgroup_root: Option<&Path>,
) -> Result<Option<Placement>, Refused> {
    if confinement == TreeConfinement::Local {
        if cgroup_root.is_some() {
            return Err(Refused::new(
                TreeRefusalReason::ConfinementRefused,
                "--cgroup-root names the root of the substrate confinement; --confinement local runs no program under one",
            ));
        }
        return Ok(None);
    }
    let held = match cgroup_root {
        Some(root) => TreeRunner::on_root(&resolved(root)?),
        None if std::env::var_os(REEXEC_MARKER).is_some() => {
            started_again(placer)?;
            TreeRunner::in_own_scope()
        }
        None => return start_again(placer).map(|status| Some(Placement::StartedAgain(status))),
    };
    held.map(|runner| Some(Placement::Held(runner)))
        .map_err(|refused| placer.refused(refused.0.detail))
}

/// Starts this command again in a delegated scope of its own and answers the exit status it
/// ended with; refused, naming why, when it never started there.
fn start_again(placer: Placer) -> Result<u8, Refused> {
    let name = placer.name;
    let failed = |why: String| {
        placer.refused(format!(
            "CgroupUndelegated: {name} could not start itself again under systemd-run --user --scope -p Delegate=yes: {why}"
        ))
    };
    let state = tempfile::Builder::new()
        .prefix("loom-delegation-")
        .tempdir()
        .map_err(|e| failed(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(state.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(|e| failed(e.to_string()))?;
    }
    let ready = state.path().join("ready");
    let executable = std::env::current_exe().map_err(|e| failed(e.to_string()))?;
    let status = start_delegated(&executable, &[(REEXEC_READY, ready.as_os_str())])
        .map_err(|e| failed(format!("systemd-run unavailable: {e}")))?;
    if !ready.exists() {
        return Err(failed(format!(
            "systemd-run exited {status} before {name} started in the scope"
        )));
    }
    Ok(
        match status.code().and_then(|code| u8::try_from(code).ok()) {
            Some(code) => code,
            None => {
                eprintln!("b10x-loom: {name}: the run started again ended {status}");
                placer.no_status
            }
        },
    )
}

/// In a command started again: says it runs in its scope, creating the file its starter named.
fn started_again(placer: Placer) -> Result<(), Refused> {
    let ready = std::env::var_os(REEXEC_READY)
        .map(PathBuf::from)
        .filter(|ready| {
            ready.file_name().is_some_and(|name| name == "ready")
                && ready
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("loom-delegation-"))
        })
        .ok_or_else(|| {
            placer.refused("CgroupUndelegated: the delegation handshake path is missing")
        })?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ready)
        .map(drop)
        .map_err(|e| {
            placer.refused(format!(
                "CgroupUndelegated: delegation handshake failed: {e}"
            ))
        })
}

fn run(arguments: &TreeRunArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    // Held until the run ends: no other tree run uses this root meanwhile.
    let held = match place(
        TREE_RUN_PLACER,
        arguments.confinement,
        arguments.cgroup_root.as_deref(),
    )? {
        Some(Placement::StartedAgain(status)) => return Ok(status),
        Some(Placement::Held(runner)) => Some(runner),
        None => None,
    };
    // Taken by the process that runs the steps alone: one that started itself again above waits
    // for that run and handles no signal of its own, as before.
    let cancel = Arc::new(AtomicBool::new(false));
    cancel_on_signals(Arc::clone(&cancel))?;
    let catalog = tree_catalog(&resolved(&arguments.store)?)?;
    let settings = RunSettings {
        max_steps: arguments.max_steps,
        max_wall_seconds: arguments.max_wall_seconds,
        time,
        runner: tree_runner(arguments.confinement, held.as_ref())?,
        confinement: arguments
            .confinement
            .to_possible_value()
            .map(|value| value.get_name().to_owned()),
        cgroup_root: arguments.cgroup_root.as_deref().map(resolved).transpose()?,
    };
    let ran = run_tree(
        &resolved(&arguments.store)?,
        &catalog,
        &settings,
        None,
        Some(&cancel),
    )
    .map_err(|mut refused| {
        // A refusal of the confinement names tree run's own remedy, never `run`'s.
        if held.is_some() && refused.0.reason == TreeRefusalReason::ConfinementRefused {
            refused.0.detail = format!("{}; {TREE_CONFINEMENT_PRECONDITION}", refused.0.detail);
        }
        refused
    })?;
    println!("{}", printable(&run_json(&ran).to_string()));
    for decision in &ran.decisions {
        eprintln!(
            "b10x-loom: tree waits: case {}: {}",
            printable(&decision.case),
            printable(&decision.decides)
        );
        eprintln!(
            "  run: {}",
            printable(
                &decision
                    .command
                    .iter()
                    .map(|word| shell_word(word))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        );
    }
    if let Some(detail) = &ran.detail {
        eprintln!(
            "b10x-loom: tree run: {:?}: case {}: {}",
            ran.stop,
            printable(&ran.case),
            printable(detail)
        );
    }
    Ok(tree_run_exit_status(ran.stop))
}

/// Sets `cancel` on the first SIGTERM or SIGINT, so the run stops its program and ends
/// `Cancelled` (exit 5) instead of dying with the program still running; a later signal changes
/// nothing, since ending at once would leave the program running. The signals are taken before
/// this returns, and a thread waits for them; one that cannot be taken refuses the run before it
/// starts anything.
fn cancel_on_signals(cancel: Arc<AtomicBool>) -> Result<(), Refused> {
    use tokio::signal::unix::{SignalKind, signal};
    let refused = |why: String| {
        Refused::new(
            TreeRefusalReason::ConfinementRefused,
            format!("{why}: a signal would end tree run with its program still running"),
        )
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| refused(format!("no runtime for the stop signals: {e}")))?;
    let (mut terminate, mut interrupt) = {
        let _entered = runtime.enter();
        let taken = |kind: SignalKind, name: &str| {
            signal(kind).map_err(|e| refused(format!("{name} cannot be handled: {e}")))
        };
        (
            taken(SignalKind::terminate(), "SIGTERM")?,
            taken(SignalKind::interrupt(), "SIGINT")?,
        )
    };
    std::thread::spawn(move || {
        runtime.block_on(async {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = interrupt.recv() => {}
            }
            cancel.store(true, Ordering::SeqCst);
            eprintln!("b10x-loom: tree run: cancelling, stopping the running program first");
            loop {
                tokio::select! {
                    _ = terminate.recv() => {}
                    _ = interrupt.recv() => {}
                }
            }
        });
    });
    Ok(())
}

/// `word` as one word of a POSIX shell: bare when it holds only safe characters, else quoted.
fn shell_word(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./@=:,+".contains(c));
    if safe {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

fn approve(arguments: &TreeApproveArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let store = resolved(&arguments.store)?;
    let catalog = tree_catalog(&store)?;
    let reasons_file = arguments
        .reasons_file
        .as_deref()
        .map(resolved)
        .transpose()?;
    let reasons = match (&arguments.reasons, &reasons_file) {
        (Some(text), _) => Some(Reasons::Text(text)),
        (None, Some(path)) => Some(Reasons::File(path)),
        (None, None) => None,
    };
    // A person action whose binding names an `argv` runs its program as `tree run` runs its own:
    // placed under `--confinement` (started again in a delegated scope of its own when it names
    // `substrate` and no `--cgroup-root`), the root held until the approval ends.
    let program = match arguments.action.as_deref() {
        Some(action) if !arguments.deny => {
            person_program(&store, &arguments.case, action, &catalog)?
        }
        _ => false,
    };
    let mut held = None;
    let runner = if program {
        match place(
            TREE_RUN_PLACER,
            arguments.confinement,
            arguments.cgroup_root.as_deref(),
        )? {
            Some(Placement::StartedAgain(status)) => return Ok(status),
            Some(Placement::Held(runner)) => held = Some(runner),
            None => {}
        }
        Some(tree_runner(arguments.confinement, held.as_ref())?)
    } else {
        None
    };
    let approval = record(&ApproveRequest {
        store: &store,
        case: &arguments.case,
        capability: arguments.capability.as_deref(),
        principal: &arguments.principal,
        decision: if arguments.deny {
            ApprovalDecision::Deny
        } else {
            ApprovalDecision::Allow
        },
        action: arguments.action.as_deref(),
        result: arguments.result.as_deref(),
        reasons,
        time,
        catalog: &catalog,
        runner,
    })
    .map_err(|mut refused| {
        // The command that approves again, once the principal fixed the cause: this one, its
        // paths resolved.
        if refused.0.reason == TreeRefusalReason::PersonProgramFailed {
            refused.0.command = Some(approve_command(arguments, &store, reasons_file.as_deref()));
        }
        refused
    })?;
    drop(held);
    // A person action that needs no capability records no authority: the action is printed.
    let printed = match approval {
        Some(approval) => json!({
            "case": approval.case,
            "capability": approval.capability,
            "decision": format!("{:?}", approval.decision),
            "principal": approval.principal,
            "case_revision": approval.case_revision,
            "at": approval.at.0,
        }),
        None => json!({
            "case": arguments.case,
            "action": arguments.action,
            "principal": arguments.principal,
        }),
    };
    println!("{}", printable(&printed.to_string()));
    Ok(0)
}

/// The argv of `tree approve` as `arguments` name it, `store` and `reasons_file` resolved: `b10x-loom
/// tree approve --store --case [--capability] --principal --action [--result] [--reasons |
/// --reasons-file] [--at] --confinement [--cgroup-root]`.
fn approve_command(
    arguments: &TreeApproveArgs,
    store: &Path,
    reasons_file: Option<&Path>,
) -> Vec<String> {
    let mut words: Vec<String> = ["b10x-loom", "tree", "approve", "--store"]
        .map(str::to_owned)
        .to_vec();
    words.push(store.display().to_string());
    words.extend(["--case".to_owned(), arguments.case.clone()]);
    let mut flag = |name: &str, value: Option<String>| {
        if let Some(value) = value {
            words.extend([name.to_owned(), value]);
        }
    };
    flag("--capability", arguments.capability.clone());
    flag("--principal", Some(arguments.principal.clone()));
    flag("--action", arguments.action.clone());
    flag("--result", arguments.result.clone());
    flag("--reasons", arguments.reasons.clone());
    flag(
        "--reasons-file",
        reasons_file.map(|path| path.display().to_string()),
    );
    flag("--at", arguments.at.clone());
    flag(
        "--confinement",
        arguments
            .confinement
            .to_possible_value()
            .map(|value| value.get_name().to_owned()),
    );
    flag(
        "--cgroup-root",
        arguments
            .cgroup_root
            .as_deref()
            .map(|root| resolved(root).unwrap_or_else(|_| root.to_owned()))
            .map(|root| root.display().to_string()),
    );
    words
}

fn close(arguments: &TreeCloseArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let store = resolved(&arguments.store)?;
    let catalog = tree_catalog(&store)?;
    let closed = b10x_loom_tree::close(&CloseRequest {
        store: &store,
        case: &arguments.case,
        outcome: &arguments.outcome,
        time,
        catalog: &catalog,
    })?;
    println!(
        "{}",
        printable(
            &json!({"case": closed.case, "outcome": closed.outcome, "at": closed.at.0}).to_string()
        )
    );
    Ok(0)
}

/// `tree hold`: the members are checked by the library before the store is read, so no catalog is
/// built and nothing of the store is read for a refused member.
fn hold(arguments: &TreeHoldArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let held = b10x_loom_tree::hold(&HoldRequest {
        store: &resolved(&arguments.store)?,
        principal: &arguments.principal,
        reason: &arguments.reason,
        time,
    })?;
    println!(
        "{}",
        printable(
            &json!({
                "case": held.case,
                "seq": held.seq,
                "at": held.at.0,
                "principal": held.principal,
                "reason": held.reason,
            })
            .to_string()
        )
    );
    Ok(0)
}

/// `tree reopen`, as [`hold`]: the principal is checked before the store is read.
fn reopen(arguments: &TreeReopenArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let lifted = b10x_loom_tree::reopen(&ReopenRequest {
        store: &resolved(&arguments.store)?,
        principal: &arguments.principal,
        time,
    })?;
    println!(
        "{}",
        printable(
            &json!({
                "case": lifted.hold.case,
                "seq": lifted.seq,
                "at": lifted.at.0,
                "principal": lifted.principal,
                "stop": lifted.hold.seq,
            })
            .to_string()
        )
    );
    Ok(0)
}

fn status(arguments: &TreeStoreArgs) -> Result<u8, Refused> {
    let store = resolved(&arguments.store)?;
    let catalog = tree_catalog(&store)?;
    let status = tree_status(&store, &catalog)?;
    println!("{}", printable(&status_json(&status).to_string()));
    Ok(0)
}

/// The suite, protocols, scratch and report paths of `tree test`, resolved.
type TestPaths = (PathBuf, PathBuf, PathBuf, Option<PathBuf>);

fn test_paths(arguments: &TreeTestArgs) -> Result<TestPaths, Refused> {
    Ok((
        resolved(&arguments.suite)?,
        resolved(&arguments.protocols)?,
        resolved(&arguments.scratch)?,
        arguments.report.as_deref().map(resolved).transpose()?,
    ))
}

/// `tree test`: parses nothing more than clap did, places its programs as `tree run` does
/// ([`place`], with its own remedy) after the worker bound, calls [`run_suite`] with the runner
/// that holds the root, prints each cycle's line as it ends and the two summary lines, and writes
/// `--report`.
fn test(arguments: &TreeTestArgs) -> u8 {
    let refuse_test = |detail: String, document: serde_json::Value| {
        eprintln!("b10x-loom: tree test refused: {}", printable(&detail));
        eprintln!("{}", printable(&document.to_string()));
        TREE_TEST_REFUSED
    };
    let tree_refused =
        |refused: Refused| refuse_test(refused.to_string(), refusal_json(&refused.0));
    let suite_refused = |refused: SuiteRefused| {
        refuse_test(
            refused.to_string(),
            json!({
                "reason": format!("{:?}", refused.reason()),
                "member": refused.member(),
                "detail": refused.0.detail,
            }),
        )
    };
    let time = match TreeTime::from_flag(Some(&arguments.at)) {
        Ok(time) => time,
        Err(refused) => return tree_refused(refused),
    };
    let (suite, protocols, scratch, report) = match test_paths(arguments) {
        Ok(paths) => paths,
        Err(refused) => return tree_refused(refused),
    };
    let confinement = arguments
        .confinement
        .to_possible_value()
        .map(|value| value.get_name().to_owned());
    let workers = NonZeroUsize::new(usize::from(arguments.workers)).unwrap_or(NonZeroUsize::MIN);
    if let Err(refused) = workers_served(confinement.as_deref(), workers) {
        return suite_refused(refused);
    }
    // Held until the suite ends: every worker's launches run under this one hold.
    let held = match place(
        TREE_TEST_PLACER,
        arguments.confinement,
        arguments.cgroup_root.as_deref(),
    ) {
        Ok(Some(Placement::StartedAgain(status))) => return status,
        Ok(Some(Placement::Held(runner))) => Some(runner),
        Ok(None) => None,
        Err(refused) => return tree_refused(refused),
    };
    let runner = match tree_runner(arguments.confinement, held.as_ref()) {
        Ok(runner) => runner,
        Err(refused) => return tree_refused(refused),
    };
    let mut settings = SuiteSettings::new(protocols, scratch, time, runner, confinement);
    settings.workers = workers;
    settings.only = arguments.only.clone();
    let mut print = |result: &CycleResult| println!("{}", printable(&cycle_line(result)));
    let report_of = match run_suite(&suite, &settings, Some(&mut print), None) {
        Ok(report) => report,
        Err(refused) => return suite_refused(refused),
    };
    for line in summary_lines(&report_of) {
        println!("{}", printable(&line));
    }
    let mut status = if report_of.failed.is_empty() {
        0
    } else {
        TREE_TEST_FAILED
    };
    if let Some(report) = report {
        let mut document = report_json(&report_of).to_string();
        document.push('\n');
        if let Err(error) = std::fs::write(&report, document) {
            eprintln!(
                "b10x-loom: tree test: the report {} cannot be written: {}",
                printable(&report.display().to_string()),
                printable(&error.to_string())
            );
            status = TREE_TEST_FAILED;
        }
    }
    status
}

fn verify(arguments: &TreeStoreArgs) -> Result<u8, Refused> {
    let store = resolved(&arguments.store)?;
    let catalog = tree_catalog(&store)?;
    let verified = verify_tree(&store, &catalog)?;
    println!("{}", printable(&verify_json(&verified).to_string()));
    Ok(match &verified.failure {
        None => 0,
        Some(failure) => {
            eprintln!(
                "b10x-loom: tree verify: case {} line {}: {}",
                printable(&failure.case),
                failure.line,
                printable(&failure.detail)
            );
            TREE_LOG_FAILED
        }
    })
}

/// `tree replay`: the cases decided again, printed as one object; the first that differs, and
/// whether its state or its decision, on standard error, with the line its undo stopped at.
fn replay(arguments: &TreeReplayArgs) -> Result<u8, Refused> {
    let store = resolved(&arguments.store)?;
    let catalog = tree_catalog(&store)?;
    let replayed = replay_tree(&store, &catalog, arguments.case.as_deref())?;
    println!("{}", printable(&replay_json(&replayed).to_string()));
    Ok(match replay_difference(&replayed) {
        None => 0,
        Some((case, what, line)) => {
            let line = line.map_or_else(String::new, |seq| format!(" at line {seq}"));
            eprintln!(
                "b10x-loom: tree replay: case {} differs from its log: {what}{line}",
                printable(case)
            );
            TREE_REPLAY_DIFFERS
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without a held root `tree run` builds no substrate runner, its refusal naming
    /// `tree run --cgroup-root`, and a path it cannot hold as a root is refused naming it. That
    /// the runner on a held root launches under that root and no other is held by the embedder
    /// fixture (`an_embedded_tree_two_levels_below_its_root_runs_under_the_root`).
    #[test]
    fn a_substrate_runner_needs_a_held_root_and_an_unheld_path_is_refused() {
        let Err(refused) = substrate_runner(None) else {
            panic!("a substrate runner without a held root");
        };
        assert_eq!(
            refused.0.reason,
            b10x_loom_tree::model::TreeRefusalReason::ConfinementRefused
        );
        assert!(
            refused.0.detail.contains("tree run --cgroup-root"),
            "{}",
            refused.0.detail
        );
        assert!(tree_runner(TreeConfinement::Substrate, None).is_err());

        // `tree test` places its programs as `tree run` does, and each of its refusals names its
        // own remedy, never `tree run`'s.
        let refused = TREE_TEST_PLACER.refused("CgroupUndelegated: the detail");
        assert_eq!(refused.0.reason, TreeRefusalReason::ConfinementRefused);
        assert!(
            refused.0.detail.contains("--confinement local")
                && refused.0.detail.contains("tree test --cgroup-root")
                && !refused.0.detail.contains("tree run"),
            "{}",
            refused.0.detail
        );
        let Err(refused) = place(
            TREE_TEST_PLACER,
            TreeConfinement::Local,
            Some(Path::new("/sys/fs/cgroup")),
        ) else {
            panic!("--confinement local placed under a cgroup root");
        };
        assert_eq!(refused.0.reason, TreeRefusalReason::ConfinementRefused);

        let root = Path::new("/sys/fs/cgroup/loom-held-root");
        let refused = TreeRunner::on_root(root).unwrap_err();
        assert_eq!(
            refused.0.reason,
            b10x_loom_tree::model::TreeRefusalReason::ConfinementRefused
        );
        assert!(
            refused.0.detail.contains("/sys/fs/cgroup/loom-held-root"),
            "{}",
            refused.0.detail
        );
    }
}
