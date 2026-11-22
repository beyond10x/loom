//! `b10x-loom tree`: the flags of the tree commands mapped onto `b10x-loom-tree`, what each
//! prints and the exit status it ends with.
//!
//! A command's document (the opened tree, the status, the verdict) is one JSON object on standard
//! output; a refusal is one line on standard error, every value in it through
//! [`printable`](b10x_loom_intake_slice::run::printable), so no text a file or a program wrote can
//! move the cursor or start a line.

use b10x_loom_intake_slice::run::printable;
use b10x_loom_tree::model::ApprovalDecision;
use b10x_loom_tree::{
    ApproveRequest, OpenRequest, Refused, TreeTime, approve as record, open_tree, refusal_json,
    status_json, tree_status, verify_json, verify_tree,
};
use loom_protocols::{InstallStore, ProtocolCatalog};
use serde_json::json;

use crate::{
    TREE_LOG_FAILED, TREE_REFUSED, TreeApproveArgs, TreeCommand, TreeOpenArgs, TreeStoreArgs,
};

/// Runs `command` and answers its exit status.
pub fn command(command: &TreeCommand) -> u8 {
    let ended = match command {
        TreeCommand::Open(arguments) => open(arguments),
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

fn open(arguments: &TreeOpenArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let catalog = host_catalog()?;
    let opened = open_tree(&OpenRequest {
        store: arguments.store.clone(),
        workspace: arguments.workspace.clone(),
        protocol: arguments.protocol.clone(),
        bindings: arguments.bindings.clone(),
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

fn approve(arguments: &TreeApproveArgs) -> Result<u8, Refused> {
    let time = TreeTime::from_flag(arguments.at.as_deref())?;
    let catalog = host_catalog()?;
    let approval = record(&ApproveRequest {
        store: &arguments.store,
        case: &arguments.case,
        capability: &arguments.capability,
        principal: &arguments.principal,
        decision: if arguments.deny {
            ApprovalDecision::Deny
        } else {
            ApprovalDecision::Allow
        },
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
    let catalog = host_catalog()?;
    let status = tree_status(&arguments.store, &catalog)?;
    println!("{}", printable(&status_json(&status).to_string()));
    Ok(0)
}

fn verify(arguments: &TreeStoreArgs) -> Result<u8, Refused> {
    let catalog = host_catalog()?;
    let verified = verify_tree(&arguments.store, &catalog)?;
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
