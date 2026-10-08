//! The JSON of `b10x-loom evaluate`: the request it reads and the decision or refusal it writes,
//! each the generated `loom.evaluation` type (`ess/domains/evaluation.yaml`) on the wire its JSON
//! Schema projection gives: members by field name, an absent `Optional` left out, an enum by its
//! variant name, and no member the type does not declare.

use loom_governor::model::evaluation::{
    ActionStatus, CaseSnapshot, ClaimValue, EvaluationDecision, EvaluationInput, EvaluationRefusal,
    EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_governor::model::json::{self, Value};
use loom_governor::model::primitives::Timestamp;

/// The members an evaluation request declares.
const MEMBERS: [&str; 4] = ["protocol", "snapshot", "evidence", "at"];

/// The request in `text`, a `loom.evaluation.EvaluationRequest` document; refused naming the
/// member it is about: the protocol, the snapshot or the time when that member is missing or not
/// the type it declares, and the request as a whole otherwise.
pub fn request_from_json(text: &str) -> Result<EvaluationRequest, EvaluationRefusal> {
    let document = json::parse(text).map_err(|error| {
        refused(
            EvaluationInput::Request,
            format!("the request is not a JSON document: {error}"),
        )
    })?;
    let Value::Object(members) = &document else {
        return Err(refused(
            EvaluationInput::Request,
            format!("the request is {}, not an object", document.describes()),
        ));
    };
    for (index, (name, _)) in members.iter().enumerate() {
        if !MEMBERS.contains(&name.as_str()) {
            return Err(refused(
                EvaluationInput::Request,
                format!("the request declares no member `{name}`"),
            ));
        }
        if members[..index].iter().any(|(earlier, _)| earlier == name) {
            return Err(refused(
                EvaluationInput::Request,
                format!("the request names `{name}` twice"),
            ));
        }
    }
    let protocol = json::member_at(&document, "", "protocol")
        .and_then(|value| json::text_at(value, "protocol", "a protocol name, <name>@<major>"))
        .map_err(|error| refused(EvaluationInput::Protocol, error.to_string()))?;
    let snapshot = json::member_at(&document, "", "snapshot")
        .map_err(|error| refused(EvaluationInput::Snapshot, error.to_string()))?;
    let evidence = json::member_at(&document, "", "evidence")
        .and_then(|value| json::items_at(value, "evidence", "an array of canon-evidence/1 records"))
        .map_err(|error| refused(EvaluationInput::Request, error.to_string()))?;
    let at = match document.member("at") {
        None => None,
        Some(value) => Some(
            json::text_at(value, "at", "an RFC 3339 instant")
                .map_err(|error| refused(EvaluationInput::Time, error.to_string()))?,
        ),
    };
    Ok(EvaluationRequest {
        protocol: ProtocolName(protocol.to_owned()),
        snapshot: CaseSnapshot(snapshot.clone()),
        evidence: evidence.iter().cloned().map(EvidenceRecord).collect(),
        at: at.map(|at| Timestamp(at.to_owned())),
    })
}

/// `decision` as one `loom.evaluation.EvaluationDecision` JSON document, ending with a newline.
pub fn decision_json(decision: &EvaluationDecision) -> String {
    let mut out = String::from("{");
    json::member(&mut out, "protocol");
    json::push_text(&mut out, &decision.protocol.0);
    json::member(&mut out, "case");
    json::push_text(&mut out, &decision.case);
    json::member(&mut out, "actions");
    array(&mut out, &decision.actions, |out, action| {
        out.push('{');
        json::member(out, "action");
        json::push_text(out, &action.action);
        json::member(out, "status");
        json::push_text(
            out,
            match action.status {
                ActionStatus::Admissible => "Admissible",
                ActionStatus::ApprovalRequired => "ApprovalRequired",
                ActionStatus::Blocked => "Blocked",
            },
        );
        json::member(out, "requires");
        array(out, &action.requires, |out, capability| {
            json::push_text(out, capability);
        });
        json::member(out, "reasons");
        array(out, &action.reasons, json::push_value);
        out.push('}');
    });
    json::member(&mut out, "claims");
    array(&mut out, &decision.claims, |out, claim| {
        out.push('{');
        json::member(out, "claim");
        json::push_text(out, &claim.claim);
        json::member(out, "value");
        json::push_text(
            out,
            match claim.value {
                ClaimValue::True => "True",
                ClaimValue::False => "False",
                ClaimValue::Unknown => "Unknown",
            },
        );
        out.push('}');
    });
    json::member(&mut out, "obligations");
    array(&mut out, &decision.obligations, |out, obligation| {
        out.push('{');
        json::member(out, "obligation");
        json::push_text(out, &obligation.obligation);
        json::member(out, "open");
        json::push_bool(out, obligation.open);
        out.push('}');
    });
    if let Some(outcome) = &decision.outcome {
        json::member(&mut out, "outcome");
        json::push_text(&mut out, outcome);
    }
    json::member(&mut out, "canon");
    json::push_value(&mut out, &decision.canon);
    out.push_str("}\n");
    out
}

/// `refusal` as one `loom.evaluation.EvaluationRefusal` JSON document, ending with a newline.
pub fn refusal_json(refusal: &EvaluationRefusal) -> String {
    let mut out = String::from("{");
    json::member(&mut out, "input");
    json::push_text(&mut out, input_name(refusal.input));
    if let Some(index) = refusal.evidence_index {
        json::member(&mut out, "evidence_index");
        json::push_integer(&mut out, index);
    }
    json::member(&mut out, "code");
    json::push_text(&mut out, &refusal.code);
    json::member(&mut out, "message");
    json::push_text(&mut out, &refusal.message);
    out.push_str("}\n");
    out
}

/// The input `refusal` is about, in words: `protocol`, `snapshot`, `evidence record <n>`, `time`
/// or `request`.
pub fn refused_input(refusal: &EvaluationRefusal) -> String {
    match (refusal.input, refusal.evidence_index) {
        (EvaluationInput::Evidence, Some(index)) => format!("evidence record {index}"),
        (input, _) => input_name(input).to_lowercase(),
    }
}

fn input_name(input: EvaluationInput) -> &'static str {
    match input {
        EvaluationInput::Request => "Request",
        EvaluationInput::Protocol => "Protocol",
        EvaluationInput::Snapshot => "Snapshot",
        EvaluationInput::Evidence => "Evidence",
        EvaluationInput::Time => "Time",
    }
}

fn refused(input: EvaluationInput, message: String) -> EvaluationRefusal {
    EvaluationRefusal {
        input,
        evidence_index: None,
        code: "malformed".to_owned(),
        message,
    }
}

fn array<T>(out: &mut String, items: &[T], mut write: impl FnMut(&mut String, &T)) {
    out.push('[');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write(out, item);
    }
    out.push(']');
}
