//! `b10x-loom tree`: the flags of the tree commands mapped onto `b10x-loom-tree`, what each
//! prints and the exit status it ends with.
//!
//! A command's document (the opened tree, the status, the verdict) is one JSON object on standard
//! output; a refusal is one line on standard error, every value in it through
//! [`printable`](b10x_loom_intake_slice::run::printable), so no text a file or a program wrote can
//! move the cursor or start a line.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::ValueEnum as _;

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    CgroupRootHold, ConfinementAdapter, EngineConfinement, EngineRunner, SubstrateRunner,
    TestRunner, hold_cgroup_root, prepare_delegated_cgroup,
};
use b10x_loom_intake_slice::run::printable;
use b10x_loom_tree::model::{ApprovalDecision, CycleResult, TreeRefusalReason};
use b10x_loom_tree::suite::{SuiteSettings, cycle_line, report_json, run_suite, summary_lines};
use b10x_loom_tree::{
    ApproveRequest, CloseRequest, OpenRequest, Reasons, Refused, RunSettings, TreeTime,
    approve as record, open_tree, read_protocols_directory, refusal_json, run_json, run_tree,
    status_json, stored_catalog, tree_status, verify_json, verify_tree,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::json;

use crate::{
    REEXEC_MARKER, REEXEC_READY, TREE_CONFINEMENT_PRECONDITION, TREE_LOG_FAILED, TREE_REFUSED,
    TREE_TEST_FAILED, TREE_TEST_REFUSED, TreeApproveArgs, TreeCloseArgs, TreeCommand,
    TreeConfinement, TreeOpenArgs, TreeRunArgs, TreeStoreArgs, TreeTestArgs, start_delegated,
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

/// The host's catalog: the bundled protocols and the user's installed ones.
fn host_catalog() -> Result<ProtocolCatalog, Refused> {
    InstallStore::user()
        .and_then(|store| store.catalog())
        .map_err(|error| {
            Refused::new(
                b10x_loom_tree::model::TreeRefusalReason::UnknownProtocol,
                format!("the host's protocol catalog cannot be read: {error}"),
            )
        })
}

/// The catalog of the tree in `store`: the one it keeps, else the host's.
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

/// The runner of `confinement`: `local` opened for the reason the `--unconfined-bindings` flag
/// gives, the one reason the bound-program port admits; `substrate` its linux adapter on
/// `cgroup_root`, the root this run holds ([`substrate_runner`]).
fn tree_runner(
    confinement: TreeConfinement,
    cgroup_root: Option<&Path>,
) -> Result<Arc<dyn TestRunner>, Refused> {
    match confinement {
        TreeConfinement::Substrate => {
            substrate_runner(cgroup_root).map(|runner| Arc::new(runner) as Arc<dyn TestRunner>)
        }
        TreeConfinement::Local => EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .map(|runner| Arc::new(runner) as Arc<dyn TestRunner>)
        .map_err(|error| Refused::new(TreeRefusalReason::ConfinementRefused, error.to_string())),
    }
}

/// Substrate's linux adapter on `cgroup_root`, refused without one: `tree run` never lets a
/// driver discover an ancestor, which another run may use too.
fn substrate_runner(cgroup_root: Option<&Path>) -> Result<SubstrateRunner, Refused> {
    cgroup_root
        .map(|root| SubstrateRunner::new(Some(root.to_owned())))
        .ok_or_else(|| confinement_refused("CgroupUndelegated: tree run holds no cgroup root"))
}

/// A `ConfinementRefused` refusal saying `detail`, then the precondition and remedy of `tree run`.
fn confinement_refused(detail: impl std::fmt::Display) -> Refused {
    Refused::new(
        TreeRefusalReason::ConfinementRefused,
        format!("{detail}; {TREE_CONFINEMENT_PRECONDITION}"),
    )
}

/// Where a `tree run` under `substrate` runs its confined programs.
enum Placement {
    /// Under the root this process holds.
    Held(CgroupRootHold),
    /// Nowhere here: the run was started again in a delegated scope of its own and ended with
    /// this exit status.
    StartedAgain(u8),
}

/// The placement of `arguments`' run, `None` under `local`: `--cgroup-root`'s root, held; in a
/// run started again, the scope it moved into a leaf of, held; otherwise the run started again in
/// a delegated scope of its own. No root is ever discovered.
fn place(arguments: &TreeRunArgs) -> Result<Option<Placement>, Refused> {
    if arguments.confinement == TreeConfinement::Local {
        if arguments.cgroup_root.is_some() {
            return Err(Refused::new(
                TreeRefusalReason::ConfinementRefused,
                "--cgroup-root names the root of the substrate confinement; --confinement local runs no program under one",
            ));
        }
        return Ok(None);
    }
    let root = match &arguments.cgroup_root {
        Some(root) => resolved(root)?,
        None if std::env::var_os(REEXEC_MARKER).is_some() => {
            started_again()?;
            prepare_delegated_cgroup().map_err(confinement_refused)?
        }
        None => return start_again().map(|status| Some(Placement::StartedAgain(status))),
    };
    hold_cgroup_root(&root)
        .map(|hold| Some(Placement::Held(hold)))
        .map_err(confinement_refused)
}

/// Starts this `tree run` again in a delegated scope of its own and answers the exit status it
/// ended with; refused, naming why, when it never started there.
fn start_again() -> Result<u8, Refused> {
    let failed = |why: String| {
        confinement_refused(format!(
            "CgroupUndelegated: tree run could not start itself again under systemd-run --user --scope -p Delegate=yes: {why}"
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
            "systemd-run exited {status} before tree run started in the scope"
        )));
    }
    Ok(
        match status.code().and_then(|code| u8::try_from(code).ok()) {
            Some(code) => code,
            None => {
                eprintln!("b10x-loom: tree run: the run started again ended {status}");
                TREE_REFUSED
            }
        },
    )
}

/// In a `tree run` started again: says it runs in its scope, creating the file its starter named.
fn started_again() -> Result<(), Refused> {
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
            confinement_refused("CgroupUndelegated: the delegation handshake path is missing")
        })?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ready)
        .map(drop)
        .map_err(|e| {
            confinement_refused(format!(
                "CgroupUndelegated: delegation handshake failed: {e}"
            ))
        })
}

