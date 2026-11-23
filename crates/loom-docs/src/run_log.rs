//! The contract of the run log a case keeps (`runlog-<stem>.jsonl`, `b10x-loom-runlog`), written
//! from the `loom.runlog` declaration it states.
//!
//! The contract is generated: `task docs-generate` writes it and `task docs-check` fails when the
//! committed file is not exactly what the declaration gives, naming the file.
use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use crate::run_events::{declared, field_names, field_type, variants};

/// The committed contract, relative to the repository root.
pub const CONTRACT: &str = "docs/contracts/run-log-v1.json";

/// The ESS system that declares the log.
pub const SYSTEM: &str = "ess";

const LINE: &str = "loom.runlog.RunLogLine";
const UNION: &str = "loom.runlog.RunLogEvent";
const REFUSAL: &str = "loom.runlog.RunLogRefusal";

/// The contract `model` declares: the line's own members, the tag that names an event's kind,
/// each kind with its fields (which sit beside the line's members, so none may share a name with
/// one), and the refusals a reader gives.
pub fn contract(model: &Value) -> Result<Value> {
    let union = declared(model, UNION)?;
    let tag = union["tag"]
        .as_str()
        .with_context(|| format!("compiled model: {UNION} has no tag"))?;
    let mut members = BTreeMap::new();
    for field in declared(model, LINE)?["fields"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = field["name"]
            .as_str()
            .with_context(|| format!("compiled model: a field of {LINE} has no name"))?;
        if name != "event" {
            members.insert(name.to_owned(), field_type(model, &field["type_ref"])?);
        }
    }
    let mut kinds = BTreeMap::new();
    for (kind, payload) in union["variants"]
        .as_object()
        .with_context(|| format!("compiled model: {UNION} has no variants"))?
    {
        let record = payload["name"]
            .as_str()
            .with_context(|| format!("compiled model: {UNION} {kind} carries no record"))?;
        let body = declared(model, record)?;
        for name in field_names(body) {
            ensure!(
                name != tag && !members.contains_key(name),
                "the field `{name}` of `{kind}` shares its name with a member of the line"
            );
        }
        let mut fields = BTreeMap::new();
        for field in body["fields"].as_array().into_iter().flatten() {
            let name = field["name"]
                .as_str()
                .with_context(|| format!("compiled model: a field of {record} has no name"))?;
            fields.insert(name.to_owned(), field_type(model, &field["type_ref"])?);
        }
        kinds.insert(kind.clone(), json!({ "fields": fields }));
    }
    Ok(json!({
        "contract": "runlog-<stem>.jsonl, the run log of one case, beside its case file <stem>.json",
        "domain": "loom.runlog",
        "generated_by": "loom-docs generate, from ess; task docs-check holds the committed file to the declaration",
        "line": {
            "members": members,
            "tag": tag,
            "encoding": "one JSON object per line, ended by a line feed; the event's tag and fields sit beside the line's members",
            "seq": "the line's position, counted from 0, with no gap",
            "prev": "the SHA-256 of the previous line's bytes without its line feed, as 64 lower-case hex digits; absent on line 0, present on every other line",
        },
        "kinds": kinds,
        "refusals": variants(model, REFUSAL)?,
    }))
}

/// The contract as the committed file holds it.
pub fn text(model: &Value) -> Result<String> {
    Ok(serde_json::to_string_pretty(&contract(model)?)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled() -> Value {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        crate::compile(std::path::Path::new("ess"), &root.join(SYSTEM)).unwrap()
    }

    /// The committed contract is the one `loom.runlog` declares today, byte for byte.
    #[test]
    fn the_committed_contract_is_the_declared_one() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let model = compiled();
        let committed = std::fs::read_to_string(root.join(CONTRACT))
            .unwrap_or_else(|error| panic!("{CONTRACT}: {error}"));
        assert_eq!(committed, text(&model).unwrap());
        let contract: Value = serde_json::from_str(&committed).unwrap();
        assert_eq!(contract["line"]["tag"], json!("kind"));
        assert_eq!(
            contract["line"]["members"]["prev"],
            json!({"type": "String", "optional": true})
        );
        assert_eq!(
            contract["refusals"],
            json!(["BrokenChain", "CutLine", "SequenceGap"])
        );
        for kind in [
            "CaseOpened",
            "Evaluated",
            "ActionStarted",
            "ActionFinished",
            "Discarded",
            "HandoffChecked",
            "ChildOpened",
            "ChildClosed",
            "AuthorityRecorded",
            "Stopped",
            "Terminated",
            "StopRecorded",
            "StopLifted",
        ] {
            assert!(contract["kinds"][kind]["fields"].is_object(), "{kind}");
        }
        assert_eq!(
            contract["kinds"]["ActionFinished"]["fields"]["revisions"]["fields"]["from"],
            json!({"type": "String", "optional": true})
        );
    }

    /// A kind's field named as a member of the line would be ambiguous on the wire: refused.
    #[test]
    fn a_field_sharing_a_name_with_a_line_member_is_refused() {
        let mut model = compiled();
        let fields = model["types"]["loom.runlog.Terminated"]["body"]["fields"]
            .as_array_mut()
            .unwrap();
        let mut renamed = fields[0].clone();
        renamed["name"] = json!("seq");
        fields.push(renamed);
        let error = contract(&model).unwrap_err().to_string();
        assert!(error.contains("`seq` of `Terminated`"), "{error}");
    }
}
