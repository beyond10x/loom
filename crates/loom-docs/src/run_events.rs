//! Holds the hand-written run events page to the `intake.events` declaration it documents.
//!
//! The page carries pasted output, so it cannot be generated; it is checked instead. It must name
//! the stream's tag and schema version member, every schema version, every record kind as its own
//! `### <Kind>` section, and in that section every field the kind's record declares. A recorded
//! line on the page must carry the version the writer writes.
use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value, json};

/// The page, relative to the repository root.
pub const PAGE: &str = "website/docs/reference/run-events.md";

/// The ESS system that declares the stream.
pub const SYSTEM: &str = "ess/intake";

const LINE: &str = "intake.events.RunEventLine";
const UNION: &str = "intake.events.RunEvent";
const VERSION: &str = "intake.events.SchemaVersion";

fn declared<'a>(model: &'a Value, name: &str) -> Result<&'a Value> {
    model["types"]
        .get(name)
        .map(|declaration| &declaration["body"])
        .with_context(|| format!("compiled model: {name} is not declared"))
}

fn field_names(body: &Value) -> Vec<&str> {
    body["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|field| field["name"].as_str())
        .collect()
}

/// The text of the `### <kind>` section of `page`, up to the next heading of level 2 or 3.
fn section<'a>(page: &'a str, kind: &str) -> Option<&'a str> {
    let heading = format!("\n### {kind}\n");
    let start = page.find(&heading)? + heading.len();
    let rest = &page[start..];
    let end = rest
        .find("\n### ")
        .into_iter()
        .chain(rest.find("\n## "))
        .min()
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Everything `page` leaves out of the stream `model` declares; `schema_version` is the number
/// the writer writes.
pub fn problems(model: &Value, page: &str, schema_version: u64) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let union = declared(model, UNION)?;
    let tag = union["tag"]
        .as_str()
        .with_context(|| format!("compiled model: {UNION} has no tag"))?;
    let line = declared(model, LINE)?;
    for member in field_names(line)
        .into_iter()
        .filter(|name| *name != "event")
        .chain([tag])
    {
        if !page.contains(&format!("`{member}`")) {
            problems.push(format!("the line member `{member}`"));
        }
    }
    for version in declared(model, VERSION)?["variants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !page.contains(&format!("`{version}`")) {
            problems.push(format!("the schema version `{version}`"));
        }
    }
    if !page.contains(&format!("\"schema_version\":{schema_version}")) {
        problems.push(format!(
            "a recorded line of schema version {schema_version}"
        ));
    }
    let variants = union["variants"]
        .as_object()
        .with_context(|| format!("compiled model: {UNION} has no variants"))?;
    for (kind, payload) in variants {
        let Some(text) = section(page, kind) else {
            problems.push(format!("the record kind `{kind}` (a `### {kind}` section)"));
            continue;
        };
        let record = payload["name"]
            .as_str()
            .with_context(|| format!("compiled model: {UNION} {kind} carries no record"))?;
        for field in declared(model, record)?["fields"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let Some(name) = field["name"].as_str() else {
                continue;
            };
            if !text.contains(&format!("`{name}`")) {
                problems.push(format!("the field `{name}` of `{kind}`"));
            }
            let Some(nested) = record_of(model, &field["type_ref"]) else {
                continue;
            };
            for member in field_names(declared(model, nested)?) {
                if !text.contains(&format!("`{member}`")) {
                    problems.push(format!("the field `{member}` of `{kind}` (in `{name}`)"));
                }
            }
        }
    }
    Ok(problems)
}

/// The record a field of type `type_ref` holds, alone, optional or in a list.
fn record_of<'a>(model: &Value, type_ref: &'a Value) -> Option<&'a str> {
    match type_ref["kind"].as_str()? {
        "optional" | "list" => record_of(model, &type_ref["of"]),
        "declared" => {
            let name = type_ref["name"].as_str()?;
            (declared(model, name).ok()?["kind"] == "struct").then_some(name)
        }
        _ => None,
    }
}

/// The committed V1 contract, relative to the repository root.
pub const CONTRACT: &str = "docs/contracts/run-events-v1.json";

/// The stop reasons the terminal record names, each of which exits with a status.
const STOP_REASON: &str = "intake.routing.StopReason";

/// What a run that failed exits with, in the contract's exit statuses.
const FAILED_RUN: &str = "failed";