fn run(arguments: &TreeRunArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    // Held until the run ends: no other tree run uses this root meanwhile.
    let held = match place(arguments)? {
        Some(Placement::StartedAgain(status)) => return Ok(status),
        Some(Placement::Held(hold)) => Some(hold),
        None => None,
    };
    let catalog = tree_catalog(&resolved(&arguments.store)?)?;
    let settings = RunSettings {
        max_steps: arguments.max_steps,
        max_wall_seconds: arguments.max_wall_seconds,
        time,
        runner: tree_runner(
            arguments.confinement,
            held.as_ref().map(CgroupRootHold::root),
        )?,
        confinement: arguments
            .confinement
            .to_possible_value()
            .map(|value| value.get_name().to_owned()),
    };
    let ran = run_tree(
        &resolved(&arguments.store)?,
        &catalog,
        &settings,
        None,
        None,
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
    })?;
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

/// `tree test`: parses nothing more than clap did, calls [`run_suite`], prints each cycle's line as
/// it ends and the two summary lines, and writes `--report`.
fn test(arguments: &TreeTestArgs) -> u8 {
    let refuse_test = |detail: String, document: serde_json::Value| {
        eprintln!("b10x-loom: tree test refused: {}", printable(&detail));
        eprintln!("{}", printable(&document.to_string()));
        TREE_TEST_REFUSED
    };
    let tree_refused =
        |refused: Refused| refuse_test(refused.to_string(), refusal_json(&refused.0));
    let time = match TreeTime::from_flag(Some(&arguments.at)) {
        Ok(time) => time,
        Err(refused) => return tree_refused(refused),
    };
    let (suite, protocols, scratch, report) = match test_paths(arguments) {
        Ok(paths) => paths,
        Err(refused) => return tree_refused(refused),
    };
    let runner = match tree_runner(arguments.confinement, None) {
        Ok(runner) => runner,
        Err(refused) => return tree_refused(refused),
    };
    let settings = SuiteSettings::new(
        protocols,
        scratch,
        time,
        runner,
        arguments
            .confinement
            .to_possible_value()
            .map(|value| value.get_name().to_owned()),
    );
    let mut print = |result: &CycleResult| println!("{}", printable(&cycle_line(result)));
    let report_of = match run_suite(&suite, &settings, Some(&mut print), None) {
        Ok(report) => report,
        Err(refused) => {
            return refuse_test(
                refused.to_string(),
                json!({
                    "reason": format!("{:?}", refused.reason()),
                    "member": refused.member(),
                    "detail": refused.0.detail,
                }),
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `tree run` never discovers an ancestor: the substrate runner it builds holds the root it
    /// was handed, and without one it builds none.
    #[test]
    fn the_tree_runner_never_discovers_an_ancestor() {
        let refused = substrate_runner(None).unwrap_err();
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

        let root = Path::new("/sys/fs/cgroup/loom-held-root");
        let runner = substrate_runner(Some(root)).unwrap();
        assert_eq!(runner.cgroup_root(), Some(root));
        assert!(tree_runner(TreeConfinement::Substrate, Some(root)).is_ok());
    }
}
