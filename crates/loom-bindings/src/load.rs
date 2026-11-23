//! The load-time check: a decoded document held against the compiled protocol it names.

use crate::{
    FORMAT, MAX_TIMEOUT_SECONDS, MIN_TIMEOUT_SECONDS, RUNS_SPENT, Refusal, refuse, workspace_path,
};
use b10x_canon::model::Protocol;
use loom::bindings::{
    BindingsDocument, BindingsRefusalReason as Reason, BoundAction, Executor, ResultSource,
};
use std::collections::BTreeSet;

/// Holds `document` against `protocol`, the compiled protocol registered as `registration`
/// (`name@major`).
///
/// Refuses the first of: a document for another registration or format; an `artifacts` key that
/// is not a declared artifact or a path that is absolute or leaves the workspace; an
/// `independence` dimension the protocol does not declare; and, per action in document order, an
/// action the protocol does not declare, a duplicate, a member that does not belong to its
/// executor (`tools`, a budget member other than `max_runs`, any executor member of another
/// kind, `exits` on any but a program), an exit code `exits` lists in both classes or in a class
/// and as a key of a `from: exit` table of the binding, an evidence `kind` outside the action's
/// `may_produce`, a `subject` or `upstream` entry
/// that is not a declared artifact, a `timeout_seconds` outside 1 to 3600, and an `inputs` or
/// `outputs` path that is absolute or leaves the workspace, and an `open` executor's `bindings` path
/// that is absolute, leaves the directory of the bindings file or names no file (`.`, a trailing
/// `/`);
/// a `retry_when` kind that is not an evidence kind of the protocol; an evidence `from` that does
/// not fit the executor (`effect` on a program, anything but `effect` on a person or an open,
/// `runs` on an agent or a copy); and a `from: runs` table that lists anything but
/// [`RUNS_SPENT`].
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
    let declared_kinds: BTreeSet<String> = protocol
        .evidence_kinds
        .ids()
        .map(ToString::to_string)
        .collect();
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
        exit_classes(bound)?;
        for condition in &bound.retry_when {
            if !declared_kinds.contains(&condition.kind) {
                return Err(refuse(
                    Reason::UndeclaredEvidenceKind,
                    Some(id),
                    format!("retry_when.{}", condition.kind),
                    format!(
                        "{} is not an evidence kind {registration} declares",
                        condition.kind
                    ),
                ));
            }
        }
        for (index, entry) in bound.evidence.iter().enumerate() {
            let slot = format!("evidence[{index}]");
            let kind = bound.executor.kind;
            let source_fits = match kind {
                Executor::Program => entry.from != ResultSource::Effect,
                Executor::Person | Executor::Open => entry.from == ResultSource::Effect,
                // Loom counts the runs of a program alone.
                Executor::Agent | Executor::Copy => entry.from != ResultSource::Runs,
            };
            if !source_fits {
                return Err(refuse(
                    Reason::MemberNotForExecutor,
                    Some(id),
                    format!("{slot}.from"),
                    format!("{:?} is not a source of a {kind:?} executor", entry.from),
                ));
            }
            if entry.from == ResultSource::Runs
                && !matches!(entry.results.as_slice(), [only] if only.value == RUNS_SPENT)
            {
                let listed: Vec<&str> = entry
                    .results
                    .iter()
                    .map(|rule| rule.value.as_str())
                    .collect();
                return Err(refuse(
                    Reason::InvalidEvidence,
                    Some(id),
                    format!("{slot}.results"),
                    format!(
                        "lists {}; a from: runs table lists {RUNS_SPENT} and nothing else",
                        listed.join(", ")
                    ),
                ));
            }
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
        if let Some(path) = &bound.executor.bindings {
            in_workspace(Some(id), "executor.bindings", path)?;
            if matches!(path.rsplit(['/', '\\']).next(), Some("" | ".")) {
                return Err(refuse(
                    Reason::PathNotInWorkspace,
                    Some(id),
                    "executor.bindings",
                    format!("{path:?} names no file; it is relative to the directory of this file"),
                ));
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
    // `argv` is a program's and a person's (`story:person-action-runs-a-program`): a person's
    // approval runs it.
    if spec.argv.is_some() && !matches!(kind, Executor::Program | Executor::Person) {
        return Err(refuse(
            Reason::MemberNotForExecutor,
            id,
            "executor.argv",
            format!("belongs to the Program and Person executors, not {kind:?}"),
        ));
    }
    let members: [(&str, bool, Executor); 10] = [
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
    for (member, present) in [
        ("exits", bound.exits.is_some()),
        ("handoff_when", !bound.handoff_when.is_empty()),
    ] {
        if present && kind != Executor::Program {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                id,
                member,
                format!("belongs to the Program executor, not {kind:?}"),
            ));
        }
    }
    // A person's program reads `{}` on standard input, and its exit decides only whether the
    // person's effect happened: it takes no arguments and no exit classes.
    for (member, present) in [
        ("arguments", bound.arguments.is_some()),
        ("arguments_from", bound.arguments_from.is_some()),
    ] {
        if present && kind == Executor::Person {
            return Err(refuse(
                Reason::MemberNotForExecutor,
                id,
                member,
                "belongs to a program step, not to the Person executor: a person's program reads {} on standard input",
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
    // A program's runs and a person's program's are counted (`prepare`); nothing else reads the
    // members of `story:rework-allowance-after-a-rejection`. `runs_per` is named last: the other
    // two are read only beside it.
    if !matches!(kind, Executor::Program | Executor::Person) {
        let budget = &bound.budget;
        for (member, present) in [
            ("rework_limit", budget.rework_limit.is_some()),
            ("rework_on", budget.rework_on.is_some()),
            ("runs_per", budget.runs_per.is_some()),
        ] {
            if present {
                return Err(refuse(
                    Reason::MemberNotForExecutor,
                    id,
                    format!("budget.{member}"),
                    format!("belongs to the Program and Person executors, not {kind:?}"),
                ));
            }
        }
    }
    Ok(())
}

/// Refuses an exit code `bound.exits` lists in both classes, or in a class and as a key of one of
/// the binding's `from: exit` tables: such an exit never reaches the table, so it is in one place.
fn exit_classes(bound: &BoundAction) -> Result<(), Refusal> {
    let Some(exits) = &bound.exits else {
        return Ok(());
    };
    let id = Some(bound.action.as_str());
    for (index, code) in exits.not_called.iter().enumerate() {
        if exits.error.contains(code) {
            return Err(refuse(
                Reason::ExitClassConflict,
                id,
                format!("exits.not_called[{index}]"),
                format!("{code} is listed in exits.error too; an exit is in one class"),
            ));
        }
    }
    for (index, entry) in bound.evidence.iter().enumerate() {
        if entry.from != ResultSource::Exit {
            continue;
        }
        for rule in &entry.results {
            let classed = |codes: &[i64]| codes.iter().any(|code| code.to_string() == rule.value);
            let class = if classed(&exits.error) {
                "exits.error"
            } else if classed(&exits.not_called) {
                "exits.not_called"
            } else {
                continue;
            };
            return Err(refuse(
                Reason::ExitClassConflict,
                id,
                format!("evidence[{index}].results.{}", rule.value),
                format!(
                    "{class} lists the exit {}, which never reaches this table",
                    rule.value
                ),
            ));
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
