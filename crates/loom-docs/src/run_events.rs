//! Holds the hand-written run events page to the `intake.events` declaration it documents.
//!
//! The page carries pasted output, so it cannot be generated; it is checked instead. It must name
//! the stream's tag and schema version member, every schema version, every record kind as its own
//! `### <Kind>` section, and in that section every field the kind's record declares. A recorded
//! line on the page must carry the version the writer writes.
use anyhow::{Context, Result};
use serde_json::Value;

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
        for field in field_names(declared(model, record)?) {
            if !text.contains(&format!("`{field}`")) {
                problems.push(format!("the field `{field}` of `{kind}`"));
            }
        }
    }
    Ok(problems)
}

/// The committed V1 contract, relative to the repository root.
pub const CONTRACT: &str = "docs/contracts/run-events-v1.json";

/// The contract `model` declares, with the exit statuses `exit_help` states.
pub fn contract(
    _model: &Value,
    _exit_help: &str,
    _failed: u8,
    _nothing_suspended: u8,
    _schema_version: u64,
) -> Result<Value> {
    Ok(Value::Null)
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

/// Every difference between the `committed` contract and the `declared` one.
pub fn differences(_committed: &Value, _declared: &Value) -> Vec<Difference> {
    Vec::new()
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
        for kind in ["Route", "Turn", "ToolCall", "Usage", "Approval", "Terminal"] {
            assert!(committed["kinds"][kind].is_object(), "{kind}");
        }
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