/// What a run with `--state` that reached its human gate holding no Suspension exits with.
const NOTHING_SUSPENDED: &str = "ApprovalRequired, nothing suspended";

fn variants(model: &Value, name: &str) -> Result<Vec<String>> {
    let mut variants: Vec<String> = declared(model, name)?["variants"]
        .as_array()
        .with_context(|| format!("compiled model: {name} is not an enum"))?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    variants.sort();
    Ok(variants)
}

/// A field's type as the contract states it: its type, whether it may be absent, the values of an
/// enum it names and, for a record it holds (alone, optional or in a list), the record's own
/// fields, each stated the same way. A record inside such a record is refused: the contract states
/// one level of records.
fn field_type(model: &Value, type_ref: &Value) -> Result<Value> {
    stated_type(model, type_ref, true)
}

/// What a field's type names besides primitives and lists: at most one enum or record.
#[derive(Default)]
struct Named {
    values: Option<Vec<String>>,
    record: Option<String>,
}

fn stated_type(model: &Value, type_ref: &Value, records: bool) -> Result<Value> {
    fn name(model: &Value, type_ref: &Value, named: &mut Named, records: bool) -> Result<String> {
        match type_ref["kind"].as_str() {
            Some("primitive") => {
                let primitive = type_ref["name"]
                    .as_str()
                    .context("compiled model: a primitive type has no name")?;
                let mut chars = primitive.chars();
                Ok(chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default())
            }
            Some("list") => Ok(format!(
                "List<{}>",
                name(model, &type_ref["of"], named, records)?
            )),
            Some("declared") => {
                let declared_name = type_ref["name"]
                    .as_str()
                    .context("compiled model: a declared type has no name")?;
                let kind = &declared(model, declared_name)?["kind"];
                ensure!(
                    named.values.is_none()
                        && named.record.is_none()
                        && (kind == "enum" || (kind == "struct" && records)),
                    "the run events contract states fields of primitive, list, enum and record \
                     types, and a record's own fields of primitive, list and enum types, \
                     not {declared_name}"
                );
                if kind == "enum" {
                    named.values = Some(variants(model, declared_name)?);
                } else {
                    named.record = Some(declared_name.to_owned());
                }
                Ok(declared_name.to_owned())
            }
            other => bail!("the run events contract cannot state a field of kind {other:?}"),
        }
    }
    let (optional, inner) = match type_ref["kind"].as_str() {
        Some("optional") => (true, &type_ref["of"]),
        _ => (false, type_ref),
    };
    let mut named = Named::default();
    let mut stated = Map::new();
    stated.insert(
        "type".into(),
        Value::String(name(model, inner, &mut named, records)?),
    );
    stated.insert("optional".into(), Value::Bool(optional));
    if let Some(values) = named.values {
        stated.insert("values".into(), json!(values));
    }
    if let Some(record) = named.record {
        let mut fields = BTreeMap::new();
        for field in declared(model, &record)?["fields"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let name = field["name"]
                .as_str()
                .with_context(|| format!("compiled model: a field of {record} has no name"))?;
            fields.insert(
                name.to_owned(),
                stated_type(model, &field["type_ref"], false)?,
            );
        }
        stated.insert("fields".into(), json!(fields));
    }
    Ok(Value::Object(stated))
}

/// The exit statuses a terminal record carries, each with what it means: the stop reasons the
/// help names in parentheses on its line, [`FAILED_RUN`] for `failed` and [`NOTHING_SUSPENDED`]
/// for `nothing_suspended`. Every stop reason is named on exactly one line.
fn exit_statuses(
    stop_reasons: &[String],
    exit_help: &str,
    failed: u8,
    nothing_suspended: u8,
) -> Result<BTreeMap<String, Vec<String>>> {
    let lines: Vec<(u8, &str)> = exit_help
        .split_once("Exit status:")
        .context("the run help states no exit status")?
        .1
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (status, text) = line.split_once(char::is_whitespace)?;
            Some((status.parse().ok()?, text))
        })
        .collect();
    let mut statuses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for reason in stop_reasons {
        let named: Vec<u8> = lines
            .iter()
            .filter(|(_, text)| {
                text.split('(')
                    .skip(1)
                    .filter_map(|group| group.split_once(')'))
                    .any(|(group, _)| group.split(',').any(|name| name.trim() == reason))
            })
            .map(|(status, _)| *status)
            .collect();
        let [status] = named[..] else {
            bail!(
                "the run help names the stop reason {reason} on {} exit status lines, not one",
                named.len()
            );
        };
        statuses
            .entry(status.to_string())
            .or_default()
            .push(reason.clone());
    }
    for (status, meaning) in [(failed, FAILED_RUN), (nothing_suspended, NOTHING_SUSPENDED)] {
        ensure!(
            lines.iter().any(|(stated, _)| *stated == status),
            "the run help states no exit status {status} ({meaning})"
        );
        statuses
            .entry(status.to_string())
            .or_default()
            .push(meaning.to_owned());
    }
    Ok(statuses)
}

