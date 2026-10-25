//! The JSON of A2A 1.0.1 for the protocol types of the `a2a` crate this listener answers with:
//! the protocol buffer JSON mapping of `a2a.proto` (member names in lower camel case, enums by
//! their value names, an absent optional member left out). The types are a2a's, pinned by tag in
//! `Cargo.toml`, so a field the protocol makes required cannot be left out here without the build
//! failing.

use a2a::discovery::{
    AgentCapabilities, AgentCard, AgentInterface, AgentSkill, HTTPAuthSecurityScheme,
    SecurityRequirement, SecurityScheme,
};
use a2a::tasks::{Artifact, Message, Part, Role, TaskSnapshot, TaskStatus, WireTaskState};
use serde_json::{Map, Value, json};

/// Inserts `value` under `name` when it is present.
fn optional(object: &mut Map<String, Value>, name: &str, value: Option<Value>) {
    if let Some(value) = value {
        object.insert(name.to_owned(), value);
    }
}

fn strings(list: &[String]) -> Value {
    Value::Array(list.iter().cloned().map(Value::String).collect())
}

/// `value` of a2a's JSON as `serde_json`'s.
pub(crate) fn json_value(value: &a2a::json::Value) -> Value {
    let mut text = String::new();
    a2a::json::push_value(&mut text, value);
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

/// The agent card.
pub fn card(card: &AgentCard) -> Value {
    let mut object = Map::new();
    object.insert("name".to_owned(), json!(card.name));
    object.insert("description".to_owned(), json!(card.description));
    object.insert(
        "supportedInterfaces".to_owned(),
        Value::Array(card.supported_interfaces.iter().map(interface).collect()),
    );
    object.insert("version".to_owned(), json!(card.version));
    optional(
        &mut object,
        "documentationUrl",
        card.documentation_url.clone().map(Value::String),
    );
    object.insert("capabilities".to_owned(), capabilities(&card.capabilities));
    optional(
        &mut object,
        "securitySchemes",
        card.security_schemes.as_ref().map(|schemes| {
            Value::Object(
                schemes
                    .iter()
                    .map(|(name, scheme)| (name.clone(), security_scheme(scheme)))
                    .collect(),
            )
        }),
    );
    optional(
        &mut object,
        "securityRequirements",
        card.security_requirements
            .as_ref()
            .map(|requirements| Value::Array(requirements.iter().map(requirement).collect())),
    );
    object.insert(
        "defaultInputModes".to_owned(),
        strings(&card.default_input_modes),
    );
    object.insert(
        "defaultOutputModes".to_owned(),
        strings(&card.default_output_modes),
    );
    object.insert(
        "skills".to_owned(),
        Value::Array(card.skills.iter().map(skill).collect()),
    );
    optional(
        &mut object,
        "iconUrl",
        card.icon_url.clone().map(Value::String),
    );
    Value::Object(object)
}

fn interface(interface: &AgentInterface) -> Value {
    let mut object = Map::new();
    object.insert("url".to_owned(), json!(interface.url));
    object.insert(
        "protocolBinding".to_owned(),
        json!(interface.protocol_binding),
    );
    optional(
        &mut object,
        "tenant",
        interface.tenant.clone().map(Value::String),
    );
    object.insert(
        "protocolVersion".to_owned(),
        json!(interface.protocol_version),
    );
    Value::Object(object)
}

fn capabilities(capabilities: &AgentCapabilities) -> Value {
    let mut object = Map::new();
    optional(
        &mut object,
        "streaming",
        capabilities.streaming.map(Value::Bool),
    );
    optional(
        &mut object,
        "pushNotifications",
        capabilities.push_notifications.map(Value::Bool),
    );
    optional(
        &mut object,
        "extendedAgentCard",
        capabilities.extended_agent_card.map(Value::Bool),
    );
    Value::Object(object)
}

fn security_scheme(scheme: &SecurityScheme) -> Value {
    let mut object = Map::new();
    optional(
        &mut object,
        "httpAuthSecurityScheme",
        scheme.http_auth_security_scheme.as_ref().map(http_auth),
    );
    Value::Object(object)
}

fn http_auth(scheme: &HTTPAuthSecurityScheme) -> Value {
    let mut object = Map::new();
    optional(
        &mut object,
        "description",
        scheme.description.clone().map(Value::String),
    );
    object.insert("scheme".to_owned(), json!(scheme.scheme));
    optional(
        &mut object,
        "bearerFormat",
        scheme.bearer_format.clone().map(Value::String),
    );
    Value::Object(object)
}

fn requirement(requirement: &SecurityRequirement) -> Value {
    json!({
        "schemes": requirement
            .schemes
            .iter()
            .map(|(name, list)| (name.clone(), json!({"list": list.list})))
            .collect::<Map<String, Value>>()
    })
}

fn skill(skill: &AgentSkill) -> Value {
    let mut object = Map::new();
    object.insert("id".to_owned(), json!(skill.id));
    object.insert("name".to_owned(), json!(skill.name));
    object.insert("description".to_owned(), json!(skill.description));
    object.insert("tags".to_owned(), strings(&skill.tags));
    optional(
        &mut object,
        "examples",
        skill.examples.as_deref().map(strings),
    );
    optional(
        &mut object,
        "inputModes",
        skill.input_modes.as_deref().map(strings),
    );
    optional(
        &mut object,
        "outputModes",
        skill.output_modes.as_deref().map(strings),
    );
    Value::Object(object)
}

/// A task.
pub fn task(task: &TaskSnapshot) -> Value {
    let mut object = Map::new();
    object.insert("id".to_owned(), json!(task.id.0));
    optional(
        &mut object,
        "contextId",
        task.context_id.as_ref().map(|id| json!(id.0)),
    );
    object.insert("status".to_owned(), status(&task.status));
    optional(
        &mut object,
        "artifacts",
        task.artifacts
            .as_ref()
            .map(|artifacts| Value::Array(artifacts.iter().map(artifact).collect())),
    );
    optional(
        &mut object,
        "history",
        task.history
            .as_ref()
            .map(|history| Value::Array(history.iter().map(message).collect())),
    );
    optional(
        &mut object,
        "metadata",
        task.metadata.as_ref().map(json_value),
    );
    Value::Object(object)
}

fn status(status: &TaskStatus) -> Value {
    let mut object = Map::new();
    object.insert("state".to_owned(), json!(state(status.state)));
    optional(&mut object, "message", status.message.as_ref().map(message));
    optional(
        &mut object,
        "timestamp",
        status.timestamp.as_ref().map(|at| json!(at.0)),
    );
    Value::Object(object)
}

/// `state` as `a2a.proto` spells it.
pub fn state(state: WireTaskState) -> &'static str {
    match state {
        WireTaskState::TASKSTATESUBMITTED => "TASK_STATE_SUBMITTED",
        WireTaskState::TASKSTATEWORKING => "TASK_STATE_WORKING",
        WireTaskState::TASKSTATECOMPLETED => "TASK_STATE_COMPLETED",
        WireTaskState::TASKSTATEFAILED => "TASK_STATE_FAILED",
        WireTaskState::TASKSTATECANCELED => "TASK_STATE_CANCELED",
        WireTaskState::TASKSTATEINPUTREQUIRED => "TASK_STATE_INPUT_REQUIRED",
        WireTaskState::TASKSTATEREJECTED => "TASK_STATE_REJECTED",
        WireTaskState::TASKSTATEAUTHREQUIRED => "TASK_STATE_AUTH_REQUIRED",
    }
}

