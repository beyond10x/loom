//! Decoding: the text of a `loom.bindings/1` document into the generated types.

use crate::tree::{self, Map as Mapping, Node as Value};
use crate::{DEFAULT_AGENT_MODEL, DEFAULT_MAX_RUNS, FORMAT, MAX_DOCUMENT_BYTES, Refusal, refuse};
use loom::bindings::{
    ActionBudget, ArtifactPath, BindingsDocument, BindingsRefusalReason as Reason, BoundAction,
    EvidenceEntry, Executor, ExecutorSpec, ExitClasses, ProducerIndependence, ProgramRoot,
    ResultRule, ResultSource, RetryCondition, RunsScope, ToolAllowList, WorkspaceWrites,
};
use loom::json::Value as Json;

type Decoded<T> = Result<T, Refusal>;

const TOP: [&str; 7] = [
    "format",
    "protocol",
    "program_root",
    "workspace",
    "artifacts",
    "independence",
    "actions",
];
const ACTION: [&str; 12] = [
    "action",
    "executor",
    "arguments",
    "arguments_from",
    "evidence",
    "tools",
    "budget",
    "timeout_seconds",
    "retry_when",
    "handoff",
    "handoff_when",
    "exits",
];
const EXITS: [&str; 2] = ["error", "not_called"];
/// The highest exit code an `exits` list holds.
const MAX_EXIT_CODE: i64 = 255;
const EXECUTOR: [&str; 12] = [
    "kind",
    "argv",
    "adapter",
    "model",
    "inputs",
    "outputs",
    "principal",
    "name",
    "each",
    "bindings",
    "evidence",
    "from",
];
const ENTRY: [&str; 6] = ["kind", "subject", "producer", "from", "results", "upstream"];

/// The argv entry a person's approval replaces by its principal
/// (`story:person-action-runs-a-program`): only in a `person` executor's `argv`, only as a whole
/// entry, never the first.
pub const PRINCIPAL_PLACEHOLDER: &str = "{principal}";

/// Whether an argv may hold [`PRINCIPAL_PLACEHOLDER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placeholder {
    /// It may not: a program's `executor.argv`, `arguments_from`, `handoff_when`.
    Refused,
    /// It may, as a whole entry after the first: a person's `executor.argv`.
    Principal,
}
const BUDGET: [&str; 7] = [
    "max_runs",
    "max_turns",
    "max_tokens",
    "max_budget_micro_usd",
    "runs_per",
    "rework_limit",
    "rework_on",
];

/// Decodes a `loom.bindings/1` document from YAML (or JSON).
///
/// # Errors
/// The first defect: text that is not a YAML object, an undeclared member, a missing or other
/// `format`, a malformed `protocol`, `program_root` (`workspace` or `bindings`), `workspace`
/// (`read_write`), `argv`, result
/// table, tool name, budget, timeout or `exits` list (exit codes from 0 to 255), or an
/// executor without the members its kind needs. Whether the actions and artifacts exist in a
/// protocol is [`crate::check`]'s.
pub fn decode(text: &str) -> Decoded<BindingsDocument> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err(refuse(
            Reason::Malformed,
            None,
            "document",
            format!(
                "is {} bytes; at most {MAX_DOCUMENT_BYTES} are read",
                text.len()
            ),
        ));
    }
    let value: Value =
        tree::parse(text).map_err(|e| refuse(Reason::Malformed, None, "document", e))?;
    let top = object(&value, None, "document")?;
    known(top, &TOP, None, "")?;
    let format = match top.get("format") {
        None => {
            return Err(refuse(
                Reason::UnsupportedFormat,
                None,
                "format",
                format!("is missing; the document's format is {FORMAT}"),
            ));
        }
        Some(Value::String(format)) if format == FORMAT => format.clone(),
        Some(other) => {
            return Err(refuse(
                Reason::UnsupportedFormat,
                None,
                "format",
                format!("is {}; the document's format is {FORMAT}", shown(other)),
            ));
        }
    };
    let protocol = match top.get("protocol") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                None,
                "protocol",
                "is missing; it is a registration, name@major",
            ));
        }
        Some(Value::String(protocol)) if registration(protocol) => protocol.clone(),
        Some(other) => {
            return Err(refuse(
                Reason::InvalidProtocol,
                None,
                "protocol",
                format!("is {}; it is a registration, name@major", shown(other)),
            ));
        }
    };
    let program_root = match top.get("program_root") {
        None => None,
        Some(Value::String(root)) if root == "workspace" => Some(ProgramRoot::Workspace),
        Some(Value::String(root)) if root == "bindings" => Some(ProgramRoot::Bindings),
        Some(other) => {
            return Err(refuse(
                Reason::Malformed,
                None,
                "program_root",
                format!("is {}; it is workspace or bindings", shown(other)),
            ));
        }
    };
    let workspace = match top.get("workspace") {
        None => None,
        Some(Value::String(writes)) if writes == "read_write" => Some(WorkspaceWrites::ReadWrite),
        Some(other) => {
            return Err(refuse(
                Reason::Malformed,
                None,
                "workspace",
                format!(
                    "is {}; it is read_write, or left out for the default",
                    shown(other)
                ),
            ));
        }
    };
    Ok(BindingsDocument {
        format,
        protocol,
        program_root,
        workspace,
        artifacts: artifacts(top)?,
        independence: independence(top)?,
        actions: actions(top)?,
    })
}

