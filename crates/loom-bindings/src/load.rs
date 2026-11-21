//! The load-time check: a decoded document held against the compiled protocol it names.

use crate::{FORMAT, MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS, Refusal, refuse, workspace_path};
use b10x_canon::model::Protocol;
use loom::bindings::{BindingsDocument, BindingsRefusalReason as Reason, BoundAction, Executor};
use std::collections::BTreeSet;

/// Holds `document` against `protocol`, the compiled protocol registered as `registration`
/// (`name@major`).
///
/// Refuses the first of: a document for another registration or format; an `artifacts` key that
/// is not a declared artifact or a path that is absolute or leaves the workspace; an
/// `independence` dimension the protocol does not declare; and, per action in document order, an
/// action the protocol does not declare, a duplicate, a member that does not belong to its
/// executor (`tools`, a budget member other than `max_runs`, any executor member of another
/// kind), an evidence `kind` outside the action's `may_produce`, a `subject` or `upstream` entry
/// that is not a declared artifact, a `timeout_seconds` outside 1 to 3600, and an `inputs` or
/// `outputs` path that is absolute or leaves the workspace.
///
/// An action the document binds nothing to is not an error; [`unbound`] lists them.
///
/// # Errors
/// The first defect found, naming the action and the member.
pub fn check(
    document: &BindingsDocument,
    registration: &str,
    protocol: &Protocol,
) -> Result<(), Refusal> {
    if document.format != FORMAT {
        return Err(refuse(
            Reason::UnsupportedFormat,
            None,
            "format",
            format!("is {}; the document's format is {FORMAT}", document.format),
        ));
    }
    if document.protocol != registration {
        return Err(refuse(
            Reason::ProtocolMismatch,
            None,
            "protocol",
            format!(
                "is {}; the protocol this document is loaded for is {registration}",
                document.protocol
            ),
        ));
    }
    let artifacts: BTreeSet<String> = protocol.artifacts.ids().map(ToString::to_string).collect();
    for entry in &document.artifacts {
        let member = format!("artifacts.{}", entry.artifact);
        if !artifacts.contains(&entry.artifact) {
            return Err(undeclared_artifact(
                None,
                member,
                &entry.artifact,
                registration,
            ));
        }
        in_workspace(None, &member, &entry.path)?;
    }
    let dimensions: BTreeSet<String> = protocol
        .independence_dimensions
        .ids()
        .map(ToString::to_string)
        .collect();
    for entry in &document.independence {
        for (index, dimension) in entry.dimensions.iter().enumerate() {
            if !dimensions.contains(dimension) {
                return Err(refuse(
                    Reason::UndeclaredDimension,
                    None,
                    format!("independence.{}[{index}]", entry.producer),
                    format!("{dimension} is not an independence dimension of {registration}"),
                ));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for bound in &document.actions {
        let id = bound.action.as_str();
        let Some((_, declared)) = protocol
            .actions
            .iter()
            .find(|(declared, _)| declared.to_string() == id)
        else {
            return Err(refuse(
                Reason::UnknownAction,
                Some(id),
                "action",
                format!("{registration} declares no action {id}"),
            ));
        };
        if !seen.insert(id) {
            return Err(refuse(
                Reason::DuplicateAction,
                Some(id),
                "action",
                "is bound more than once",
            ));
        }
        let produces: Vec<String> = declared
            .may_produce
            .iter()
            .map(|production| production.evidence.to_string())
            .collect();
        fit(bound)?;
        for (index, entry) in bound.evidence.iter().enumerate() {
            let slot = format!("evidence[{index}]");
            if !produces.contains(&entry.kind) {
                return Err(not_producible(
                    id,
                    format!("{slot}.kind"),
                    &entry.kind,
                    registration,
                ));
            }
            if !artifacts.contains(&entry.subject) {
                return Err(undeclared_artifact(
                    Some(id),
                    format!("{slot}.subject"),
                    &entry.subject,
                    registration,
                ));
            }
            for (at, upstream) in entry.upstream.iter().enumerate() {
                if !artifacts.contains(upstream) {
                    return Err(undeclared_artifact(
                        Some(id),
                        format!("{slot}.upstream[{at}]"),
                        upstream,
                        registration,
                    ));
                }
            }
        }
        if let Some(kind) = &bound.executor.evidence
            && !produces.contains(kind)
        {
            return Err(not_producible(
                id,
                "executor.evidence".to_owned(),
                kind,
                registration,
            ));
        }
        if let Some(seconds) = bound.timeout_seconds
            && !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&seconds)
        {
            return Err(refuse(
                Reason::TimeoutOutOfRange,
                Some(id),
                "timeout_seconds",
                format!("{seconds} is not from {MIN_TIMEOUT_SECONDS} to {MAX_TIMEOUT_SECONDS}"),
            ));
        }
        for (name, paths) in [
            ("inputs", &bound.executor.inputs),
            ("outputs", &bound.executor.outputs),
        ] {
            for (index, path) in paths.iter().flatten().enumerate() {
                in_workspace(Some(id), &format!("executor.{name}[{index}]"), path)?;
            }
        }
    }
    Ok(())
}

/// The actions `protocol` declares, in declaration order, that `document` binds nothing to.
pub fn unbound(document: &BindingsDocument, protocol: &Protocol) -> Vec<String> {
    protocol
        .actions
        .ids()
        .map(ToString::to_string)
        .filter(|id| !document.actions.iter().any(|bound| &bound.action == id))
        .collect()
}

/// Refuses a member that belongs to an executor other than the action's own.
fn fit(bound: &BoundAction) -> Result<(), Refusal> {
    let id = Some(bound.action.as_str());
    let spec = &bound.executor;
    let kind = spec.kind;
    let members: [(&str, bool, Executor); 11] = [
        ("argv", spec.argv.is_some(), Executor::Program),
        ("adapter", spec.adapter.is_some(), Executor::Agent),
        ("model", spec.model.is_some(), Executor::Agent),
        ("inputs", spec.inputs.is_some(), Executor::Agent),
        ("outputs", spec.outputs.is_some(), Executor::Agent),
        ("principal", spec.principal.is_some(), Executor::Person),
        ("name", spec.name.is_some(), Executor::Open),
        ("each", spec.each.is_some(), Executor::Open),
        ("bindings", spec.bindings.is_some(), Executor::Open),
        ("evidence", spec.evidence.is_some(), Executor::Copy),
        ("from", spec.from.is_some(), Executor::Copy),
    ];
    for (member, present, owner) in members {
        if present && owner != kind {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                id,
                format!("executor.{member}"),
                format!("belongs to the {owner:?} executor, not {kind:?}"),
            ));
        }
    }
    if kind != Executor::Agent {
        if bound.tools.is_some() {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                id,
                "tools",
                format!("belongs to the Agent executor, not {kind:?}"),
            ));
        }
        let budget = &bound.budget;
        for (member, present) in [
            ("max_turns", budget.max_turns.is_some()),
            ("max_tokens", budget.max_tokens.is_some()),
            (
                "max_budget_micro_usd",
                budget.max_budget_micro_usd.is_some(),
            ),
        ] {
            if present {
                return Err(refuse(
                    Reason::MemberNotForExecutor,
                    id,
                    format!("budget.{member}"),
                    format!("belongs to the Agent executor, not {kind:?}"),
                ));
            }
        }
    }
    Ok(())
}

fn in_workspace(action: Option<&str>, member: &str, path: &str) -> Result<(), Refusal> {
    workspace_path(path).map_err(|why| {
        refuse(
            Reason::PathNotInWorkspace,
            action,
            member,
            format!("{path:?} {why}; a path is relative to the workspace"),
        )
    })
}

fn undeclared_artifact(
    action: Option<&str>,
    member: String,
    artifact: &str,
    registration: &str,
) -> Refusal {
    refuse(
        Reason::UndeclaredArtifact,
        action,
        member,
        format!("{artifact} is not an artifact {registration} declares"),
    )
}

fn not_producible(action: &str, member: String, kind: &str, registration: &str) -> Refusal {
    refuse(
        Reason::KindNotProducible,
        Some(action),
        member,
        format!("{kind} is not in the may_produce of {action} in {registration}"),
    )
}
