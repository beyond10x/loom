//! Adversary, wave 2026-11-21-w4 unit U3 (story:engineering-registry-calls-replaced).
//!
//! What `classify` shows the model is behaviour: the protocols, in the order the base commit
//! (`63668a97`) showed them, written out here as a literal rather than read from the catalog under
//! test. And `classify` building the catalog before its threshold check must not change what an
//! out-of-range threshold answers, nor ask the model. The model is a recorded fake.

use b10x_loom_intake_router::{RouterError, classify};
use llm_core::{
    BoxFuture, CallId, Cancel, Capabilities, Error, Id, Item, Model, Protocol, Provenance,
    StopReason, StreamSink, ToolCall, ToolName, TurnObservation, TurnOutcome, TurnRequest,
};
use serde_json::json;
use std::sync::Mutex;

/// The protocols the base commit offered, in the order it offered them.
const BASE_ORDER: [&str; 10] = [
    "eval-gated-change@1",
    "incident-response@1",
    "loop-improvement@1",
    "ml-experiment@1",
    "protocol-adopt@1",
    "self-improvement@1",
    "software-change@1",
    "story-delivery@1",
    "story-delivery@2",
    "support-triage@1",
];

struct Recorded {
    provenance: Provenance,
    capabilities: Capabilities,
    seen: Mutex<Vec<TurnRequest>>,
}

impl Recorded {
    fn new() -> Self {
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
                ..Capabilities::text(32_768, 2_048)
            },
            seen: Mutex::new(Vec::new()),
        }
    }

    fn turns(&self) -> usize {
        self.seen.lock().expect("unpoisoned").len()
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
                name: ToolName::new("pick_protocol").expect("tool name"),
                arguments: json!({
                    "protocol": "software-change@1",
                    "confidence": 0.9,
                    "reasons": ["a change"]
                }),
            })],
            observation,
        };
        Box::pin(async move { Ok(outcome) })
    }
}

#[tokio::test]
async fn adversary_the_model_is_shown_the_base_protocols_in_the_base_order() {
    let model = Recorded::new();
    let pick = classify("change the parser", &model, 0.5)
        .await
        .expect("a pick");
    assert_eq!(pick.protocol, "software-change@1");
    let request = model.seen.lock().expect("unpoisoned")[0].clone();
    let offered: Vec<String> = request.tools[0].input_schema["properties"]["protocol"]["enum"]
        .as_array()
        .expect("`protocol` is an enum")
        .iter()
        .map(|entry| entry.as_str().expect("a string entry").to_owned())
        .collect();
    assert_eq!(offered, BASE_ORDER, "the enum the model sees");
    let mut last = 0;
    for name in BASE_ORDER {
        let line = format!("\n- {name}: ");
        let at = request
            .instructions
            .find(&line)
            .unwrap_or_else(|| panic!("{line:?} not in the instructions"));
        assert!(at >= last, "{name} is out of the base order");
        last = at;
    }
    assert_eq!(
        request.instructions.matches("\n- ").count(),
        BASE_ORDER.len(),
        "one protocol line per protocol, nothing else listed"
    );
}

#[tokio::test]
async fn adversary_an_out_of_range_threshold_is_refused_before_any_call() {
    for threshold in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
        let model = Recorded::new();
        match classify("change the parser", &model, threshold).await {
            Err(RouterError::InvalidThreshold(given)) => {
                assert!(given.is_nan() && threshold.is_nan() || given == threshold)
            }
            other => panic!("threshold {threshold}: InvalidThreshold, got {other:?}"),
        }
        assert_eq!(model.turns(), 0, "threshold {threshold}: no model asked");
    }
}