/// The rule the protocol registry holds a registration to: lowercase letters, digits and hyphens
/// before `@`, starting with a lowercase letter and at most 100 bytes, then a positive major in
/// canonical form (`software.change/1` is registered as `software-change@1`).
fn registration(text: &str) -> bool {
    let Some((stem, version)) = text.split_once('@') else {
        return false;
    };
    if stem.is_empty()
        || stem.len() > 100
        || !stem.as_bytes()[0].is_ascii_lowercase()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return false;
    }
    version
        .parse::<u64>()
        .is_ok_and(|major| major != 0 && major.to_string() == version)
}

fn artifacts(top: &Mapping) -> Decoded<Vec<ArtifactPath>> {
    let Some(value) = top.get("artifacts") else {
        return Ok(Vec::new());
    };
    let map = object(value, None, "artifacts")?;
    let mut out = Vec::new();
    for (key, path) in map.iter() {
        let artifact = key.clone();
        let member = format!("artifacts.{artifact}");
        let path = text_of(path, None, &member)?;
        out.push(ArtifactPath { artifact, path });
    }
    Ok(out)
}

fn independence(top: &Mapping) -> Decoded<Vec<ProducerIndependence>> {
    let Some(value) = top.get("independence") else {
        return Ok(Vec::new());
    };
    let map = object(value, None, "independence")?;
    let mut out = Vec::new();
    for (key, dimensions) in map.iter() {
        let producer = key.clone();
        let member = format!("independence.{producer}");
        let dimensions = strings(dimensions, None, &member, Reason::Malformed)?;
        out.push(ProducerIndependence {
            producer,
            dimensions,
        });
    }
    Ok(out)
}

fn actions(top: &Mapping) -> Decoded<Vec<BoundAction>> {
    let Some(value) = top.get("actions") else {
        return Ok(Vec::new());
    };
    let Value::Sequence(items) = value else {
        return Err(refuse(
            Reason::Malformed,
            None,
            "actions",
            format!("is {}; it is a list of bindings", shown(value)),
        ));
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| bound_action(item, index))
        .collect()
}

