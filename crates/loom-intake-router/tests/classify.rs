//! Acceptance for story `router-classifier`: an intent picks one protocol of the ELS registry.
//!
//! Every model here is a recorded fake: it answers each turn with one fixed `pick_protocol` call
//! and keeps the request it saw. No test reaches a model, the network or a credential.

use b10x_loom_intake_router::{ProtocolPick, RouterError, classify};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolChoice, ToolName, TurnObservation, TurnOutcome,
    TurnRequest,
};
use serde_json::{Value, json};
use std::sync::Mutex;

/// The tool `classify` forces.
const TOOL: &str = "pick_protocol";

/// The threshold every case classifies against.
const THRESHOLD: f64 = 0.5;

/// A recorded model: answers every turn with one `pick_protocol` call carrying `arguments`, and
/// keeps the requests it saw.
struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    arguments: Value,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Recorded {
    fn answering(arguments: Value) -> Self {
        let id = |value: &str| Id::new(value).expect("fixture identifier");
        Self {
            provenance: Provenance {
                protocol: Protocol::Responses,
                provider: id("recorded"),
                account: id("recorded"),
                endpoint: id("recorded"),
                model: id("recorded-model"),
                binding_revision: id("rev-1"),
            },
            capabilities: Capabilities {
                tools: true,
                tool_choice: true,
                temperature: false,
                top_p: false,
                reasoning_efforts: Vec::new(),
                context_window: 32_768,
                max_output_tokens: 2_048,
            },
            arguments,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// The one request this model saw.
    fn request(&self) -> TurnRequest {
        let seen = self.seen.lock().expect("unpoisoned");
        assert_eq!(seen.len(), 1, "one turn, no retry");
        seen[0].clone()
    }
}

impl Model for Recorded {
    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        _sink: &'a mut dyn StreamSink,
        _cancel: &'a Cancel,
    ) -> BoxFuture<'a, Result<TurnOutcome, Error>> {
        self.seen.lock().expect("unpoisoned").push(request.clone());
        let mut observation = TurnObservation::new(self.provenance.clone());
        observation.final_usage = true;
        let outcome = TurnOutcome {
            stop_reason: StopReason::ToolCalls,
            items: vec![Item::ToolCall(ToolCall {
                call_id: CallId::new("call_1").expect("call id"),
                name: ToolName::new(TOOL).expect("tool name"),
                arguments: self.arguments.clone(),
            })],
            observation,
        };
        Box::pin(async move { Ok(outcome) })
    }
}

/// Every registry entry as `name@major`, in registry order.
fn registry() -> Vec<String> {
    canon_engineering::registry::list()
        .into_iter()
        .map(|(name, major)| format!("{name}@{major}"))
        .collect()
}

/// Every string anywhere in `value`, one per line.
fn strings(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => {
            out.push_str(text);
            out.push('\n');
        }
        Value::Array(values) => values.iter().for_each(|value| strings(value, out)),
        Value::Object(fields) => fields.values().for_each(|value| strings(value, out)),
        _ => {}
    }
}

/// Asserts what every `classify` request carries: the intent, the forced `pick_protocol` tool
/// whose `protocol` is an enum of the registry, and each protocol's Canon description and
/// artifact descriptions.
fn assert_request(model: &Recorded, intent: &str) {
    let request = model.request();
    let mut text = String::new();
    strings(
        &serde_json::to_value(&request).expect("a serialisable request"),
        &mut text,
    );
    assert!(text.contains(intent), "the intent was not sent: {text}");

    assert_eq!(request.tools.len(), 1, "one tool, the forced one");
    let tool = &request.tools[0];
    assert_eq!(tool.name.as_str(), TOOL);
    assert_eq!(
        request.tool_choice,
        ToolChoice::Named(ToolName::new(TOOL).expect("tool name"))
    );
    let properties = &tool.input_schema["properties"];
    let mut offered: Vec<String> = properties["protocol"]["enum"]
        .as_array()
        .expect("`protocol` is an enum")
        .iter()
        .map(|entry| entry.as_str().expect("a string entry").to_owned())
        .collect();
    offered.sort();
    let mut listed = registry();
    listed.sort();
    assert_eq!(offered, listed, "the enum is the registry, nothing else");
    assert!(listed.contains(&"software-change@1".to_owned()));
    assert!(listed.contains(&"incident-response@1".to_owned()));
    assert!(properties.get("confidence").is_some(), "{properties}");
    assert!(properties.get("reasons").is_some(), "{properties}");

    for (name, major) in canon_engineering::registry::list() {
        let builtin = canon_engineering::registry::get(name, major).expect("a valid built-in");
        let description = builtin
            .model
            .protocol
            .description
            .as_deref()
            .expect("every built-in describes itself");
        assert!(
            text.contains(description),
            "{name}@{major}'s description was not sent: {text}"
        );
        for (artifact, declared) in builtin.model.artifacts.iter() {
            if let Some(description) = declared.description.as_deref() {
                assert!(
                    text.contains(description),
                    "{name}@{major} artifact {artifact:?}'s description was not sent: {text}"
                );
            }
        }
    }
}

#[tokio::test]
async fn intents_pick_their_protocol() {
    // A software change.
    let intent = "make the failing test pass";
    let reasons = vec!["the intent asks for a code change verified by a test".to_owned()];
    let model = Recorded::answering(json!({
        "protocol": "software-change@1",
        "confidence": 0.9,
        "reasons": reasons,
    }));
    let pick: ProtocolPick = classify(intent, &model, THRESHOLD)
        .await
        .expect("a software change is picked");
    assert_eq!(pick.protocol, "software-change@1");
    assert_eq!(pick.confidence, 0.9);
    assert_eq!(pick.reasons, reasons);
    assert_request(&model, intent);

    // An incident.
    let intent = "the checkout service is down";
    let reasons = vec!["a running service is unavailable".to_owned()];
    let model = Recorded::answering(json!({
        "protocol": "incident-response@1",
        "confidence": 0.8,
        "reasons": reasons,
    }));
    let pick = classify(intent, &model, THRESHOLD)
        .await
        .expect("an incident is picked");
    assert_eq!(pick.protocol, "incident-response@1");
    assert_eq!(pick.confidence, 0.8);
    assert_eq!(pick.reasons, reasons);
    assert_request(&model, intent);

    // A confident pick of a protocol the registry does not hold is refused, not guessed.
    let intent = "ship it all to production now";
    let model = Recorded::answering(json!({
        "protocol": "deploy-everything@1",
        "confidence": 0.95,
        "reasons": ["the intent asks for a deployment"],
    }));
    let error = classify(intent, &model, THRESHOLD)
        .await
        .expect_err("a pick outside the registry is refused");
    match error {
        RouterError::OutsideRegistry { protocol } => assert_eq!(protocol, "deploy-everything@1"),
        other => panic!("expected OutsideRegistry, got {other:?}"),
    }
    assert_request(&model, intent);

    // A registry pick below the threshold is refused as unsure.
    let intent = "look into the thing from yesterday";
    let model = Recorded::answering(json!({
        "protocol": "software-change@1",
        "confidence": 0.2,
        "reasons": ["the intent names no change and no service"],
    }));
    let error = classify(intent, &model, THRESHOLD)
        .await
        .expect_err("a pick below the threshold is refused");
    match error {
        RouterError::Unsure {
            protocol,
            confidence,
            threshold,
        } => {
            assert_eq!(protocol, "software-change@1");
            assert_eq!(confidence, 0.2);
            assert_eq!(threshold, THRESHOLD);
        }
        other => panic!("expected Unsure, got {other:?}"),
    }
    assert_request(&model, intent);
}
