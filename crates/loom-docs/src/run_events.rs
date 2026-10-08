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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