fn bound_action(value: &Value, index: usize) -> Decoded<BoundAction> {
    let slot = format!("actions[{index}]");
    let map = object(value, None, &slot)?;
    let id = match map.get("action") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                None,
                format!("{slot}.action"),
                "is missing; it is the id of the protocol's action",
            ));
        }
        Some(id) => non_empty(id, None, &format!("{slot}.action"))?,
    };
    let action = Some(id.as_str());
    known(map, &ACTION, action, "")?;
    let executor = match map.get("executor") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                action,
                "executor",
                "is missing; it is one of program, agent, person, open, copy",
            ));
        }
        Some(value) => executor(value, action)?,
    };
    let arguments = match map.get("arguments") {
        None => None,
        Some(value @ Value::Mapping(_)) => Some(json(value, action, "arguments")?),
        Some(other) => {
            return Err(refuse(
                Reason::Malformed,
                action,
                "arguments",
                format!("is {}; it is a literal object", shown(other)),
            ));
        }
    };
    let arguments_from = match map.get("arguments_from") {
        None => None,
        Some(value) => Some(argv(value, action, "arguments_from", Placeholder::Refused)?),
    };
    if arguments.is_some() && arguments_from.is_some() {
        return Err(refuse(
            Reason::Malformed,
            action,
            "arguments_from",
            "is given with arguments; the arguments are one or the other",
        ));
    }
    let evidence = match map.get("evidence") {
        None => Vec::new(),
        Some(Value::Sequence(entries)) => entries
            .iter()
            .enumerate()
            .map(|(i, entry)| evidence_entry(entry, action, i, executor.kind == Executor::Person))
            .collect::<Decoded<Vec<_>>>()?,
        Some(other) => {
            return Err(refuse(
                Reason::Malformed,
                action,
                "evidence",
                format!("is {}; it is a list of entries", shown(other)),
            ));
        }
    };
    let tools = map
        .get("tools")
        .map(|value| tools(value, action))
        .transpose()?;
    let budget = budget(map.get("budget"), action)?;
    let timeout_seconds = match map.get("timeout_seconds") {
        None => None,
        Some(value) => Some(value.as_i64().ok_or_else(|| {
            refuse(
                Reason::Malformed,
                action,
                "timeout_seconds",
                format!("is {}; it is a whole number of seconds", shown(value)),
            )
        })?),
    };
    let retry_when = match map.get("retry_when") {
        None => Vec::new(),
        Some(value) => retry_when(value, action)?,
    };
    rework(&budget, &retry_when, action)?;
    let handoff = map
        .get("handoff")
        .map(|value| non_empty(value, action, "handoff"))
        .transpose()?;
    let handoff_when = match map.get("handoff_when") {
        None => Vec::new(),
        Some(value) => argv(value, action, "handoff_when", Placeholder::Refused)?,
    };
    if !handoff_when.is_empty() && handoff.is_none() {
        return Err(refuse(
            Reason::MissingMember,
            action,
            "handoff",
            "is missing; handoff_when decides when the handoff is offered, so it needs the handoff text",
        ));
    }
    let exits = map
        .get("exits")
        .map(|value| exits(value, action))
        .transpose()?;
    Ok(BoundAction {
        action: id.clone(),
        executor,
        arguments,
        arguments_from,
        evidence,
        tools,
        budget,
        timeout_seconds,
        retry_when,
        handoff,
        handoff_when,
        exits,
    })
}

/// The exit classes of a program binding: `error` and `not_called`, each a list of exit codes
/// from 0 to [`MAX_EXIT_CODE`], empty when the document leaves it out.
fn exits(value: &Value, action: Option<&str>) -> Decoded<ExitClasses> {
    let map = object(value, action, "exits")?;
    known(map, &EXITS, action, "exits.")?;
    let codes = |name: &str| -> Decoded<Vec<i64>> {
        let member = format!("exits.{name}");
        let Some(value) = map.get(name) else {
            return Ok(Vec::new());
        };
        let Value::Sequence(items) = value else {
            return Err(refuse(
                Reason::Malformed,
                action,
                member,
                format!("is {}; it is a list of exit codes", shown(value)),
            ));
        };
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_i64()
                    .filter(|code| (0..=MAX_EXIT_CODE).contains(code))
                    .ok_or_else(|| {
                        refuse(
                            Reason::Malformed,
                            action,
                            format!("{member}[{index}]"),
                            format!(
                                "is {}; it is an exit code from 0 to {MAX_EXIT_CODE}",
                                shown(item)
                            ),
                        )
                    })
            })
            .collect()
    };
    Ok(ExitClasses {
        error: codes("error")?,
        not_called: codes("not_called")?,
    })
}

