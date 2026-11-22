//! The JSON documents the tree commands print, member for member as `loom.tree` declares them;
//! an absent optional member is left out, and an enum is its variant's name.

use loom::tree::{
    CaseStatus, LogFailure, NextDecision, StatusApproval, StatusCopy, StatusEvidence, StatusRun,
    TreeProtocol, TreeRefusal, TreeRun, TreeStatus, TreeVerify,
};
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
    put(&mut object, "name", case.name.clone().map(Value::from));
    object.insert("protocol_id".into(), json!(case.protocol_id));
    object.insert("protocol_revision".into(), json!(case.protocol_revision));
    object.insert("stop_next".into(), json!(case.stop_next));
    object.insert(
        "evidence".into(),
        Value::Array(case.evidence.iter().map(evidence_json).collect()),
    );
    object.insert(
        "runs".into(),
        Value::Array(case.runs.iter().map(run_entry_json).collect()),
    );
    object.insert(
        "approvals".into(),
        Value::Array(case.approvals.iter().map(approval_json).collect()),
    );
    object.insert(
        "copies".into(),
        Value::Array(case.copies.iter().map(copy_json).collect()),
    );
    Value::Object(object)
}

fn evidence_json(record: &StatusEvidence) -> Value {
    let mut object = Map::new();
    object.insert("index".into(), json!(record.index));
    object.insert("evidence_id".into(), json!(record.evidence_id));
    object.insert("kind".into(), json!(record.kind));
    put(
        &mut object,
        "result",
        record.result.clone().map(Value::from),
    );
    object.insert("subject".into(), json!(record.subject));
    object.insert("subject_revision".into(), json!(record.subject_revision));
    object.insert(
        "upstream_revisions".into(),
        crate::run::plain(&record.upstream_revisions),
    );
    object.insert("producer".into(), json!(record.producer));
    put(
        &mut object,
        "action",
        record.action.clone().map(Value::from),
    );
    put(&mut object, "name", record.name.clone().map(Value::from));
    Value::Object(object)
}

fn run_entry_json(run: &StatusRun) -> Value {
    let mut object = Map::new();
    object.insert("action".into(), json!(run.action));
    put(&mut object, "exit", run.exit.map(Value::from));
    object.insert("attempt".into(), json!(run.attempt));
    object.insert("retried".into(), json!(run.retried));
    object.insert("retry_of".into(), json!(run.retry_of));
    put(&mut object, "cancelled", run.cancelled.map(Value::from));
    Value::Object(object)
}

fn approval_json(approval: &StatusApproval) -> Value {
    let mut object = Map::new();
    object.insert("capability".into(), json!(approval.capability));
    put(
        &mut object,
        "action",
        approval.action.clone().map(Value::from),
    );
    object.insert("answer".into(), json!(format!("{:?}", approval.answer)));
    object.insert("actor".into(), json!(approval.actor));
    object.insert("at".into(), json!(approval.at));
    Value::Object(object)
}

fn copy_json(copy: &StatusCopy) -> Value {
    json!({
        "source_case": copy.source_case,
        "target_case": copy.target_case,
        "record_kind": copy.record_kind,
    })
}

/// `value` as a generated `loom` JSON value; a number keeps its spelling.
pub(crate) fn loom_json(value: &Value) -> loom::json::Value {
    use loom::json::Value as Loom;
    match value {
        Value::Null => Loom::Null,
        Value::Bool(value) => Loom::Bool(*value),
        Value::Number(number) => Loom::Number(number.to_string()),
        Value::String(text) => Loom::Text(text.clone()),
        Value::Array(items) => Loom::Array(items.iter().map(loom_json).collect()),
        Value::Object(members) => Loom::Object(
            members
                .iter()
                .map(|(name, member)| (name.clone(), loom_json(member)))
                .collect(),
        ),
    }
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
        "format": status.format,
        "version": status.version,
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

/// One decision a waiting tree names, as one JSON object.
pub fn decision_json(decision: &NextDecision) -> Value {
    let mut object = Map::new();
    object.insert("case".into(), json!(decision.case));
    object.insert("kind".into(), json!(format!("{:?}", decision.kind)));
    put(
        &mut object,
        "action",
        decision.action.clone().map(Value::from),
    );
    put(
        &mut object,
        "capability",
        decision.capability.clone().map(Value::from),
    );
    object.insert("decides".into(), json!(decision.decides));
    put(
        &mut object,
        "subject",
        decision.subject.clone().map(Value::from),
    );
    put(
        &mut object,
        "revision",
        decision.revision.clone().map(Value::from),
    );
    put(&mut object, "path", decision.path.clone().map(Value::from));
    put(
        &mut object,
        "outcome",
        decision.outcome.clone().map(Value::from),
    );
    object.insert("command".into(), json!(decision.command));
    Value::Object(object)
}

/// `run` as one JSON object.
pub fn run_json(run: &TreeRun) -> Value {
    let mut object = Map::new();
    object.insert("stop".into(), json!(format!("{:?}", run.stop)));
    object.insert("case".into(), json!(run.case));
    object.insert("steps".into(), json!(run.steps));
    object.insert(
        "decisions".into(),
        Value::Array(run.decisions.iter().map(decision_json).collect()),
    );
    put(&mut object, "detail", run.detail.clone().map(Value::from));
    Value::Object(object)
}