/// The contract `model` declares, with the exit statuses `exit_help` states: `failed` is the
/// status of a run that failed, `nothing_suspended` the status of a run with a state directory
/// that reached its human gate holding no Suspension, `schema_version` the number every line
/// carries.
pub fn contract(
    model: &Value,
    exit_help: &str,
    failed: u8,
    nothing_suspended: u8,
    schema_version: u64,
) -> Result<Value> {
    let union = declared(model, UNION)?;
    let tag = union["tag"]
        .as_str()
        .with_context(|| format!("compiled model: {UNION} has no tag"))?;
    let members: Vec<&str> = field_names(declared(model, LINE)?)
        .into_iter()
        .filter(|name| *name != "event")
        .collect();
    let mut kinds = BTreeMap::new();
    for (kind, payload) in union["variants"]
        .as_object()
        .with_context(|| format!("compiled model: {UNION} has no variants"))?
    {
        let record = payload["name"]
            .as_str()
            .with_context(|| format!("compiled model: {UNION} {kind} carries no record"))?;
        let mut fields = BTreeMap::new();
        for field in declared(model, record)?["fields"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let name = field["name"]
                .as_str()
                .with_context(|| format!("compiled model: a field of {record} has no name"))?;
            fields.insert(name.to_owned(), field_type(model, &field["type_ref"])?);
        }
        kinds.insert(kind.clone(), json!({ "fields": fields }));
    }
    Ok(json!({
        "contract": "b10x-loom run --output jsonl",
        "domain": "intake.events",
        "generated_by": "loom-docs generate, from ess/intake; task docs-check compares it with the declaration",
        "schema_version": schema_version,
        "schema_versions": variants(model, VERSION)?,
        "line": {"members": members, "tag": tag},
        "kinds": kinds,
        "exit_statuses": exit_statuses(&variants(model, STOP_REASON)?, exit_help, failed, nothing_suspended)?,
    }))
}

/// A field as the contract states it, in one line: `Integer, optional`, `StopReason [A, B]`,
/// `List<Model> {name: String; cost: Decimal, optional}`.
fn describe(field: &Value) -> String {
    let mut text = field["type"].as_str().unwrap_or("?").to_owned();
    if field["optional"] == true {
        text.push_str(", optional");
    }
    if let Some(values) = field["values"].as_array() {
        let values: Vec<&str> = values.iter().filter_map(Value::as_str).collect();
        text.push_str(&format!(" [{}]", values.join(", ")));
    }
    if let Some(fields) = field["fields"].as_object() {
        let fields: Vec<String> = fields
            .iter()
            .map(|(name, field)| format!("{name}: {}", describe(field)))
            .collect();
        text.push_str(&format!(" {{{}}}", fields.join("; ")));
    }
    text
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// The keys of `committed` and `declared`, in order, each with its value on either side.
fn paired(committed: &Value, declared: &Value) -> BTreeMap<String, (Value, Value)> {
    let (committed, declared) = (object(committed), object(declared));
    committed
        .keys()
        .chain(declared.keys())
        .map(|key| {
            let side = |map: &Map<String, Value>| map.get(key).cloned().unwrap_or(Value::Null);
            (key.clone(), (side(&committed), side(&declared)))
        })
        .collect()
}

/// One way a declared contract differs from the committed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// What differs: a record kind, a field of one, an exit status or a member of the contract.
    pub subject: String,
    /// How it differs.
    pub change: String,
    /// Whether a reader of the committed contract breaks on it.
    pub breaking: bool,
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let verdict = if self.breaking {
            "a break"
        } else {
            "not a break"
        };
        write!(f, "{}: {} ({verdict})", self.subject, self.change)
    }
}