fn executor(value: &Value, action: Option<&str>) -> Decoded<ExecutorSpec> {
    let map = object(value, action, "executor")?;
    known(map, &EXECUTOR, action, "executor.")?;
    let kind = match map.get("kind") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                action,
                "executor.kind",
                "is missing; it is one of program, agent, person, open, copy",
            ));
        }
        Some(Value::String(kind)) => match kind.as_str() {
            "program" => Executor::Program,
            "agent" => Executor::Agent,
            "person" => Executor::Person,
            "open" => Executor::Open,
            "copy" => Executor::Copy,
            other => {
                return Err(refuse(
                    Reason::Malformed,
                    action,
                    "executor.kind",
                    format!("is {other}; it is one of program, agent, person, open, copy"),
                ));
            }
        },
        Some(other) => {
            return Err(refuse(
                Reason::Malformed,
                action,
                "executor.kind",
                format!(
                    "is {}; it is one of program, agent, person, open, copy",
                    shown(other)
                ),
            ));
        }
    };
    let text = |name: &str| -> Decoded<Option<String>> {
        map.get(name)
            .map(|value| non_empty(value, action, &format!("executor.{name}")))
            .transpose()
    };
    let paths = |name: &str| -> Decoded<Option<Vec<String>>> {
        map.get(name)
            .map(|value| {
                strings(
                    value,
                    action,
                    &format!("executor.{name}"),
                    Reason::Malformed,
                )
            })
            .transpose()
    };
    // The kind is known before the argv: only a person's argv may name its principal.
    let placeholder = if kind == Executor::Person {
        Placeholder::Principal
    } else {
        Placeholder::Refused
    };
    let mut spec = ExecutorSpec {
        kind,
        argv: map
            .get("argv")
            .map(|value| argv(value, action, "executor.argv", placeholder))
            .transpose()?,
        adapter: text("adapter")?,
        model: text("model")?,
        inputs: paths("inputs")?,
        outputs: paths("outputs")?,
        principal: text("principal")?,
        name: text("name")?,
        each: text("each")?,
        bindings: text("bindings")?,
        evidence: text("evidence")?,
        from: text("from")?,
    };
    let missing = |member: &str, what: &str| {
        refuse(
            Reason::MissingMember,
            action,
            format!("executor.{member}"),
            format!("is missing; {what}"),
        )
    };
    match kind {
        Executor::Program => {
            if spec.argv.is_none() {
                return Err(missing("argv", "a program executor runs an argv"));
            }
        }
        Executor::Agent => {
            if spec.adapter.is_none() {
                return Err(missing("adapter", "an agent executor names its adapter"));
            }
            if spec.model.is_none() {
                spec.model = Some(DEFAULT_AGENT_MODEL.to_owned());
            }
        }
        Executor::Person => {
            if spec.principal.is_none() {
                return Err(missing(
                    "principal",
                    "a person executor names the principal",
                ));
            }
        }
        Executor::Open => {
            if spec.bindings.is_none() {
                return Err(missing(
                    "bindings",
                    "an open executor names the child's bindings file",
                ));
            }
            match (&spec.name, &spec.each) {
                (None, None) => {
                    return Err(missing("name", "an open executor takes a name or an each"));
                }
                (Some(_), Some(_)) => {
                    return Err(refuse(
                        Reason::Malformed,
                        action,
                        "executor.name",
                        "is given with each; an open executor takes one of them",
                    ));
                }
                _ => {}
            }
        }
        Executor::Copy => {
            if spec.evidence.is_none() {
                return Err(missing(
                    "evidence",
                    "a copy executor names the evidence kind",
                ));
            }
            if spec.from.is_none() {
                return Err(missing(
                    "from",
                    "a copy executor names the protocol id it copies from",
                ));
            }
        }
    }
    Ok(spec)
}

