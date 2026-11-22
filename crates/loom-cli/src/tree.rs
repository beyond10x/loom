//! `b10x-loom tree`: the flags of the tree commands mapped onto `b10x-loom-tree`, what each
//! prints and the exit status it ends with.
//!
//! A command's document (the opened tree, the status, the verdict) is one JSON object on standard
//! output; a refusal is one line on standard error, every value in it through
//! [`printable`](b10x_loom_intake_slice::run::printable), so no text a file or a program wrote can
//! move the cursor or start a line.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use b10x_loom_intake_slice::bindings::UNCONFINED_FLAG_REASON;
use b10x_loom_intake_slice::confinement::{
    ConfinementAdapter, EngineConfinement, EngineRunner, SubstrateRunner, TestRunner,
};
use b10x_loom_intake_slice::run::printable;
use b10x_loom_tree::model::ApprovalDecision;
use b10x_loom_tree::{
    ApproveRequest, OpenRequest, Refused, RunSettings, TreeTime, approve as record, open_tree,
    refusal_json, run_json, run_tree, status_json, stored_catalog, tree_status, verify_json, verify_tree,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::json;

use crate::{
    TREE_LOG_FAILED, TREE_REFUSED, TreeApproveArgs, TreeCommand, TreeConfinement, TreeOpenArgs,
    TreeRunArgs, TreeStoreArgs, tree_run_exit_status,
};

/// Runs `command` and answers its exit status.
pub fn command(command: &TreeCommand) -> u8 {
    let ended = match command {
        TreeCommand::Open(arguments) => open(arguments),
        TreeCommand::Run(arguments) => run(arguments),
        TreeCommand::Approve(arguments) => approve(arguments),
        TreeCommand::Status(arguments) => status(arguments),
        TreeCommand::Verify(arguments) => verify(arguments),
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
    let catalog = host_catalog()?;
    let opened = open_tree(&OpenRequest {
        store: resolved(&arguments.store)?,
        workspace: resolved(&arguments.workspace)?,
        protocol: arguments.protocol.clone(),
        bindings: resolved(&arguments.bindings)?,
        revisions: arguments.revisions.clone(),
        case: arguments.case.clone(),
        time,
        catalog: &catalog,
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
/// gives, the one reason the bound-program port admits; `substrate` its linux adapter.
fn tree_runner(confinement: TreeConfinement) -> Result<Arc<dyn TestRunner>, Refused> {
    match confinement {
        TreeConfinement::Substrate => Ok(Arc::new(SubstrateRunner::new(None))),
        TreeConfinement::Local => EngineRunner::open(&EngineConfinement {
            adapter: ConfinementAdapter::Local,
            local_reason: Some(UNCONFINED_FLAG_REASON.to_owned()),
            cgroup_root: None,
        })
        .map(|runner| Arc::new(runner) as Arc<dyn TestRunner>)
        .map_err(|error| {
            Refused::new(
                b10x_loom_tree::model::TreeRefusalReason::ConfinementRefused,
                error.to_string(),
            )
        }),
    }
}

fn run(arguments: &TreeRunArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let catalog = tree_catalog(&resolved(&arguments.store)?)?;
    let settings = RunSettings {
        max_steps: arguments.max_steps,
        max_wall_seconds: arguments.max_wall_seconds,
        time,
        runner: tree_runner(arguments.confinement)?,
    };
    let ran = run_tree(
        &resolved(&arguments.store)?,
        &catalog,
        &settings,
        None,
        None,
    )?;
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
    let approval = record(&ApproveRequest {
        store: &store,
        case: &arguments.case,
        capability: &arguments.capability,
        principal: &arguments.principal,
        decision: if arguments.deny {
            ApprovalDecision::Deny
        } else {
            ApprovalDecision::Allow
        },
        action: arguments.action.as_deref(),
        time,
        catalog: &catalog,
    })?;
    println!(
        "{}",
        printable(
            &json!({
                "case": approval.case,
                "capability": approval.capability,
                "decision": format!("{:?}", approval.decision),
                "principal": approval.principal,
                "case_revision": approval.case_revision,
                "at": approval.at.0,
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
