//! The JSON documents the tree commands print, member for member as `loom.tree` declares them;
//! an absent optional member is left out, and an enum is its variant's name.

use loom::tree::{CaseStatus, LogFailure, TreeProtocol, TreeRefusal, TreeStatus, TreeVerify};
use serde_json::{Map, Value, json};

fn put(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value);
    }
}

/// `refusal` as one JSON object.
pub fn refusal_json(refusal: &TreeRefusal) -> Value {
    let mut object = Map::new();
    object.insert("reason".into(), json!(format!("{:?}", refusal.reason)));
    object.insert("detail".into(), json!(refusal.detail));
    put(&mut object, "case", refusal.case.clone().map(Value::from));
    put(&mut object, "file", refusal.file.clone().map(Value::from));
    put(
        &mut object,
        "action",
        refusal.action.clone().map(Value::from),
    );
    put(&mut object, "story", refusal.story.clone().map(Value::from));
    put(&mut object, "holder", refusal.holder.map(Value::from));
    Value::Object(object)
}

fn case_json(case: &CaseStatus) -> Value {
    let mut object = Map::new();
    object.insert("case".into(), json!(case.case));
    put(&mut object, "parent", case.parent.clone().map(Value::from));
    object.insert("protocol".into(), json!(case.protocol));
    object.insert("revision".into(), json!(case.revision));
    put(
        &mut object,
        "outcome",
        case.outcome.clone().map(Value::from),
    );
    put(&mut object, "stop", case.stop.clone().map(Value::from));
    object.insert("open_obligations".into(), json!(case.open_obligations));
    object.insert("case_file".into(), json!(case.case_file));
    object.insert("run_log".into(), json!(case.run_log));
    Value::Object(object)
}

/// One protocol of a tree as one JSON object.
pub(crate) fn protocol_json(protocol: &TreeProtocol) -> Value {
    json!({
        "registration": protocol.registration,
        "protocol": protocol.protocol,
        "revision": protocol.revision,
        "file": protocol.file,
        "sha256": protocol.sha256,
    })
}

/// `status` as one JSON object.
pub fn status_json(status: &TreeStatus) -> Value {
    json!({
        "store": status.store,
        "workspace": status.workspace,
        "root": status.root,
        "cases": status.cases.iter().map(case_json).collect::<Vec<_>>(),
        "protocols": status.protocols.iter().map(protocol_json).collect::<Vec<_>>(),
    })
}

fn failure_json(failure: &LogFailure) -> Value {
    json!({"case": failure.case, "line": failure.line, "detail": failure.detail})
}

/// `verified` as one JSON object.
pub fn verify_json(verified: &TreeVerify) -> Value {
    let mut object = Map::new();
    object.insert("cases".into(), json!(verified.cases));
    put(
        &mut object,
        "failure",
        verified.failure.as_ref().map(failure_json),
    );
    Value::Object(object)
}