/// The argv `member` holds: a list of strings, never a shell string, its first entry naming the
/// program. [`PRINCIPAL_PLACEHOLDER`] stands only where `placeholder` allows it, as a whole entry
/// after the first; anywhere else an entry holding it is refused.
fn argv(
    value: &Value,
    action: Option<&str>,
    member: &str,
    placeholder: Placeholder,
) -> Decoded<Vec<String>> {
    let Value::Sequence(items) = value else {
        return Err(refuse(
            Reason::InvalidArgv,
            action,
            member,
            format!(
                "is {}; it is a list of strings, never a shell string",
                shown(value)
            ),
        ));
    };
    if items.is_empty() {
        return Err(refuse(Reason::InvalidArgv, action, member, "is empty"));
    }
    let mut out = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            Value::String(text) => out.push(text.clone()),
            // `{principal}` unquoted in a YAML flow list is a mapping: say how it is written.
            Value::Mapping(entries)
                if entries.keys().count() == 1
                    && entries
                        .get("principal")
                        .is_some_and(|value| matches!(value, Value::Null)) =>
            {
                return Err(refuse(
                    Reason::InvalidArgv,
                    action,
                    member,
                    format!(
                        "entry {index} is the mapping {PRINCIPAL_PLACEHOLDER}; the placeholder is the string \"{PRINCIPAL_PLACEHOLDER}\", quoted"
                    ),
                ));
            }
            other => {
                return Err(refuse(
                    Reason::InvalidArgv,
                    action,
                    member,
                    format!("entry {index} is {}; it is a string", shown(other)),
                ));
            }
        }
    }
    if out[0].is_empty() {
        return Err(refuse(
            Reason::InvalidArgv,
            action,
            member,
            "names no program: its first entry is empty",
        ));
    }
    for (index, entry) in out.iter().enumerate() {
        if !entry.contains(PRINCIPAL_PLACEHOLDER) {
            continue;
        }
        let why = if placeholder == Placeholder::Refused {
            format!(
                "entry {index} holds {PRINCIPAL_PLACEHOLDER}, which is replaced only in a person executor's argv"
            )
        } else if index == 0 {
            format!("entry 0 is {PRINCIPAL_PLACEHOLDER}: the placeholder never names the program")
        } else if entry != PRINCIPAL_PLACEHOLDER {
            format!(
                "entry {index} holds {PRINCIPAL_PLACEHOLDER} and more; the placeholder is a whole entry"
            )
        } else {
            continue;
        };
        return Err(refuse(Reason::InvalidArgv, action, member, why));
    }
    Ok(out)
}

