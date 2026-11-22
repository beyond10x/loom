//! `read_status`: a printed tree status read back into the value `tree_status` returned.

use loom::tree::{
    ApprovalDecision, CaseStatus, StatusApproval, StatusCopy, StatusEvidence, StatusRun,
    TreeProtocol, TreeRefusalReason, TreeStatus,
};
use serde_json::Value;

use crate::Refused;
use crate::encode::loom_json;
use crate::status::STATUS_FORMAT;

/// What is wrong with a document that names [`STATUS_FORMAT`] but does not decode.
struct Bad(String);

type Read<T> = Result<T, Bad>;

fn member<'a>(object: &'a Value, at: &str, name: &str) -> Read<&'a Value> {
    object
        .get(name)
        .ok_or_else(|| Bad(format!("{at}: the member {name} is missing")))
}

fn text(object: &Value, at: &str, name: &str) -> Read<String> {
    member(object, at, name)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Bad(format!("{at}.{name}: not a string")))
}

fn optional_text(object: &Value, at: &str, name: &str) -> Read<Option<String>> {
    match object.get(name) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_owned()))
            .ok_or_else(|| Bad(format!("{at}.{name}: not a string"))),
    }
}

fn integer(object: &Value, at: &str, name: &str) -> Read<i64> {
    member(object, at, name)?
        .as_i64()
        .ok_or_else(|| Bad(format!("{at}.{name}: not an integer")))
}

fn texts(object: &Value, at: &str, name: &str) -> Read<Vec<String>> {
    member(object, at, name)?
        .as_array()
        .ok_or_else(|| Bad(format!("{at}.{name}: not a list")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| Bad(format!("{at}.{name}: an entry is not a string")))
        })
        .collect()
}

fn list<T>(
    object: &Value,
    at: &str,
    name: &str,
    each: impl Fn(&Value, &str) -> Read<T>,
) -> Read<Vec<T>> {
    member(object, at, name)?
        .as_array()
        .ok_or_else(|| Bad(format!("{at}.{name}: not a list")))?
        .iter()
        .enumerate()
        .map(|(index, item)| each(item, &format!("{at}.{name}[{index}]")))
        .collect()
}

fn evidence(value: &Value, at: &str) -> Read<StatusEvidence> {
    Ok(StatusEvidence {
        index: integer(value, at, "index")?,
        evidence_id: text(value, at, "evidence_id")?,
        kind: text(value, at, "kind")?,
        result: optional_text(value, at, "result")?,
        subject: text(value, at, "subject")?,
        subject_revision: text(value, at, "subject_revision")?,
        upstream_revisions: loom_json(member(value, at, "upstream_revisions")?),
        producer: text(value, at, "producer")?,
        action: optional_text(value, at, "action")?,
        name: None,
    })
}

fn run(value: &Value, at: &str) -> Read<StatusRun> {
    Ok(StatusRun {
        action: text(value, at, "action")?,
        exit: match value.get("exit") {
            None => None,
            Some(exit) => Some(
                exit.as_i64()
                    .ok_or_else(|| Bad(format!("{at}.exit: not an integer")))?,
            ),
        },
        attempt: integer(value, at, "attempt")?,
        retried: member(value, at, "retried")?
            .as_bool()
            .ok_or_else(|| Bad(format!("{at}.retried: not a boolean")))?,
        retry_of: texts(value, at, "retry_of")?,
    })
}

fn approval(value: &Value, at: &str) -> Read<StatusApproval> {
    let answer = match text(value, at, "answer")?.as_str() {
        "Allow" => ApprovalDecision::Allow,
        "Deny" => ApprovalDecision::Deny,
        other => return Err(Bad(format!("{at}.answer: {other:?} is not Allow or Deny"))),
    };
    Ok(StatusApproval {
        capability: text(value, at, "capability")?,
        action: optional_text(value, at, "action")?,
        answer,
        actor: text(value, at, "actor")?,
        at: text(value, at, "at")?,
    })
}

fn copy(value: &Value, at: &str) -> Read<StatusCopy> {
    Ok(StatusCopy {
        source_case: text(value, at, "source_case")?,
        target_case: text(value, at, "target_case")?,
        record_kind: text(value, at, "record_kind")?,
    })
}

fn case(value: &Value, at: &str) -> Read<CaseStatus> {
    Ok(CaseStatus {
        case: text(value, at, "case")?,
        parent: optional_text(value, at, "parent")?,
        protocol: text(value, at, "protocol")?,
        revision: integer(value, at, "revision")?,
        outcome: optional_text(value, at, "outcome")?,
        stop: optional_text(value, at, "stop")?,
        open_obligations: texts(value, at, "open_obligations")?,
        case_file: text(value, at, "case_file")?,
        run_log: text(value, at, "run_log")?,
        name: optional_text(value, at, "name")?,
        protocol_id: text(value, at, "protocol_id")?,
        protocol_revision: text(value, at, "protocol_revision")?,
        stop_next: texts(value, at, "stop_next")?,
        evidence: list(value, at, "evidence", evidence)?,
        runs: list(value, at, "runs", run)?,
        approvals: list(value, at, "approvals", approval)?,
        copies: list(value, at, "copies", copy)?,
    })
}

fn protocol(value: &Value, at: &str) -> Read<TreeProtocol> {
    Ok(TreeProtocol {
        registration: text(value, at, "registration")?,
        protocol: text(value, at, "protocol")?,
        revision: integer(value, at, "revision")?,
        file: text(value, at, "file")?,
        sha256: text(value, at, "sha256")?,
    })
}

/// The status a `tree status` printed, from its bytes: the value [`crate::tree_status`] returned.
///
/// # Errors
/// `UnknownStatusFormat` naming the format found, or none, when the document's `format` is not
/// [`STATUS_FORMAT`]; `StoreUnreadable` when the bytes are not a JSON object or a member does not
/// decode. `version` is not checked.
pub fn read_status(bytes: &[u8]) -> Result<TreeStatus, Refused> {
    let unreadable = |detail: String| Refused::new(TreeRefusalReason::StoreUnreadable, detail);
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| unreadable(format!("the status is not JSON: {error}")))?;
    if !value.is_object() {
        return Err(unreadable("the status is not a JSON object".to_owned()));
    }
    match value.get("format") {
        Some(Value::String(format)) if format == STATUS_FORMAT => {}
        Some(found) => {
            return Err(Refused::new(
                TreeRefusalReason::UnknownStatusFormat,
                format!("the status names the format {found}, and {STATUS_FORMAT} is read"),
            ));
        }
        None => {
            return Err(Refused::new(
                TreeRefusalReason::UnknownStatusFormat,
                format!("the status names no format, and {STATUS_FORMAT} is read"),
            ));
        }
    }
    let at = "status";
    let read = || -> Read<TreeStatus> {
        Ok(TreeStatus {
            format: text(&value, at, "format")?,
            version: text(&value, at, "version")?,
            store: text(&value, at, "store")?,
            workspace: text(&value, at, "workspace")?,
            root: text(&value, at, "root")?,
            cases: list(&value, at, "cases", case)?,
            protocols: list(&value, at, "protocols", protocol)?,
        })
    };
    read().map_err(|Bad(detail)| unreadable(detail))
}