/// How the field `subject` names differs between `old` and `new`, either absent as null; `None`
/// when it does not.
fn field_difference(subject: String, old: &Value, new: &Value) -> Option<Difference> {
    if old == new {
        return None;
    }
    Some(match (old.is_null(), new.is_null()) {
        (false, true) => Difference {
            subject,
            change: format!("removed or renamed (was {})", describe(old)),
            breaking: true,
        },
        (true, false) => Difference {
            subject,
            change: format!("added ({})", describe(new)),
            breaking: new["optional"] != true,
        },
        _ => Difference {
            subject,
            change: format!("retyped from {} to {}", describe(old), describe(new)),
            breaking: true,
        },
    })
}

/// Whether `old` and `new` state the same record field but for the record's own fields.
fn record_changed_alone(old: &Value, new: &Value) -> bool {
    let rest = |field: &Value| {
        let mut field = object(field);
        field.remove("fields");
        field
    };
    old["fields"].is_object() && new["fields"].is_object() && rest(old) == rest(new)
}

/// Every difference between the `committed` contract and the `declared` one.
///
/// Within one schema version only a new optional field of an existing record kind, or of a record
/// one of its fields holds, is not a break: a reader by name ignores it. A record kind, field or
/// exit status removed, renamed, retyped or added, an enum value added or removed, and any other
/// change of the contract are.
pub fn differences(committed: &Value, declared: &Value) -> Vec<Difference> {
    let mut found = Vec::new();
    let breaking = |subject: String, change: String| Difference {
        subject,
        change,
        breaking: true,
    };
    for (key, (old, new)) in paired(committed, declared) {
        if old == new {
            continue;
        }
        match key.as_str() {
            "kinds" => {
                for (kind, (old, new)) in paired(&old, &new) {
                    let subject = format!("record kind `{kind}`");
                    match (old.is_null(), new.is_null()) {
                        (false, true) => found.push(breaking(subject, "removed or renamed".into())),
                        (true, false) => found.push(breaking(subject, "added".into())),
                        _ => {
                            for (field, (old, new)) in paired(&old["fields"], &new["fields"]) {
                                if record_changed_alone(&old, &new) {
                                    for (member, (old, new)) in
                                        paired(&old["fields"], &new["fields"])
                                    {
                                        found.extend(field_difference(
                                            format!("field `{member}` of `{kind}` (in `{field}`)"),
                                            &old,
                                            &new,
                                        ));
                                    }
                                } else {
                                    found.extend(field_difference(
                                        format!("field `{field}` of `{kind}`"),
                                        &old,
                                        &new,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            "exit_statuses" => {
                for (status, (old, new)) in paired(&old, &new) {
                    if old != new {
                        found.push(breaking(
                            format!("exit status {status}"),
                            format!("meant {old}, now means {new}"),
                        ));
                    }
                }
            }
            _ => found.push(breaking(
                format!("`{key}`"),
                format!("was {old}, now {new}"),
            )),
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const STOP: &str = "intake.routing.StopReason";

    const HELP: &str = "Exit status:\n  0  completed (Completed)\n  4  with --state, nothing suspended: `stopped: ApprovalRequired (<action>), nothing suspended`\n  3  stopped (Refused)\n  1  the run failed\n  2  the command line is not valid";

    fn prim(name: &str) -> Value {
        json!({"kind": "primitive", "name": name})
    }

    fn optional(of: Value) -> Value {
        json!({"kind": "optional", "of": of})
    }

    /// A compiled model declaring `kinds`, each a record kind with its fields and their types.
    fn typed(kinds: &[(&str, Vec<(&str, Value)>)], stop_reasons: &[&str]) -> Value {
        let mut types = serde_json::Map::new();
        let mut variants = serde_json::Map::new();
        for (kind, fields) in kinds {
            let record = format!("intake.events.{kind}Event");
            variants.insert(
                (*kind).to_owned(),
                json!({"kind": "declared", "name": record}),
            );
            let fields: Vec<Value> = fields
                .iter()
                .map(|(name, type_ref)| json!({"name": name, "type_ref": type_ref}))
                .collect();
            types.insert(
                record,
                json!({"body": {"kind": "struct", "fields": fields}}),
            );
        }
        types.insert(
            LINE.to_owned(),
            json!({"body": {"kind": "struct", "fields": [
                {"name": "schema_version", "type_ref": {"kind": "declared", "name": VERSION}},
                {"name": "event", "type_ref": {"kind": "declared", "name": UNION}}
            ]}}),
        );
        types.insert(
            UNION.to_owned(),
            json!({"body": {"kind": "union", "tag": "kind", "variants": variants}}),
        );
        types.insert(
            VERSION.to_owned(),
            json!({"body": {"kind": "enum", "variants": ["V1"]}}),
        );
        types.insert(
            STOP.to_owned(),
            json!({"body": {"kind": "enum", "variants": stop_reasons}}),
        );
        json!({ "types": types })
    }

    fn route() -> (&'static str, Vec<(&'static str, Value)>) {
        (
            "Route",
            vec![("protocol", prim("string")), ("accepted", prim("boolean"))],
        )
    }

    fn terminal() -> (&'static str, Vec<(&'static str, Value)>) {
        (
            "Terminal",
            vec![
                (
                    "stop_reason",
                    optional(json!({"kind": "declared", "name": STOP})),
                ),
                ("exit_status", prim("integer")),
            ],
        )
    }

    fn contract_of(
        kinds: &[(&str, Vec<(&str, Value)>)],
        stop_reasons: &[&str],
        help: &str,
    ) -> Value {
        contract(&typed(kinds, stop_reasons), help, 1, 4, 1).unwrap()
    }

    fn base() -> Value {
        contract_of(&[route(), terminal()], &["Completed", "Refused"], HELP)
    }

    fn subjects(found: &[Difference]) -> Vec<(&str, bool)> {
        found
            .iter()
            .map(|difference| (difference.subject.as_str(), difference.breaking))
            .collect()
    }

    /// The committed snapshot is the contract `ess/intake` declares today, with the exit statuses
    /// `b10x-loom run --help` states and the binary returns.
    #[test]
    fn the_v1_contract_matches_the_declared_events() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let model = crate::compile(std::path::Path::new("ess"), &root.join(SYSTEM)).unwrap();
        let declared = contract(
            &model,
            b10x_loom_cli::EXIT_STATUS,
            b10x_loom_cli::events::FAILED,
            b10x_loom_cli::NOTHING_SUSPENDED_AT_GATE,
            b10x_loom_cli::events::SCHEMA_VERSION,
        )
        .unwrap();
        let text = std::fs::read_to_string(root.join(CONTRACT))
            .unwrap_or_else(|error| panic!("{CONTRACT}: {error}"));
        let committed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(differences(&committed, &declared), []);
        assert_eq!(committed, declared);
        for kind in [
            "Route", "Turn", "ToolCall", "Usage", "Approval", "Session", "Terminal",
        ] {
            assert!(committed["kinds"][kind].is_object(), "{kind}");
        }
        assert_eq!(
            committed["kinds"]["Session"]["fields"]["tool_calls"]["fields"]["answered"],
            json!({"type": "Boolean", "optional": false})
        );
        assert_eq!(
            committed["kinds"]["Usage"]["fields"]["input_tokens"],
            json!({"type": "Integer", "optional": true})
        );
        assert_eq!(committed["schema_versions"], json!(["V1"]));
        assert_eq!(committed["exit_statuses"]["1"], json!(["failed"]));
        assert_eq!(
            committed["exit_statuses"]["0"],
            json!(["ApprovalRequired", "Completed"])
        );
        assert_eq!(
            text,
            serde_json::to_string_pretty(&declared).unwrap() + "\n"
        );
    }

    #[test]
    fn a_removed_renamed_or_retyped_field_is_a_break() {
        let removed = contract_of(
            &[("Route", vec![("protocol", prim("string"))]), terminal()],
            &["Completed", "Refused"],
            HELP,
        );
        assert_eq!(
            subjects(&differences(&base(), &removed)),
            [("field `accepted` of `Route`", true)]
        );

        let renamed = contract_of(
            &[
                (
                    "Route",
                    vec![("pick", prim("string")), ("accepted", prim("boolean"))],
                ),
                terminal(),
            ],
            &["Completed", "Refused"],
            HELP,
        );
        assert_eq!(
            subjects(&differences(&base(), &renamed)),
            [
                ("field `pick` of `Route`", true),
                ("field `protocol` of `Route`", true)
            ]
        );

        let retyped = contract_of(
            &[
                (
                    "Route",
                    vec![("protocol", prim("string")), ("accepted", prim("string"))],
                ),
                terminal(),
            ],
            &["Completed", "Refused"],
            HELP,
        );
        let found = differences(&base(), &retyped);
        assert_eq!(subjects(&found), [("field `accepted` of `Route`", true)]);
        assert!(
            found[0].change.contains("Boolean") && found[0].change.contains("String"),
            "{}",
            found[0].change
        );

        let now_optional = contract_of(
            &[
                (
                    "Route",
                    vec![
                        ("protocol", optional(prim("string"))),
                        ("accepted", prim("boolean")),
                    ],
                ),
                terminal(),
            ],
            &["Completed", "Refused"],
            HELP,
        );
        assert_eq!(
            subjects(&differences(&base(), &now_optional)),
            [("field `protocol` of `Route`", true)]
        );

        let new_value = contract_of(
            &[route(), terminal()],
            &["Completed", "Refused", "StepBudget"],
            &HELP.replace("(Refused)", "(Refused, StepBudget)"),
        );
        let found = differences(&base(), &new_value);
        assert_eq!(
            subjects(&found),
            [
                ("exit status 3", true),
                ("field `stop_reason` of `Terminal`", true)
            ]
        );
    }

    #[test]
    fn a_new_record_kind_is_a_break() {
        let added = contract_of(
            &[
                route(),
                terminal(),
                ("Usage", vec![("turn", prim("integer"))]),
            ],
            &["Completed", "Refused"],
            HELP,
        );
        let found = differences(&base(), &added);
        assert_eq!(subjects(&found), [("record kind `Usage`", true)]);
        let removed = contract_of(&[terminal()], &["Completed", "Refused"], HELP);
        assert_eq!(
            subjects(&differences(&base(), &removed)),
            [("record kind `Route`", true)]
        );
    }

    #[test]
    fn a_new_optional_field_is_not_a_break() {
        let with = |type_ref: Value| {
            contract_of(
                &[
                    (
                        "Route",
                        vec![
                            ("protocol", prim("string")),
                            ("accepted", prim("boolean")),
                            ("reason", type_ref),
                        ],
                    ),
                    terminal(),
                ],
                &["Completed", "Refused"],
                HELP,
            )
        };
        let found = differences(&base(), &with(optional(prim("string"))));
        assert_eq!(subjects(&found), [("field `reason` of `Route`", false)]);
        assert!(found[0].to_string().contains("not a break"), "{}", found[0]);
        let found = differences(&base(), &with(prim("string")));
        assert_eq!(subjects(&found), [("field `reason` of `Route`", true)]);
        assert!(differences(&base(), &base()).is_empty());
    }

    #[test]
    fn an_exit_status_that_changes_meaning_is_a_break() {
        let moved = contract_of(
            &[route(), terminal()],
            &["Completed", "Refused"],
            &HELP
                .replace("completed (Completed)", "completed (Completed, Refused)")
                .replace("stopped (Refused)", "stopped"),
        );
        assert_eq!(
            subjects(&differences(&base(), &moved)),
            [("exit status 0", true), ("exit status 3", true)]
        );
        let unstated = typed(
            &[route(), terminal()],
            &["Completed", "Refused", "StepBudget"],
        );
        assert!(contract(&unstated, HELP, 1, 4, 1).is_err());
    }

    fn model() -> Value {
        let fields = |names: &[&str]| json!({"fields": names.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()});
        json!({"types": {
            LINE: {"body": fields(&["schema_version", "event"])},
            UNION: {"body": {"kind": "union", "tag": "kind", "variants": {
                "Route": {"kind": "declared", "name": "intake.events.RouteEvent"},
                "Terminal": {"kind": "declared", "name": "intake.events.TerminalEvent"}
            }}},
            VERSION: {"body": {"kind": "enum", "variants": ["V1"]}},
            "intake.events.RouteEvent": {"body": fields(&["protocol"])},
            "intake.events.TerminalEvent": {"body": fields(&["exit_status"])}
        }})
    }

    const PAGE_TEXT: &str = "# Run events\n\n`schema_version` `kind` `V1`\n\n## Record kinds\n\n### Route\n\n`protocol`\n\n### Terminal\n\n`exit_status`\n\n## Run\n\n{\"schema_version\":1}\n";

    #[test]
    fn a_page_naming_every_kind_field_and_version_passes() {
        assert!(problems(&model(), PAGE_TEXT, 1).unwrap().is_empty());
    }

    #[test]
    fn a_missing_kind_field_member_or_version_is_named() {
        let page = PAGE_TEXT
            .replace("### Terminal", "### Final")
            .replace("`protocol`", "protocol")
            .replace("`kind`", "kind");
        let found = problems(&model(), &page, 2).unwrap();
        assert_eq!(
            found,
            [
                "the line member `kind`",
                "a recorded line of schema version 2",
                "the field `protocol` of `Route`",
                "the record kind `Terminal` (a `### Terminal` section)",
            ]
        );
    }

    /// `base()`'s kinds and a `Session` kind whose `models` field is an optional list of the
    /// record `SessionModel`, declaring `members`.
    fn with_records(members: Vec<(&str, Value)>) -> Value {
        let record = "intake.events.SessionModel";
        let list = json!({"kind": "list", "of": {"kind": "declared", "name": record}});
        let mut model = typed(
            &[
                route(),
                terminal(),
                ("Session", vec![("models", optional(list))]),
            ],
            &["Completed", "Refused"],
        );
        let fields: Vec<Value> = members
            .into_iter()
            .map(|(name, type_ref)| json!({"name": name, "type_ref": type_ref}))
            .collect();
        model["types"][record] = json!({"body": {"kind": "struct", "fields": fields}});
        model
    }

    /// A field holding a record states the record's own fields, each judged as a field of a
    /// record kind is: a new optional one is no break, any other change is.
    #[test]
    fn a_record_field_states_its_fields_each_judged_as_a_kinds_field() {
        let members = || {
            vec![
                ("name", prim("string")),
                ("cost_usd", optional(prim("decimal"))),
            ]
        };
        let stated = contract(&with_records(members()), HELP, 1, 4, 1).unwrap();
        assert_eq!(
            stated["kinds"]["Session"]["fields"]["models"],
            json!({
                "type": "List<intake.events.SessionModel>",
                "optional": true,
                "fields": {
                    "cost_usd": {"type": "Decimal", "optional": true},
                    "name": {"type": "String", "optional": false}
                }
            })
        );
        let changed = |change: &dyn Fn(&mut Vec<(&str, Value)>)| {
            let mut members = members();
            change(&mut members);
            differences(
                &stated,
                &contract(&with_records(members), HELP, 1, 4, 1).unwrap(),
            )
        };
        let found = changed(&|members| members.push(("turns", optional(prim("integer")))));
        assert_eq!(
            subjects(&found),
            [("field `turns` of `Session` (in `models`)", false)]
        );
        let found = changed(&|members| members.push(("turns", prim("integer"))));
        assert_eq!(
            subjects(&found),
            [("field `turns` of `Session` (in `models`)", true)]
        );
        let found = changed(&|members| members[0].1 = prim("integer"));
        assert_eq!(
            subjects(&found),
            [("field `name` of `Session` (in `models`)", true)]
        );
        let found = changed(&|members| {
            members.remove(1);
        });
        assert_eq!(
            subjects(&found),
            [("field `cost_usd` of `Session` (in `models`)", true)]
        );

        let looped = with_records(vec![(
            "again",
            json!({"kind": "declared", "name": "intake.events.SessionModel"}),
        )]);
        assert!(contract(&looped, HELP, 1, 4, 1).is_err());
    }

    /// The page names every field of a record a kind's field holds, in the kind's section.
    #[test]
    fn a_field_of_a_nested_record_the_page_leaves_out_is_named() {
        let model = with_records(vec![
            ("name", prim("string")),
            ("cost_usd", optional(prim("decimal"))),
        ]);
        let page = "# Run events\n\n`schema_version` `kind` `V1`\n\n### Route\n\n`protocol` `accepted`\n\n### Terminal\n\n`stop_reason` `exit_status`\n\n### Session\n\n`models`: `name`\n\n## Run\n\n{\"schema_version\":1}\n";
        assert_eq!(
            problems(&model, page, 1).unwrap(),
            ["the field `cost_usd` of `Session` (in `models`)"]
        );
        let page = page.replace("`name`", "`name` `cost_usd`");
        assert!(problems(&model, &page, 1).unwrap().is_empty());
    }

    #[test]
    fn a_field_named_only_in_another_section_does_not_count() {
        let page = PAGE_TEXT.replace("### Terminal\n\n`exit_status`", "### Terminal\n\nnone");
        let page = page.replace("`protocol`", "`protocol` `exit_status`");
        assert_eq!(
            problems(&model(), &page, 1).unwrap(),
            ["the field `exit_status` of `Terminal`"]
        );
    }
}