/// One evidence entry of `action`; `person`: the action is bound to a person, whose `effect` table
/// may name results other than `performed`.
fn evidence_entry(
    value: &Value,
    action: Option<&str>,
    index: usize,
    person: bool,
) -> Decoded<EvidenceEntry> {
    let slot = format!("evidence[{index}]");
    let map = object(value, action, &slot)?;
    known(map, &ENTRY, action, &format!("{slot}."))?;
    let required = |name: &str| -> Decoded<String> {
        let member = format!("{slot}.{name}");
        match map.get(name) {
            None => Err(refuse(Reason::MissingMember, action, member, "is missing")),
            Some(Value::String(text)) if !text.is_empty() => Ok(text.clone()),
            Some(other) => Err(refuse(
                Reason::InvalidEvidence,
                action,
                member,
                format!("is {}; it is a non-empty string", shown(other)),
            )),
        }
    };
    let kind = required("kind")?;
    let subject = required("subject")?;
    let producer = required("producer")?;
    let from_member = format!("{slot}.from");
    let (from, pointer) = match map.get("from") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                action,
                from_member,
                "is missing",
            ));
        }
        Some(Value::String(text)) if text == "exit" => (ResultSource::Exit, None),
        Some(Value::String(text)) if text == "effect" => (ResultSource::Effect, None),
        Some(Value::String(text)) if text == "runs" => (ResultSource::Runs, None),
        Some(Value::String(text)) if json_pointer(text) => {
            (ResultSource::Report, Some(text.clone()))
        }
        Some(other) => {
            return Err(refuse(
                Reason::InvalidEvidence,
                action,
                from_member,
                format!(
                    "is {}; it is exit, effect, runs or a JSON pointer into the program's report",
                    shown(other)
                ),
            ));
        }
    };
    let results_member = format!("{slot}.results");
    let results = match map.get("results") {
        None => {
            return Err(refuse(
                Reason::MissingMember,
                action,
                results_member,
                "is missing",
            ));
        }
        Some(Value::Mapping(rows)) if !rows.is_empty() => {
            let mut table = Vec::new();
            for (key, result) in rows.iter() {
                let value = key.clone();
                let member = format!("{results_member}.{value}");
                let result = match result {
                    Value::Null => None,
                    Value::String(text) if !text.is_empty() => Some(text.clone()),
                    other => {
                        return Err(refuse(
                            Reason::InvalidEvidence,
                            action,
                            member,
                            format!("is {}; it is a result name or null", shown(other)),
                        ));
                    }
                };
                // A person names the result of the action taking it; any other effect is performed.
                if from == ResultSource::Effect
                    && value != "performed"
                    && (!person || value == crate::ANY_KEY)
                {
                    return Err(refuse(
                        Reason::InvalidEffectResult,
                        action,
                        member,
                        format!("the key {value} is not performed, the one value of an effect"),
                    ));
                }
                table.push(ResultRule { value, result });
            }
            table
        }
        Some(other) => {
            return Err(refuse(
                Reason::InvalidEvidence,
                action,
                results_member,
                format!(
                    "is {}; it is a non-empty table from value to result",
                    shown(other)
                ),
            ));
        }
    };
    let upstream = match map.get("upstream") {
        None => Vec::new(),
        Some(value) => strings(
            value,
            action,
            &format!("{slot}.upstream"),
            Reason::Malformed,
        )?,
    };
    Ok(EvidenceEntry {
        kind,
        subject,
        producer,
        from,
        pointer,
        results,
        upstream,
    })
}

/// An RFC 6901 JSON pointer that is not the empty one: it starts with `/` and `~` escapes only
/// `0` and `1`.
fn json_pointer(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('/') else {
        return false;
    };
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

fn tools(value: &Value, action: Option<&str>) -> Decoded<ToolAllowList> {
    let map = object(value, action, "tools")?;
    known(map, &["allow"], action, "tools.")?;
    let Some(allow) = map.get("allow") else {
        return Err(refuse(
            Reason::MissingMember,
            action,
            "tools.allow",
            "is missing; it is the list of tool names the agent may be offered",
        ));
    };
    let Value::Sequence(items) = allow else {
        return Err(refuse(
            Reason::Malformed,
            action,
            "tools.allow",
            format!("is {}; it is a list of tool names", shown(allow)),
        ));
    };
    let mut names = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let member = format!("tools.allow[{index}]");
        let Value::String(name) = item else {
            return Err(refuse(
                Reason::InvalidToolName,
                action,
                member,
                format!("entry is {}; it is a tool name", shown(item)),
            ));
        };
        let defect = if name.is_empty() {
            Some("is empty")
        } else if name.contains(',') {
            Some("holds a comma")
        } else if name.chars().any(char::is_whitespace) {
            Some("holds white space")
        } else {
            None
        };
        if let Some(defect) = defect {
            return Err(refuse(
                Reason::InvalidToolName,
                action,
                member,
                format!(
                    "entry {name:?} {defect}; the session executor joins the names with commas"
                ),
            ));
        }
        names.push(name.clone());
    }
    Ok(ToolAllowList { allow: names })
}