/// A message.
pub fn message(message: &Message) -> Value {
    let mut object = Map::new();
    object.insert("messageId".to_owned(), json!(message.message_id));
    optional(
        &mut object,
        "contextId",
        message.context_id.as_ref().map(|id| json!(id.0)),
    );
    optional(
        &mut object,
        "taskId",
        message.task_id.as_ref().map(|id| json!(id.0)),
    );
    object.insert(
        "role".to_owned(),
        json!(match message.role {
            Role::ROLEUSER => "ROLE_USER",
            Role::ROLEAGENT => "ROLE_AGENT",
        }),
    );
    object.insert(
        "parts".to_owned(),
        Value::Array(message.parts.iter().map(part).collect()),
    );
    Value::Object(object)
}

fn artifact(artifact: &Artifact) -> Value {
    let mut object = Map::new();
    object.insert("artifactId".to_owned(), json!(artifact.artifact_id));
    optional(
        &mut object,
        "name",
        artifact.name.clone().map(Value::String),
    );
    optional(
        &mut object,
        "description",
        artifact.description.clone().map(Value::String),
    );
    object.insert(
        "parts".to_owned(),
        Value::Array(artifact.parts.iter().map(part).collect()),
    );
    Value::Object(object)
}

fn part(part: &Part) -> Value {
    let mut object = Map::new();
    optional(&mut object, "text", part.text.clone().map(Value::String));
    optional(&mut object, "url", part.url.clone().map(Value::String));
    optional(&mut object, "data", part.data.as_ref().map(json_value));
    optional(
        &mut object,
        "mediaType",
        part.media_type.clone().map(Value::String),
    );
    Value::Object(object)
}

/// A text part.
pub(crate) fn text_part(text: &str) -> Part {
    Part {
        text: Some(text.to_owned()),
        raw: None,
        url: None,
        data: None,
        metadata: None,
        filename: None,
        media_type: Some("text/plain".to_owned()),
    }
}

/// A data part holding `data`.
pub(crate) fn data_part(data: a2a::json::Value) -> Part {
    Part {
        text: None,
        raw: None,
        url: None,
        data: Some(data),
        metadata: None,
        filename: None,
        media_type: Some("application/json".to_owned()),
    }
}

/// The text of a message's parts as a request carries them: every part with a `text` member, in
/// order, joined by a blank line. `None` when no part carries text.
pub(crate) fn request_text(parts: &[Value]) -> Option<String> {
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect();
    (!texts.is_empty()).then(|| texts.join("\n\n"))
}