fn budget(value: Option<&Value>, action: Option<&str>) -> Decoded<ActionBudget> {
    let mut budget = ActionBudget {
        max_runs: DEFAULT_MAX_RUNS,
        max_turns: None,
        max_tokens: None,
        max_budget_micro_usd: None,
        runs_per: None,
        rework_limit: None,
        rework_on: None,
    };
    let Some(value) = value else {
        return Ok(budget);
    };
    let map = object(value, action, "budget")?;
    known(map, &BUDGET, action, "budget.")?;
    for (key, member) in map.iter() {
        let name = key.clone();
        match name.as_str() {
            "runs_per" => {
                budget.runs_per = Some(runs_scope(member, action)?);
                continue;
            }
            "rework_on" => {
                budget.rework_on = Some(rework_on(member, action)?);
                continue;
            }
            _ => {}
        }
        let number = member.as_i64().filter(|n| *n >= 1).ok_or_else(|| {
            refuse(
                Reason::InvalidBudget,
                action,
                format!("budget.{name}"),
                format!("is {}; it is a whole number from 1", shown(member)),
            )
        })?;
        match name.as_str() {
            "max_runs" => budget.max_runs = number,
            "max_turns" => budget.max_turns = Some(number),
            "max_tokens" => budget.max_tokens = Some(number),
            "rework_limit" => budget.rework_limit = Some(number),
            _ => budget.max_budget_micro_usd = Some(number),
        }
    }
    Ok(budget)
}

/// `budget.runs_per`: `case` or `rejection` (`story:rework-allowance-after-a-rejection`).
fn runs_scope(value: &Value, action: Option<&str>) -> Decoded<RunsScope> {
    match value {
        Value::String(scope) if scope == "case" => Ok(RunsScope::Case),
        Value::String(scope) if scope == "rejection" => Ok(RunsScope::Rejection),
        other => Err(refuse(
            Reason::InvalidBudget,
            action,
            "budget.runs_per",
            format!("is {}; it is case or rejection", shown(other)),
        )),
    }
}

/// `budget.rework_on`: the shape of `retry_when`, never empty and naming no kind with an empty
/// list, or it would name no rejection.
fn rework_on(value: &Value, action: Option<&str>) -> Decoded<Vec<RetryCondition>> {
    let member = "budget.rework_on";
    let conditions = conditions(value, action, member, Reason::InvalidBudget)?;
    if conditions.is_empty() {
        return Err(refuse(
            Reason::InvalidBudget,
            action,
            member,
            "is empty; it names the kinds and results that are rejections",
        ));
    }
    if let Some(empty) = conditions
        .iter()
        .find(|condition| condition.results.is_empty())
    {
        return Err(refuse(
            Reason::InvalidBudget,
            action,
            format!("{member}.{}", empty.kind),
            "is empty; it lists the results of the kind that are rejections",
        ));
    }
    Ok(conditions)
}

/// The rework members against `runs_per` and the decoded `retry_when`: `rework_limit` and
/// `rework_on` are read only under `runs_per: rejection`, which needs a `retry_when` to name a
/// rejection (a kind with no result names none), and each pair `rework_on` lists is one
/// `retry_when` lists.
fn rework(
    budget: &ActionBudget,
    retry_when: &[RetryCondition],
    action: Option<&str>,
) -> Decoded<()> {
    if budget.runs_per != Some(RunsScope::Rejection) {
        for (member, present) in [
            ("rework_limit", budget.rework_limit.is_some()),
            ("rework_on", budget.rework_on.is_some()),
        ] {
            if present {
                return Err(refuse(
                    Reason::InvalidBudget,
                    action,
                    format!("budget.{member}"),
                    "is read only under runs_per: rejection",
                ));
            }
        }
        return Ok(());
    }
    if retry_when
        .iter()
        .all(|condition| condition.results.is_empty())
    {
        return Err(refuse(
            Reason::InvalidBudget,
            action,
            "budget.runs_per",
            "is rejection and retry_when lists no result, so no record could open an allowance",
        ));
    }
    for condition in budget.rework_on.iter().flatten() {
        for result in &condition.results {
            if !crate::retry::lists(retry_when, &condition.kind, result) {
                return Err(refuse(
                    Reason::InvalidBudget,
                    action,
                    "budget.rework_on",
                    format!(
                        "lists {} {result}, which retry_when does not list; a rejection is a record that sends the action back",
                        condition.kind
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn retry_when(value: &Value, action: Option<&str>) -> Decoded<Vec<RetryCondition>> {
    conditions(value, action, "retry_when", Reason::Malformed)
}

/// A map of evidence kinds to lists of results at `member`, a defect refused `reason`.
fn conditions(
    value: &Value,
    action: Option<&str>,
    member: &str,
    reason: Reason,
) -> Decoded<Vec<RetryCondition>> {
    let map = value.as_mapping().ok_or_else(|| {
        refuse(
            reason,
            action,
            member,
            format!("is {}; it is an object", shown(value)),
        )
    })?;
    let mut out = Vec::new();
    for (key, results) in map.iter() {
        let kind = key.clone();
        let member = format!("{member}.{kind}");
        let results = strings(results, action, &member, reason)?;
        if let Some(index) = results.iter().position(String::is_empty) {
            return Err(refuse(
                reason,
                action,
                format!("{member}[{index}]"),
                "is empty",
            ));
        }
        out.push(RetryCondition { kind, results });
    }
    Ok(out)
}

fn json(value: &Value, action: Option<&str>, member: &str) -> Decoded<Json> {
    Ok(match value {
        Value::Null => Json::Null,
        Value::Bool(flag) => Json::Bool(*flag),
        Value::Int(number) => Json::Number(number.to_string()),
        Value::Big(digits) => Json::Number(digits.clone()),
        Value::Real(number) => {
            if !number.is_finite() {
                return Err(refuse(
                    Reason::Malformed,
                    action,
                    member,
                    "holds a number JSON cannot write",
                ));
            }
            Json::Number(format!("{number:?}"))
        }
        Value::String(text) => Json::Text(text.clone()),
        Value::Sequence(items) => Json::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, item)| json(item, action, &format!("{member}[{i}]")))
                .collect::<Decoded<Vec<_>>>()?,
        ),
        Value::Mapping(map) => {
            let mut members = Vec::new();
            for (key, item) in map.iter() {
                let name = key.clone();
                let inner = json(item, action, &format!("{member}.{name}"))?;
                members.push((name, inner));
            }
            Json::Object(members)
        }
    })
}

fn object<'a>(value: &'a Value, action: Option<&str>, member: &str) -> Decoded<&'a Mapping> {
    value.as_mapping().ok_or_else(|| {
        refuse(
            Reason::Malformed,
            action,
            member,
            format!("is {}; it is an object", shown(value)),
        )
    })
}

/// Refuses the first member of `map` that is not one of `allowed`.
fn known(map: &Mapping, allowed: &[&str], action: Option<&str>, prefix: &str) -> Decoded<()> {
    for name in map.keys() {
        if !allowed.contains(&name.as_str()) {
            return Err(refuse(
                Reason::UndeclaredMember,
                action,
                format!("{prefix}{name}"),
                format!(
                    "is not a member here; the members are {}",
                    allowed.join(", ")
                ),
            ));
        }
    }
    Ok(())
}

fn text_of(value: &Value, action: Option<&str>, member: &str) -> Decoded<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        other => Err(refuse(
            Reason::Malformed,
            action,
            member,
            format!("is {}; it is a string", shown(other)),
        )),
    }
}

fn non_empty(value: &Value, action: Option<&str>, member: &str) -> Decoded<String> {
    let text = text_of(value, action, member)?;
    if text.is_empty() {
        return Err(refuse(Reason::Malformed, action, member, "is empty"));
    }
    Ok(text)
}

fn strings(
    value: &Value,
    action: Option<&str>,
    member: &str,
    reason: Reason,
) -> Decoded<Vec<String>> {
    let Value::Sequence(items) = value else {
        return Err(refuse(
            reason,
            action,
            member,
            format!("is {}; it is a list of strings", shown(value)),
        ));
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| match item {
            Value::String(text) => Ok(text.clone()),
            other => Err(refuse(
                reason,
                action,
                format!("{member}[{index}]"),
                format!("is {}; it is a string", shown(other)),
            )),
        })
        .collect()
}

/// What a value is, for a message.
fn shown(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        Value::Int(number) => number.to_string(),
        Value::Big(digits) => digits.clone(),
        Value::Real(number) => number.to_string(),
        Value::String(text) => format!("{text:?}"),
        Value::Sequence(_) => "a list".to_owned(),
        Value::Mapping(_) => "an object".to_owned(),
    }
}
